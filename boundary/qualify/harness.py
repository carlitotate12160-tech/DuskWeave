#!/usr/bin/env python3
"""ADR-015 boundary qualification harness. Runs on the candidate host only;
every scenario is self-contained on a dedicated attempt bridge with a
host-local stub responder — no external network is contacted. Emits one
evidence JSON per scenario plus a verdict summary."""
import json
import os
import secrets
import struct
import subprocess
import sys
import time
import uuid

HERE = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.dirname(os.path.dirname(HERE))
LAUNCHER = os.path.join(REPO, "..", "target", "debug", "dw-boundary-launcher")
STUB = os.path.join(HERE, "stub_responder.py")
PROBE = os.path.join(HERE, "probe_capture.py")
CONTROL = os.path.expanduser("~/duskweave/qualify/control")
DENIED_IP = "198.51.100.99"  # TEST-NET-2: unroutable, no live contact
SUBNET_BASE = 10


def run(cmd, data=None, timeout=40, sudo=False):
    if sudo:
        cmd = ["sudo", "-n"] + cmd
    p = subprocess.run(
        cmd, input=data, capture_output=True, text=True, timeout=timeout
    )
    return p


def sh_json(out):
    line = out.stdout.strip().splitlines()
    return json.loads(line[-1]) if line else {}


def net_create(tag, session, subnet):
    bridge = f"dwcap{tag}"
    rc = run([
        "docker", "network", "create",
        "--opt", f"com.docker.network.bridge.name={bridge}",
        "--subnet", subnet,
        # dw.fixture, not dw.session: the launcher sweeps dw.session objects
        # at attempt start and would delete this stub bridge out from under
        # the fixture. Harness-owned net, torn down by net_teardown.
        "--label", f"dw.fixture={session}",
        f"dw-cap-{tag}",
    ])
    if rc.returncode != 0:
        raise RuntimeError(f"net_create failed: {rc.stderr.strip()[:200]}")
    gw = run([
        "docker", "network", "inspect", "-f",
        "{{(index .IPAM.Config 0).Gateway}}", f"dw-cap-{tag}",
    ]).stdout.strip()
    # Fixture plumbing only: the host-local stub sits at the bridge gateway,
    # so its packets arrive on the host INPUT path where ufw policy-drops
    # unknown traffic. Admit exactly this attempt bridge + stub port for the
    # scenario window; production endpoints are external (FORWARD path) and
    # never need this rule.
    run(["iptables", "-I", "INPUT", "1", "-i", bridge, "-p", "udp",
         "--dport", "5353", "-j", "ACCEPT"], sudo=True)
    return f"dw-cap-{tag}", bridge, gw


def net_teardown(bridge):
    run(["iptables", "-D", "INPUT", "-i", bridge, "-p", "udp",
         "--dport", "5353", "-j", "ACCEPT"], sudo=True)
    # The launcher normally removes its adopted net; defensive cleanup for
    # early-failure paths where the adopt/remove sequence never ran.
    run(["docker", "network", "rm", f"dw-cap-{bridge[len('dwcap'):]}"],
        sudo=False)


def stub_start(gw, port, mode):
    return subprocess.Popen(
        ["python3", STUB, gw, str(port), mode],
        stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
    )


def attempt_request(image, session, attempt, gw, port=5353, deadline_s=30):
    return {
        "version": "dw_boundary_attempt_v1",
        "session_id": session,
        "attempt_id": attempt,
        "control_root": CONTROL,
        "image": image,
        "effect": "dns_udp",
        "qname": "probe.example.invalid",
        "qtype": "A",
        "endpoint": {"ipv4": gw, "port": port, "transport": "udp"},
        "txid": secrets.randbelow(65535),
        "deadline_ms": int(time.time() * 1000) + deadline_s * 1000,
        "max_wire": 512,
        "max_raw": 2048,
        "lease_ms": 2000,
        "renew_ms": 400,
    }


def launch():
    return subprocess.Popen(
        ["sudo", "-n", LAUNCHER, "run-attempt"],
        stdin=subprocess.PIPE, stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )


def _frame(obj):
    raw = json.dumps(obj).encode()
    return struct.pack(">I", len(raw)) + raw


def begin_attempt(image, session, gw, port=5353, deadline_s=30, attempt=None):
    attempt = attempt or uuid.uuid4().hex
    req = attempt_request(image, session, attempt, gw, port, deadline_s)
    p = launch()
    p.stdin.write(_frame(req))
    p.stdin.flush()
    return p, req


def send_stop(p, attempt):
    """In-flight STOP on the attempt's private stdin channel (ADR-017
    framed control message). The file latch stays for pre-attempt checks."""
    try:
        p.stdin.write(_frame({"version": "dw_boundary_request_v1",
                              "phase": "stop", "attempt_id": attempt}))
        p.stdin.flush()
    except BrokenPipeError:
        pass


def wait_receipt(p, timeout=50):
    """Read the receipt line off stdout with a deadline. Do not rely on
    p.wait(): under sudo `use_pty` the launcher exits but can linger as a
    zombie while the sudo wrapper holds the pty, so process-exit is not a
    reliable end-of-attempt signal. The receipt line is."""
    import select
    deadline = time.monotonic() + timeout
    lines = []
    rec = None
    while time.monotonic() < deadline and rec is None:
        ready, _, _ = select.select([p.stdout], [], [], min(0.5, deadline - time.monotonic()))
        if ready:
            line = p.stdout.readline()
            if not line:
                break
            lines.append(line.decode("utf-8", "replace"))
            try:
                cand = json.loads(line)
                if isinstance(cand, dict) and cand.get("version") == "dw_boundary_receipt_v1":
                    rec = cand
            except json.JSONDecodeError:
                pass
        if p.poll() is not None and not ready:
            break
    if rec is None:
        for line in reversed(lines):
            try:
                cand = json.loads(line)
            except json.JSONDecodeError:
                continue
            if isinstance(cand, dict) and cand.get("version") == "dw_boundary_receipt_v1":
                rec = cand
                break
    if rec is None:
        try:
            rec = json.loads("".join(lines).strip().splitlines()[-1])
        except (json.JSONDecodeError, IndexError):
            return {"disposition": "parse_failure", "raw": "".join(lines)[-400:]}
    # Drain remaining stdout/stderr briefly; reap the sudo wrapper if it
    # is still lingering.
    try:
        p.wait(timeout=2)
    except subprocess.TimeoutExpired:
        p.kill()
        try:
            p.wait(timeout=3)
        except subprocess.TimeoutExpired:
            pass
    try:
        tail = p.stderr.read()[-1500:].decode("utf-8", "replace")
        if tail.strip():
            rec["stderr_tail"] = tail
    except Exception:
        pass
    return rec


def leftovers(session):
    nets = run(["docker", "network", "ls", "-q",
                "--filter", f"label=dw.session={session}"]).stdout.strip()
    ctrs = run(["docker", "ps", "-aq",
                "--filter", f"label=dw.session={session}"]).stdout.strip()
    tbls = run(["sudo", "-n", "nft", "list", "tables", "inet"],
               ).stdout
    tbl_left = [t for t in tbls.splitlines() if "dwatt" in t]
    return {"networks": nets.split(), "containers": ctrs.split(),
            "nft_tables": tbl_left}


def probe(image, net, ip, port, attempt, session):
    p = run(["python3", PROBE, image, net, ip, str(port), attempt, session],
            timeout=15)
    return json.loads(p.stdout.strip())


def inspect_pair(tag):
    out = {}
    for role in ("capture", "exporter"):
        p = run(["docker", "inspect", "-f",
                 "{{json .HostConfig}}|{{.Config.User}}|{{.Config.Image}}",
                 f"dw-{role}-{tag}"])
        if p.returncode == 0 and "|" in p.stdout:
            host, user, _img = p.stdout.strip().split("|", 2)
            cfg = json.loads(host)
            ul = {u["Name"]: (u["Soft"], u["Hard"]) for u in cfg.get("Ulimits") or []}
            out[role] = {
                "read_only_rootfs": cfg.get("ReadonlyRootfs"),
                "memory": cfg.get("Memory"),
                "memory_swap": cfg.get("MemorySwap"),
                "cap_drop_all": "ALL" in (cfg.get("CapDrop") or []),
                "no_new_privileges": any(
                    "no-new-privileges" in s for s in cfg.get("SecurityOpt") or []),
                "log_driver": (cfg.get("LogConfig") or {}).get("Type"),
                "pids_limit": cfg.get("PidsLimit"),
                "core_rlimit": ul.get("core"),
                "nofile_rlimit": ul.get("nofile"),
                "uid": user,
            }
    return out


def scenario_happy(image, session):
    attempt = uuid.uuid4().hex
    tag = attempt[:10]
    _net, br, gw = net_create(tag, session, f"10.245.{SUBNET_BASE}.0/24")
    stub = stub_start(gw, 5353, "normal")
    p, _req = begin_attempt(image, session, gw, attempt=attempt)
    receipt = wait_receipt(p)
    try:
        stub.terminate(); stub.wait(timeout=5)
    except Exception:
        stub.kill()
    net_teardown(br)
    return {"scenario": "happy_path", "receipt": receipt,
            "residual": leftovers(session)}


def scenario_denied(image, session):
    attempt = uuid.uuid4().hex
    tag = attempt[:10]
    net, br, gw = net_create(tag, session, "10.245.11.0/24")
    stub = stub_start(gw, 5353, "slow")
    p, _req = begin_attempt(image, session, gw, deadline_s=45, attempt=attempt)
    time.sleep(0.7)
    denied = probe(image, net, DENIED_IP, 53, uuid.uuid4().hex, session)
    admitted = probe(image, net, gw, 5353, uuid.uuid4().hex, session)
    receipt = wait_receipt(p)
    try:
        stub.terminate(); stub.wait(timeout=5)
    except Exception:
        stub.kill()
    net_teardown(br)
    return {"scenario": "egress_filter", "receipt": receipt,
            "denied_probe": denied, "admitted_probe": admitted,
            "residual": leftovers(session)}


def _launcher_pids():
    """PIDs of the real launcher processes (pgrep -f also matches the sudo
    wrapper, so filter on /proc/<pid>/comm)."""
    out = run(["pgrep", "-f", "dw-boundary-launcher run-attempt"]).stdout
    pids = []
    for pid in out.split():
        comm = run(["cat", f"/proc/{pid}/comm"]).stdout.strip()
        if comm.startswith("dw-boundary-lau"):
            pids.append(pid)
    return pids


def scenario_lease_expiry(image, session):
    attempt = uuid.uuid4().hex
    tag = attempt[:10]
    net, br, gw = net_create(tag, session, "10.245.12.0/24")
    stub = stub_start(gw, 5353, "slow")
    p, _req = begin_attempt(image, session, gw, deadline_s=45, attempt=attempt)
    time.sleep(0.7)
    # Freeze the launcher itself (the child of the sudo wrapper), not p.pid:
    # stopping the renewer lets the lease element expire mid-attempt.
    pids = _launcher_pids()
    for pid in pids:
        run(["kill", "-STOP", pid], sudo=True)
    time.sleep(2.6)
    frozen = probe(image, net, gw, 5353, uuid.uuid4().hex, session)
    for pid in pids:
        run(["kill", "-CONT", pid], sudo=True)
    receipt = wait_receipt(p)
    try:
        stub.terminate(); stub.wait(timeout=5)
    except Exception:
        stub.kill()
    net_teardown(br)
    return {"scenario": "lease_expiry", "receipt": receipt,
            "frozen_window_probe": frozen, "residual": leftovers(session)}


def scenario_stop(image, session):
    attempt = uuid.uuid4().hex
    tag = attempt[:10]
    _net, br, gw = net_create(tag, session, "10.245.13.0/24")
    stub = stub_start(gw, 5353, "slow")
    p, req = begin_attempt(image, session, gw, deadline_s=45, attempt=attempt)
    time.sleep(0.7)
    t0 = time.monotonic()
    send_stop(p, attempt)
    receipt = wait_receipt(p)
    stop_ms = int((time.monotonic() - t0) * 1000)
    try:
        stub.terminate(); stub.wait(timeout=5)
    except Exception:
        stub.kill()
    net_teardown(br)
    return {"scenario": "stop_latch", "receipt": receipt,
            "stop_to_receipt_ms": stop_ms, "residual": leftovers(session)}


def scenario_isolation(image, session):
    attempt = uuid.uuid4().hex
    tag = attempt[:10]
    _net, br, gw = net_create(tag, session, "10.245.14.0/24")
    stub = stub_start(gw, 5353, "slow")
    p, _req = begin_attempt(image, session, gw, deadline_s=45, attempt=attempt)
    time.sleep(0.6)
    iso = inspect_pair(tag)
    receipt = wait_receipt(p)
    try:
        stub.terminate(); stub.wait(timeout=5)
    except Exception:
        stub.kill()
    net_teardown(br)
    return {"scenario": "isolation_config", "receipt": receipt,
            "isolation": iso, "residual": leftovers(session)}


def scenario_malformed(image, session, mode, subnet):
    attempt = uuid.uuid4().hex
    tag = attempt[:10]
    _net, br, gw = net_create(tag, session, subnet)
    stub = stub_start(gw, 5353, mode)
    p, _req = begin_attempt(image, session, gw, attempt=attempt)
    receipt = wait_receipt(p)
    try:
        stub.terminate(); stub.wait(timeout=5)
    except Exception:
        stub.kill()
    net_teardown(br)
    return {"scenario": f"rejection_{mode}", "receipt": receipt,
            "residual": leftovers(session)}


def scenario_lease_static():
    tag = secrets.token_hex(5)
    r1 = run(["sudo", "-n", "nft", "-f", "-"],
             data=f"add table inet dwlease{tag}\n"
                  f"add set inet dwlease{tag} s "
                  "{ type ipv4_addr . inet_service; flags timeout; }\n"
                  f"add element inet dwlease{tag} s "
                  "{ 203.0.113.7 . 53 timeout 1000ms }\n")
    present = run(["sudo", "-n", "nft", "list", "set", "inet",
                   f"dwlease{tag}", "s"]).stdout
    time.sleep(1.6)
    after = run(["sudo", "-n", "nft", "list", "set", "inet",
                 f"dwlease{tag}", "s"]).stdout
    run(["sudo", "-n", "nft", "delete", "table", "inet", f"dwlease{tag}"])
    return {"scenario": "lease_static_expiry",
            "element_present_at_add": "203.0.113.7" in present,
            "element_gone_after_timeout": "203.0.113.7" not in after,
            "nft_rc": r1.returncode}


def main() -> int:
    if len(sys.argv) != 3:
        sys.stderr.write("usage: harness.py <image@sha256> <out_dir>\n")
        return 2
    image, outdir = sys.argv[1], sys.argv[2]
    os.makedirs(CONTROL, exist_ok=True)
    os.makedirs(outdir, exist_ok=True)
    session = f"q{secrets.token_hex(6)}"
    evidence = {"session": session, "image": image,
                "started_at": int(time.time()), "scenarios": []}
    pre = run([LAUNCHER, "preflight"], sudo=True)
    evidence["preflight"] = sh_json(pre)
    for fn in (scenario_happy, scenario_denied, scenario_lease_expiry,
               scenario_stop, scenario_isolation):
        try:
            evidence["scenarios"].append(fn(image, session))
        except Exception as e:  # noqa: BLE001 - qualification records, never hides
            evidence["scenarios"].append({"scenario": fn.__name__,
                                          "harness_error": repr(e)})
    for mode, subnet in (("garbage", "10.245.15.0/24"),
                         ("forged", "10.245.16.0/24"),
                         ("oversize", "10.245.17.0/24"),
                         ("nxdomain", "10.245.18.0/24")):
        try:
            evidence["scenarios"].append(scenario_malformed(image, session, mode, subnet))
        except Exception as e:  # noqa: BLE001
            evidence["scenarios"].append({"scenario": f"rejection_{mode}",
                                          "harness_error": repr(e)})
    evidence["scenarios"].append(scenario_lease_static())
    evidence["finished_at"] = int(time.time())
    out = os.path.join(outdir, f"qualification-{session}.json")
    with open(out, "w") as fh:
        json.dump(evidence, fh, indent=1)
    print(out)
    return 0


if __name__ == "__main__":
    sys.exit(main())
