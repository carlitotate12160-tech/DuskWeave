#!/usr/bin/env python3
"""Unit tests for HTTPS LAB OCI image/PEM inspection, 0038 schema verification, and shell guards."""

from __future__ import annotations

import hashlib
import json
import os
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
QUALIFY_DIR = ROOT / "boundary" / "qualify"
if str(QUALIFY_DIR) not in sys.path:
    sys.path.insert(0, str(QUALIFY_DIR))

import https_lab_host  # noqa: E402
from test_ci_candidate_host import _cp, _make_save_tar  # noqa: E402


class HttpsLabHostAndShellTests(unittest.TestCase):
    def test_extract_https_lab_image_digests_verifies_workers_and_root_pem(self) -> None:
        img_id = "sha256:" + ("a" * 64)
        cap_b, exp_b = b"CAP_ELF", b"EXP_ELF"
        pem_b = (ROOT / "boundary" / "fixtures" / "https-lab-root-v1.pem").read_bytes()
        meta = json.dumps({
            "Id": img_id, "Os": "linux", "Architecture": "arm64",
            "RootFS": {"Type": "layers", "Layers": ["sha256:1"]},
        })

        def runner(save_bytes: bytes):
            def _run(cmd, **_kw):
                return _cp(0, stdout=meta) if cmd[2] == "inspect" else _cp(0, stdout=save_bytes)
            return _run

        good_tar = _make_save_tar([("l0.tar", [
            ("dw-dns-capture", cap_b),
            ("dw-dns-exporter", exp_b),
            ("etc/duskweave/https-lab-root-v1.pem", pem_b),
        ])])
        got = https_lab_host.extract_https_lab_image_digests("dw-https-lab:r1", docker_runner=runner(good_tar))
        self.assertEqual(got["workers"]["dw-dns-capture"], hashlib.sha256(cap_b).hexdigest())
        self.assertEqual(got["workers"]["dw-dns-exporter"], hashlib.sha256(exp_b).hexdigest())
        self.assertEqual(got["extras"][https_lab_host.ROOT_CA_IMAGE_PATH], https_lab_host.ROOT_CA_SHA256)

        bad_pem_tar = _make_save_tar([("l0.tar", [
            ("dw-dns-capture", cap_b),
            ("dw-dns-exporter", exp_b),
            ("etc/duskweave/https-lab-root-v1.pem", b"wrong-pem"),
        ])])
        with self.assertRaisesRegex(https_lab_host.HostObservationError, "^image_extra_member_mismatch:"):
            https_lab_host.extract_https_lab_image_digests("dw-https-lab:r1", docker_runner=runner(bad_pem_tar))

        missing_pem_tar = _make_save_tar([("l0.tar", [("dw-dns-capture", cap_b), ("dw-dns-exporter", exp_b)])])
        with self.assertRaisesRegex(https_lab_host.HostObservationError, "^image_workers_incomplete$"):
            https_lab_host.extract_https_lab_image_digests("dw-https-lab:r1", docker_runner=runner(missing_pem_tar))

    def test_observe_installed_root_pem_and_0038_spec_extractor(self) -> None:
        uid = os.geteuid()
        pem_b = (ROOT / "boundary" / "fixtures" / "https-lab-root-v1.pem").read_bytes()
        with tempfile.TemporaryDirectory() as tmp:
            base = Path(tmp) / "etc"
            base.mkdir(mode=0o755)
            os.chmod(base, 0o755)
            pem = base / "https-lab-root-v1.pem"
            pem.write_bytes(pem_b)
            os.chmod(pem, 0o444)
            digest = https_lab_host.observe_installed_root_pem(pem, expected_uids=(uid,), stop_at=base)
            self.assertEqual(digest, https_lab_host.ROOT_CA_SHA256)

            os.chmod(pem, 0o644)
            pem.write_bytes(b"tampered")
            os.chmod(pem, 0o444)
            with self.assertRaisesRegex(https_lab_host.HostObservationError, "^https_lab_root_pem_mismatch$"):
                https_lab_host.observe_installed_root_pem(pem, expected_uids=(uid,), stop_at=base)

        specs = https_lab_host.extract_0038_function_specs(ROOT / "migrations" / "0038_https_lab.sql")
        self.assertEqual(set(specs.keys()), set(https_lab_host.REQUIRED_0038_FUNCTIONS))
        invoker_fns = {"execution.inference_profile_matches", "execution.assert_successor_pack"}
        for name, spec in specs.items():
            self.assertEqual(spec["security_definer"], name not in invoker_fns, name)
            self.assertEqual(spec["search_path"], "search_path=pg_catalog, pg_temp", name)

    def test_https_lab_sh_is_sourceable_and_refuses_off_host_privileged_subcommands(self) -> None:
        clean_env = {"PATH": os.environ.get("PATH", "/usr/bin:/bin"), "HOME": "/tmp"}
        res = subprocess.run(
            ["bash", "-c", f'set -euo pipefail; source "{QUALIFY_DIR / "https_lab.sh"}"; declare -F https_lab_main >/dev/null'],
            capture_output=True, text=True, env=clean_env, timeout=15,
        )
        self.assertEqual(res.returncode, 0, res.stderr)
        self.assertEqual(res.stdout, "")

        for args in ("", "bogus", "build-storage --expected-driver-commit " + "a" * 40):
            proc = subprocess.run(
                ["bash", str(QUALIFY_DIR / "https_lab.sh"), *([a for a in args.split() if a])],
                capture_output=True, text=True, env=clean_env, timeout=15,
            )
            self.assertNotEqual(proc.returncode, 0)


if __name__ == "__main__":
    unittest.main()
