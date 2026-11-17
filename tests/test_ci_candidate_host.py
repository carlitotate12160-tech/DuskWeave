#!/usr/bin/env python3
"""Unit tests for candidate_host account, sudoers, sinks, OCI image layer inspection, and quiescence."""

from __future__ import annotations

import hashlib
import io
import json
import os
import subprocess
import sys
import tarfile
import tempfile
import unittest
from pathlib import Path
from types import SimpleNamespace

QUALIFY_DIR = Path(__file__).resolve().parents[1] / "boundary" / "qualify"
if str(QUALIFY_DIR) not in sys.path:
    sys.path.insert(0, str(QUALIFY_DIR))

import candidate_host  # noqa: E402


def _make_layer(members: list[tuple[str, bytes | str]]) -> bytes:
    buf = io.BytesIO()
    with tarfile.open(fileobj=buf, mode="w") as tf:
        for name, content in members:
            if isinstance(content, str) and content.startswith("SYM:"):
                ti = tarfile.TarInfo(name)
                ti.type = tarfile.SYMTYPE
                ti.linkname = content[4:]
                tf.addfile(ti)
            else:
                raw = content.encode("utf-8") if isinstance(content, str) else content
                ti = tarfile.TarInfo(name)
                ti.size = len(raw)
                tf.addfile(ti, io.BytesIO(raw))
    return buf.getvalue()


def _make_save_tar(layer_specs: list[tuple[str, list[tuple[str, bytes | str]]]]) -> bytes:
    buf = io.BytesIO()
    layer_names = [n for n, _ in layer_specs]
    with tarfile.open(fileobj=buf, mode="w") as tf:
        manifest_bytes = json.dumps([{"Layers": layer_names}]).encode("utf-8")
        mi = tarfile.TarInfo("manifest.json")
        mi.size = len(manifest_bytes)
        tf.addfile(mi, io.BytesIO(manifest_bytes))
        for lname, members in layer_specs:
            lb = _make_layer(members)
            li = tarfile.TarInfo(lname)
            li.size = len(lb)
            tf.addfile(li, io.BytesIO(lb))
    return buf.getvalue()


def _cp(rc: int = 0, stdout: str | bytes = "", stderr: str = "") -> SimpleNamespace:
    return SimpleNamespace(returncode=rc, stdout=stdout, stderr=stderr)


class CandidateHostAccountAndSinksTests(unittest.TestCase):
    """Test core UID resolution, sudoers exact-match policy, and host sinks."""

    def test_resolve_core_account_checks_uid_and_forbidden_groups(self) -> None:
        uid = os.geteuid()
        with tempfile.TemporaryDirectory() as tmp:
            base = Path(tmp) / "host"
            base.mkdir(mode=0o755)
            os.chmod(base, 0o755)
            uid_file = base / ".core-account-uid"
            uid_file.write_text("996\n", encoding="utf-8")
            os.chmod(uid_file, 0o400)
            pw = SimpleNamespace(pw_name="duskweave-core", pw_uid=996, pw_gid=996)
            gr = SimpleNamespace(gr_name="duskweave-core", gr_mem=[])
            res = candidate_host.resolve_core_account(
                core_uid_path=uid_file, stop_at=base, expected_uids=(uid,),
                getpwuid_fn=lambda _u: pw, getpwnam_fn=lambda _n: pw,
                getgrgid_fn=lambda _g: gr, getgrall_fn=lambda: [gr],
            )
            self.assertEqual(res, {"name": "duskweave-core", "uid": 996, "gid": 996})

            docker_gr = SimpleNamespace(gr_name="docker", gr_mem=["duskweave-core"])
            with self.assertRaises(candidate_host.HostObservationError) as grp_err:
                candidate_host.resolve_core_account(
                    core_uid_path=uid_file, stop_at=base, expected_uids=(uid,),
                    getpwuid_fn=lambda _u: pw, getpwnam_fn=lambda _n: pw,
                    getgrgid_fn=lambda _g: gr, getgrall_fn=lambda: [gr, docker_gr],
                )
            self.assertEqual(grp_err.exception.reason, "core_account_forbidden_group")

            bad_uid = base / ".zero-uid"
            bad_uid.write_text("0\n", encoding="utf-8")
            os.chmod(bad_uid, 0o400)
            with self.assertRaises(candidate_host.HostObservationError) as z_err:
                candidate_host.resolve_core_account(core_uid_path=bad_uid, stop_at=base, expected_uids=(uid,))
            self.assertEqual(z_err.exception.reason, "core_account_uid_invalid")

    def test_verify_sudoers_requires_exact_policy_bytes_and_visudo(self) -> None:
        uid = os.geteuid()
        with tempfile.TemporaryDirectory() as tmp:
            base = Path(tmp) / "sudo"
            base.mkdir(mode=0o755)
            os.chmod(base, 0o755)
            sp = base / "duskweave-m1-launcher"
            sp.write_text(candidate_host.EXPECTED_SUDOERS, encoding="utf-8")
            os.chmod(sp, 0o440)
            candidate_host.verify_sudoers(
                sudoers_path=sp, stop_at=base, expected_uids=(uid,), cmd_runner=lambda *a, **k: _cp(0),
            )
            os.chmod(sp, 0o600)
            sp.write_text("duskweave-core ALL=(ALL) NOPASSWD: ALL\n", encoding="utf-8")
            os.chmod(sp, 0o440)
            with self.assertRaises(candidate_host.HostObservationError) as mis:
                candidate_host.verify_sudoers(
                    sudoers_path=sp, stop_at=base, expected_uids=(uid,), cmd_runner=lambda *a, **k: _cp(0),
                )
            self.assertEqual(mis.exception.reason, "sudoers_policy_mismatch")

    def test_observe_host_sinks_checks_swap_core_pattern_suid_and_owner(self) -> None:
        uid = os.geteuid()
        with tempfile.TemporaryDirectory() as tmp:
            base = Path(tmp) / "sinks"
            proc = base / "proc"
            (proc / "sys/kernel/random").mkdir(parents=True, mode=0o755)
            (proc / "sys/fs").mkdir(parents=True, mode=0o755)
            (proc / "sys/kernel/random/boot_id").write_text("11111111-1111-4111-8111-111111111111\n", encoding="utf-8")
            (proc / "swaps").write_text("Filename\t\tType\t\tSize\n", encoding="utf-8")
            (proc / "sys/kernel/core_pattern").write_text("|/bin/false\n", encoding="utf-8")
            (proc / "sys/fs/suid_dumpable").write_text("0\n", encoding="utf-8")
            owner = base / ".live-dns-owner"
            owner.write_text(candidate_host.EXPECTED_LIVE_DNS_OWNER + "\n", encoding="utf-8")
            os.chmod(owner, 0o400)
            ctrl, lnch = base / "m1", base / "launcher"
            ctrl.mkdir(mode=0o700)
            lnch.mkdir(mode=0o700)
            for d in (base, proc, proc / "sys", proc / "sys/kernel", proc / "sys/kernel/random", proc / "sys/fs"):
                os.chmod(d, 0o755)
            os.chmod(ctrl, 0o700)
            os.chmod(lnch, 0o700)

            boot = candidate_host.observe_host_sinks(
                uid, proc_root=proc, live_dns_owner_path=owner, control_root=ctrl,
                launcher_root=lnch, stop_at=base, expected_uids=(uid,),
            )
            self.assertEqual(boot, "11111111-1111-4111-8111-111111111111")

            (proc / "swaps").write_text("Filename\n/swapfile partition\n", encoding="utf-8")
            with self.assertRaises(candidate_host.HostObservationError) as sw_err:
                candidate_host.observe_host_sinks(
                    uid, proc_root=proc, live_dns_owner_path=owner, control_root=ctrl, stop_at=base, expected_uids=(uid,),
                )
            self.assertEqual(sw_err.exception.reason, "active_swap_detected")


class CandidateHostImageAndQuiescenceTests(unittest.TestCase):
    """Test OCI layer worker extraction safety and multi-signal quiescence checks."""

    def test_extract_image_worker_digests_valid_and_malicious_layers(self) -> None:
        img_id = "sha256:" + ("a" * 64)
        cap_b, exp_b = b"CAP_ELF", b"EXP_ELF"
        meta = json.dumps({"Id": img_id, "Os": "linux", "Architecture": "arm64", "RootFS": {"Type": "layers", "Layers": ["sha256:1"]}})

        def make_runner(save_bytes: bytes, arch: str = "arm64"):
            m = json.dumps({"Id": img_id, "Os": "linux", "Architecture": arch, "RootFS": {"Type": "layers", "Layers": ["sha256:1"]}})
            def _run(cmd, **_kw):
                return _cp(0, stdout=m) if cmd[2] == "inspect" else _cp(0, stdout=save_bytes)
            return _run

        def _make_dev_layer() -> bytes:
            buf = io.BytesIO()
            with tarfile.open(fileobj=buf, mode="w") as tf:
                ti = tarfile.TarInfo("dw-dns-capture")
                ti.type, ti.devmajor, ti.devminor = tarfile.CHRTYPE, 1, 3
                tf.addfile(ti)
            return _wrap_layers([buf.getvalue()])

        def _wrap_layers(layers: list[bytes]) -> bytes:
            buf = io.BytesIO()
            with tarfile.open(fileobj=buf, mode="w") as tf:
                mb = json.dumps([{"Layers": [f"l{i}.tar" for i in range(len(layers))]}]).encode("utf-8")
                mi = tarfile.TarInfo("manifest.json")
                mi.size = len(mb)
                tf.addfile(mi, io.BytesIO(mb))
                for i, lb in enumerate(layers):
                    li = tarfile.TarInfo(f"l{i}.tar")
                    li.size = len(lb)
                    tf.addfile(li, io.BytesIO(lb))
            return buf.getvalue()

        for good_members in (
            [("dw-dns-capture", cap_b), ("dw-dns-exporter", exp_b)],
            [("./dw-dns-capture", cap_b), ("./dw-dns-exporter", exp_b)],
            [("./dw-dns-capture", cap_b), ("dw-dns-exporter", exp_b)],
        ):
            good_tar = _make_save_tar([("l0.tar", good_members)])
            got = candidate_host.extract_image_worker_digests("dw:test", docker_runner=make_runner(good_tar))
            self.assertEqual(got["workers"]["dw-dns-capture"], hashlib.sha256(cap_b).hexdigest())
            self.assertEqual(got["workers"]["dw-dns-exporter"], hashlib.sha256(exp_b).hexdigest())

        for label, bad_members, expected_prefix in (
            ("whiteout", [(".wh.dw-dns-capture", b""), ("dw-dns-exporter", exp_b)], "image_layer_unsafe_member"),
            ("symlink", [("dw-dns-capture", "SYM:/etc/passwd"), ("dw-dns-exporter", exp_b)], "image_layer_link_or_device_refused"),
            ("extra", [("dw-dns-capture", cap_b), ("dw-dns-exporter", exp_b), ("extra", b"x")], "image_unexpected_worker_member:extra"),
            ("hidden_dot", [(".dw-dns-capture", cap_b), ("dw-dns-exporter", exp_b)], "image_unexpected_worker_member:.dw-dns-capture"),
            ("dot_slash_hidden", [("./.dw-dns-capture", cap_b), ("dw-dns-exporter", exp_b)], "image_unexpected_worker_member:.dw-dns-capture"),
            ("traversal", [("../dw-dns-capture", cap_b), ("dw-dns-exporter", exp_b)], "image_layer_unsafe_member"),
            ("dot_slash_traversal", [("./../dw-dns-capture", cap_b), ("dw-dns-exporter", exp_b)], "image_layer_unsafe_member"),
            ("absolute", [("/dw-dns-capture", cap_b), ("dw-dns-exporter", exp_b)], "image_layer_unsafe_member"),
            ("double_slash", [(".//dw-dns-capture", cap_b), ("dw-dns-exporter", exp_b)], "image_layer_unsafe_member"),
            ("subdir", [("sub/dw-dns-capture", cap_b), ("dw-dns-exporter", exp_b)], "image_unexpected_worker_member:sub/dw-dns-capture"),
        ):
            bad_tar = _make_save_tar([("l0.tar", bad_members)])
            with self.subTest(case=label):
                with self.assertRaises(candidate_host.HostObservationError) as err:
                    candidate_host.extract_image_worker_digests("dw:test", docker_runner=make_runner(bad_tar))
                self.assertEqual(err.exception.status, "REFUSED")
                self.assertEqual(err.exception.reason, expected_prefix)

        with self.assertRaises(candidate_host.HostObservationError) as dev_err:
            candidate_host.extract_image_worker_digests("dw:test", docker_runner=make_runner(_make_dev_layer()))
        self.assertEqual(dev_err.exception.status, "REFUSED")
        self.assertEqual(dev_err.exception.reason, "image_layer_link_or_device_refused")

        with self.assertRaises(candidate_host.HostObservationError) as arch_err:
            candidate_host.extract_image_worker_digests("dw:test", docker_runner=make_runner(good_tar, arch="amd64"))
        self.assertEqual(arch_err.exception.reason, "image_platform_mismatch")

        with self.assertRaises(candidate_host.HostObservationError) as cor_err:
            candidate_host.extract_image_worker_digests("dw:test", docker_runner=make_runner(b"not-a-tar"))
        self.assertEqual(cor_err.exception.status, "UNKNOWN")
        self.assertEqual(meta[:1], "{")

    def test_check_quiescence_catches_active_procs_containers_netns_launcher_and_dbs(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            base = Path(tmp)
            netns, lnch = base / "netns", base / "launcher"
            netns.mkdir()
            lnch.mkdir()

            def runner_fn(pg_rc: int = 1, ps_out: str = "dw-live-dns-20261030-pg\n"):
                return lambda cmd, **_k: _cp(pg_rc) if cmd[0] == "pgrep" else _cp(0, stdout=ps_out)

            ok = candidate_host.check_quiescence(
                candidate_db="dw_cand", netns_dir=netns, launcher_root=lnch,
                cmd_runner=runner_fn(), sql_runner=lambda _d, _s: "0",
            )
            self.assertTrue(ok["quiescent"])
            self.assertEqual(ok["checked_dbs"][-1], "dw_cand")

            with self.assertRaises(candidate_host.HostObservationError) as pg0:
                candidate_host.check_quiescence(netns_dir=netns, launcher_root=lnch, cmd_runner=runner_fn(pg_rc=0), sql_runner=lambda _d, _s: "0")
            self.assertEqual(pg0.exception.reason, "active_runtime_processes_present")

            with self.assertRaises(candidate_host.HostObservationError) as pg2:
                candidate_host.check_quiescence(netns_dir=netns, launcher_root=lnch, cmd_runner=runner_fn(pg_rc=2), sql_runner=lambda _d, _s: "0")
            self.assertEqual(pg2.exception.status, "UNKNOWN")

            with self.assertRaises(candidate_host.HostObservationError) as cnt_err:
                candidate_host.check_quiescence(netns_dir=netns, launcher_root=lnch, cmd_runner=runner_fn(ps_out="dwcap-1\n"), sql_runner=lambda _d, _s: "0")
            self.assertEqual(cnt_err.exception.reason, "active_worker_container:dwcap-1")

            (netns / "dw123").mkdir()
            with self.assertRaises(candidate_host.HostObservationError) as ns_err:
                candidate_host.check_quiescence(netns_dir=netns, launcher_root=lnch, cmd_runner=runner_fn(), sql_runner=lambda _d, _s: "0")
            self.assertEqual(ns_err.exception.reason, "active_worker_netns:dw123")
            (netns / "dw123").rmdir()

            pend = lnch / "attempt1.pending"
            pend.write_text("p", encoding="utf-8")
            with self.assertRaises(candidate_host.HostObservationError) as ln_err:
                candidate_host.check_quiescence(netns_dir=netns, launcher_root=lnch, cmd_runner=runner_fn(), sql_runner=lambda _d, _s: "0")
            self.assertEqual(ln_err.exception.reason, "pending_launcher_marker:attempt1.pending")
            pend.unlink()

            db_map = {db: "0" for db in candidate_host.RETAINED_QUIESCENCE_DBS}
            db_map[candidate_host.RETAINED_QUIESCENCE_DBS[0]] = "-1"
            with self.assertRaises(candidate_host.HostObservationError) as cat_err:
                candidate_host.check_quiescence(netns_dir=netns, launcher_root=lnch, cmd_runner=runner_fn(), sql_runner=lambda db, _s: db_map[db])
            self.assertEqual(cat_err.exception.status, "UNKNOWN")

            db_map[candidate_host.RETAINED_QUIESCENCE_DBS[0]] = "1"
            with self.assertRaises(candidate_host.HostObservationError) as act_err:
                candidate_host.check_quiescence(netns_dir=netns, launcher_root=lnch, cmd_runner=runner_fn(), sql_runner=lambda db, _s: db_map[db])
            self.assertEqual(act_err.exception.reason, f"active_db_runs_present:{candidate_host.RETAINED_QUIESCENCE_DBS[0]}")


if __name__ == "__main__":
    unittest.main()
