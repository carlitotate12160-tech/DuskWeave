#!/usr/bin/env python3
"""Unit tests for HTTPS LAB deployment manifest rendering, atomic publish, and reconcile."""

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

import https_lab_binding  # noqa: E402


def sample_identities(ch: str = "a") -> dict:
    h = ch * 64
    return {
        "release_sha256": h,
        "launcher_sha256": h,
        "capture_sha256": h,
        "exporter_sha256": h,
        "image_digest": f"sha256:{h}",
        "boot_id": "11111111-1111-4111-8111-111111111111",
        "core_uid": 1001,
    }


class HttpsLabBindingTests(unittest.TestCase):
    def _base_env(self) -> dict[str, str]:
        return {"PATH": "/usr/bin:/bin", "HOME": "/root", "LANG": "C.UTF-8"}

    def test_render_lab_binding_emits_15_fields_and_matching_13_col_sql_row(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            out = Path(tmp) / "deployment.json"
            res = https_lab_binding.render_lab_binding(
                identities=sample_identities("a"),
                valid_from_ms=1000,
                valid_until_ms=4_000_000_000_000,
                now_ms=2000,
                output_file=out,
            )
            self.assertEqual(res["status"], "UNAPPLIED")
            self.assertFalse(res["applied"])
            raw = out.read_bytes()
            self.assertEqual(hashlib.sha256(raw).hexdigest(), res["manifest_sha256"])
            manifest = json.loads(raw.decode("utf-8"))
            self.assertEqual(set(manifest.keys()), https_lab_binding.MANIFEST_FIELDS)
            self.assertEqual(len(manifest), 15)
            self.assertEqual(manifest["purpose"], "HTTPS_LOOP_LAB")
            self.assertEqual(manifest["run_profile"], "autonomous-https-genai-lab-v1")
            self.assertEqual(manifest["resolver_ipv4"], "192.0.2.53")
            self.assertEqual(manifest["https_lab_root_version"], "duskweave-https-lab-root-v1")
            self.assertEqual(manifest["https_lab_root_sha256"], https_lab_binding.HTTPS_LAB_ROOT_SHA256)
            self.assertEqual(set(res["sql_row"].keys()), set(https_lab_binding.SQL_COLUMNS))
            self.assertEqual(res["sql_row"]["manifest_sha256"], res["manifest_sha256"])

    def test_refuses_live_purpose_wrong_resolver_wrong_root_or_ambient_secrets(self) -> None:
        bad_bundles = (
            {"purpose": "LIVE"},
            {"purpose": "LAB"},
            {"run_profile": "autonomous-dns-genai-live-v1"},
            {"resolver_ipv4": "1.1.1.1"},
            {"root_version": "wrong-root"},
            {"root_sha256": "b" * 64},
            {"valid_from_ms": 3000, "valid_until_ms": 4000, "now_ms": 2000},
        )
        for overrides in bad_bundles:
            kw = {
                "identities": sample_identities("b"),
                "valid_from_ms": 1000,
                "valid_until_ms": 4_000_000_000_000,
                "now_ms": 2000,
            }
            kw.update(overrides)
            with self.subTest(overrides=overrides), self.assertRaises(https_lab_binding.BindingError):
                https_lab_binding.build_lab_manifest_bundle(**kw)

        with self.assertRaises(https_lab_binding.BindingError):
            https_lab_binding.publish_lab_binding(
                identities=sample_identities("c"), installed_identities=sample_identities("c"),
                valid_from_ms=1000, valid_until_ms=4_000_000_000_000,
                manifest_path=Path("/tmp/unused"), intent_dir=Path("/tmp/unused-i"),
                sql_executor=lambda *_a: None, env={**self._base_env(), "DEEPSEEK_API_KEY": "secret"},
            )

    def test_publish_ack_loss_reconcile_and_tamper_guards(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            base = Path(tmp)
            os.chmod(base, 0o755)
            cfg = base / "cfg"
            cfg.mkdir(mode=0o755)
            os.chmod(cfg, 0o755)
            state = {"rows": {}, "writes": 0}

            def ack_loss(op: str, arg=None):
                if op == "exists":
                    return arg in state["rows"]
                if op == "insert":
                    state["writes"] += 1
                    state["rows"][arg["qualification_ref"]] = dict(arg)
                    raise TimeoutError("postgresql://admin:secret@127.0.0.1:5432/db")
                return state["rows"].get(arg)

            receipt = https_lab_binding.publish_lab_binding(
                identities=sample_identities("d"), installed_identities=sample_identities("d"),
                valid_from_ms=1000, valid_until_ms=4_000_000_000_000, now_ms=2000,
                manifest_path=cfg / "deployment.json", intent_dir=base / "intent",
                sql_executor=ack_loss, env=self._base_env(), expected_uids=(os.geteuid(),), stop_at=base,
            )
            self.assertEqual(receipt["status"], "UNKNOWN")
            self.assertNotIn("secret", receipt["reason"])
            self.assertNotIn("postgresql://", receipt["reason"])

            rec = https_lab_binding.reconcile_lab_binding(
                qualification_ref=receipt["qualification_ref"], manifest_path=cfg / "deployment.json",
                installed_identities=sample_identities("d"), sql_reader=lambda q: state["rows"].get(q),
                expected_uids=(os.geteuid(),), stop_at=base, clock_fn=lambda: 2000,
            )
            self.assertEqual(rec["status"], "SUCCESS")
            self.assertTrue(rec["row_manifest_consistent"])
            self.assertTrue(rec["current_qualified"])

            expired = https_lab_binding.reconcile_lab_binding(
                qualification_ref=receipt["qualification_ref"], manifest_path=cfg / "deployment.json",
                installed_identities=sample_identities("d"), sql_reader=lambda q: state["rows"].get(q),
                expected_uids=(os.geteuid(),), stop_at=base, clock_fn=lambda: 5_000_000_000_000,
            )
            self.assertEqual(expired["status"], "INCOMPLETE")
            self.assertFalse(expired["current_qualified"])

    def test_bind_cli_refuses_outside_fixed_source(self) -> None:
        proc = subprocess.run(
            [sys.executable, str(QUALIFY_DIR / "https_lab_binding.py"),
             "bind", "--expected-driver-commit", "a" * 40, "--expected-driver-tree", "b" * 40,
             "--expected-descriptor-sha256", "c" * 64, "--valid-from-ms", "1000", "--valid-until-ms", "2000"],
            capture_output=True, text=True, timeout=10,
        )
        self.assertEqual(proc.returncode, 1)
        self.assertEqual(json.loads(proc.stdout)["reason"], "privileged_driver_outside_fixed_source")


if __name__ == "__main__":
    unittest.main()
