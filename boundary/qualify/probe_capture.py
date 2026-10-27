#!/usr/bin/env python3
"""Qualification driver that speaks ADR-017 framing to a manually spawned
capture container on the attempt bridge. Used to prove the verdict chain:
an admitted endpoint answers; any other destination is dropped at the host.
Runs on the qualified host only; never part of the runtime path.
"""
import json
import struct
import subprocess
import sys
import time

REQ = "dw_boundary_request_v1"


def frame(body: dict) -> bytes:
    raw = json.dumps(body).encode()
    return struct.pack(">I", len(raw)) + raw


def read_frame(sock_file):
    hdr = sock_file.read(4)
    if len(hdr) < 4:
        return None, b""
    n = struct.unpack(">I", hdr)[0]
    if n > 16 * 1024:
        return None, b""
    return json.loads(sock_file.read(n)), None


def run_probe(image, net, endpoint_ip, endpoint_port, attempt, session):
    req = {
        "version": REQ,
        "kind": "capture",
        "attempt_id": attempt,
        "session_id": session,
        "effect": "dns_udp",
        "qname": "probe.example.invalid",
        "qtype": "A",
        "endpoint": {"ipv4": endpoint_ip, "port": int(endpoint_port), "transport": "udp"},
        "txid": 0xBEEF,
        "deadline_ms": int(time.time() * 1000) + 4000,
        "max_wire": 512,
    }
    cmd = [
        "docker", "run", "-i", "--rm", "--network", net,
        "--read-only", "--cap-drop", "ALL", "--security-opt",
        "no-new-privileges:true", "--memory", "128m", "--memory-swap",
        "128m", "--cpus", "0.5", "--pids-limit", "32", "--ulimit",
        "core=0:0", "--ulimit", "nofile=64:64", "--log-driver", "none",
        "--user", "65532", image,
    ]
    t0 = time.monotonic()
    proc = subprocess.Popen(cmd, stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                            stderr=subprocess.PIPE)
    proc.stdin.write(frame(req))
    proc.stdin.flush()
    out = proc.stdout
    ready, _ = read_frame(out)
    if not ready:
        err = proc.stderr.read()[:240].decode("utf-8", "replace")
        code = proc.poll()
        proc.kill()
        return {"probe": "no_ready", "exit_code": code, "stderr": err,
                "elapsed_ms": int((time.monotonic() - t0) * 1000)}
    start = {"version": REQ, "phase": "start", "attempt_id": attempt}
    proc.stdin.write(frame(start))
    proc.stdin.flush()
    raw_bytes = 0
    # Raw channel: binary frames then sentinel; count payload bytes only.
    while True:
        hdr = out.read(4)
        if len(hdr) < 4:
            break
        n = struct.unpack(">I", hdr)[0]
        if n == 0:
            break
        out.read(n)
        raw_bytes += n
    code = proc.wait(timeout=8)
    return {
        "probe": "capture_direct",
        "destination": f"{endpoint_ip}:{endpoint_port}",
        "raw_bytes": raw_bytes,
        "exit_code": code,
        "elapsed_ms": int((time.monotonic() - t0) * 1000),
    }


def main() -> int:
    if len(sys.argv) != 7:
        sys.stderr.write("usage: probe_capture.py <image> <net> <ip> <port> <attempt> <session>\n")
        return 2
    print(json.dumps(run_probe(*sys.argv[1:5], sys.argv[5], sys.argv[6])))
    return 0


if __name__ == "__main__":
    sys.exit(main())
