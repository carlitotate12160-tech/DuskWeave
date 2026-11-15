"""Contract tests for candidate LIVE binding manifest rendering, ordered publication, and reconcile."""

import hashlib
import json
import os
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path
from unittest import mock

ROOT = Path(__file__).resolve().parents[1]
QUALIFY_DIR = ROOT / "boundary" / "qualify"
sys.path.insert(0, str(QUALIFY_DIR))

import candidate_binding  # noqa: E402


def sample_identities(digest_char: str = "a") -> dict:
    h = digest_char * 64
    return {
        "release_sha256": h,
        "launcher_sha256": h,
        "capture_sha256": h,
        "exporter_sha256": h,
        "image_digest": f"sha256:{h}",
        "boot_id": "11111111-1111-4111-8111-111111111111",
        "core_uid": 1001,
    }


class CandidateBindingRenderTests(unittest.TestCase):
    def test_render_binding_emits_unapplied_evidence_and_exact_hash(self):
        with tempfile.TemporaryDirectory() as tmp:
            out_path = Path(tmp) / "rendered-manifest.json"
            res = candidate_binding.render_binding(
                identities=sample_identities("a"),
                resolver_ipv4="1.1.1.1",
                valid_from_ms=1000,
                valid_until_ms=4_000_000_000_000,
                now_ms=2000,
                output_file=out_path,
            )
            self.assertEqual(res["status"], "UNAPPLIED")
            self.assertFalse(res["applied"])
            raw = out_path.read_bytes()
            self.assertLessEqual(len(raw), 16384)
            self.assertEqual(hashlib.sha256(raw).hexdigest(), res["manifest_sha256"])
            manifest = json.loads(raw.decode("utf-8"))
            self.assertEqual(manifest["purpose"], "LIVE")
            self.assertEqual(manifest["run_profile"], "autonomous-dns-genai-live-v1")
            self.assertEqual(manifest["resolver_ipv4"], "1.1.1.1")
            self.assertEqual(res["sql_row"]["manifest_sha256"], res["manifest_sha256"])
            self.assertEqual(res["sql_row"]["qualification_ref"], manifest["qualification_ref"])

    def test_refuses_non_global_ipv4_profile_mismatch_window_and_bad_identities(self):
        bad_cases = (
            {"resolver_ipv4": "192.0.2.53"},
            {"resolver_ipv4": "192.88.99.1"},
            {"resolver_ipv4": "10.0.0.1"},
            {"resolver_ipv4": "127.0.0.1"},
            {"resolver_ipv4": "100.64.0.1"},
            {"resolver_ipv4": "1.1.1.1/32"},
            {"purpose": "LAB"},
            {"run_profile": "autonomous-dns-genai-lab-v1"},
            {"valid_from_ms": 5000, "valid_until_ms": 4000},
            {"valid_from_ms": 1000, "valid_until_ms": 1500, "now_ms": 2000},
            {"valid_from_ms": 0, "valid_until_ms": 0},
        )
        for overrides in bad_cases:
            kwargs = {
                "identities": sample_identities("b"),
                "resolver_ipv4": "1.1.1.1",
                "valid_from_ms": 1000,
                "valid_until_ms": 4_000_000_000_000,
                "now_ms": 2000,
            }
            kwargs.update(overrides)
            with self.subTest(overrides=overrides), self.assertRaises(candidate_binding.BindingError):
                candidate_binding.render_binding(**kwargs)

        for ok_ip in ("192.88.98.255", "192.88.100.0"):
            self.assertEqual(candidate_binding.validate_global_ipv4(ok_ip), ok_ip)

        bad_ids = sample_identities("b")
        bad_ids["core_uid"] = 0
        with self.assertRaises(candidate_binding.BindingError):
            candidate_binding.render_binding(
                identities=bad_ids, resolver_ipv4="1.1.1.1",
                valid_from_ms=1000, valid_until_ms=4_000_000_000_000, now_ms=2000,
            )

    def test_bind_cli_rejects_now_ms_and_boot_id_and_enforces_fixed_source(self):
        for flag in ("--now-ms", "--boot-id", "--core-uid"):
            res = subprocess.run(
                [sys.executable, str(QUALIFY_DIR / "candidate_binding.py"), "bind", flag, "1"],
                capture_output=True, text=True, timeout=10,
            )
            self.assertNotEqual(res.returncode, 0)
        rc = candidate_binding.main([
            "bind", "--expected-driver-commit", "a" * 40, "--expected-driver-tree", "b" * 40,
            "--expected-descriptor-sha256", "c" * 64, "--resolver-ipv4", "1.1.1.1",
            "--purpose", "LIVE", "--run-profile", "autonomous-dns-genai-live-v1",
            "--valid-from-ms", "1000", "--valid-until-ms", "4000000000000",
        ])
        self.assertEqual(rc, 1)


class OrderedPublicationAndReconcileTests(unittest.TestCase):
    def _base_env(self):
        return {"PATH": "/usr/bin:/bin", "HOME": "/root", "LANG": "C.UTF-8"}

    def _run_publish(self, base: Path, sql_exec, installed_check=None, replace_hook=os.replace, **extra):
        os.chmod(base, 0o755)
        cfg = base / "cfg"
        cfg.mkdir(mode=0o755, exist_ok=True)
        os.chmod(cfg, 0o755)
        return candidate_binding.publish_binding(
            identities=sample_identities("c"),
            installed_identities=sample_identities("c") if installed_check is None else installed_check,
            resolver_ipv4="9.9.9.9", valid_from_ms=1000, valid_until_ms=4_000_000_000_000, now_ms=2000,
            manifest_path=cfg / "deployment.json", intent_dir=base / "intent", sql_executor=sql_exec,
            env=self._base_env(), expected_uids=(os.geteuid(),), stop_at=base, replacer=replace_hook, **extra,
        )

    def test_manifest_publish_failure_executes_zero_sql(self):
        with tempfile.TemporaryDirectory() as tmp:
            base = Path(tmp)
            os.chmod(base, 0o755)
            sql_calls = []

            def fail_replace(_src, _dst):
                raise OSError("manifest_replace_failed")

            res = self._run_publish(base, lambda q, params=None: sql_calls.append(q) and False, replace_hook=fail_replace)
            self.assertEqual(res["status"], "INCOMPLETE")
            self.assertNotIn("insert", sql_calls)

    def test_sql_ack_loss_returns_unknown_and_readonly_reconcile_confirms_same_qref(self):
        with tempfile.TemporaryDirectory() as tmp:
            base = Path(tmp)
            os.chmod(base, 0o755)
            state = {"rows": {}, "writes": 0, "reads": 0}

            def sql_ack_loss(op: str, row: dict | str | None = None):
                if op == "exists":
                    return row in state["rows"]
                if op == "insert":
                    assert isinstance(row, dict)
                    state["writes"] += 1
                    state["rows"][row["qualification_ref"]] = dict(row)
                    raise TimeoutError("lost_commit_ack")
                if op == "read":
                    state["reads"] += 1
                    return state["rows"].get(row)
                raise AssertionError(f"unexpected op:{op}")

            receipt = self._run_publish(base, sql_ack_loss)
            self.assertEqual(receipt["status"], "UNKNOWN")
            self.assertEqual(state["writes"], 1)
            qref = receipt["qualification_ref"]

            reconciled = candidate_binding.reconcile_binding(
                qualification_ref=qref, manifest_path=base / "cfg" / "deployment.json",
                installed_identities=sample_identities("c"), sql_reader=lambda q: sql_ack_loss("read", q),
                expected_uids=(os.geteuid(),), stop_at=base, clock_fn=lambda: 2000,
            )
            self.assertEqual(reconciled["status"], "SUCCESS")
            self.assertTrue(reconciled["row_manifest_consistent"])
            self.assertTrue(reconciled["current_qualified"])

            expired = candidate_binding.reconcile_binding(
                qualification_ref=qref, manifest_path=base / "cfg" / "deployment.json",
                installed_identities=sample_identities("c"), sql_reader=lambda q: sql_ack_loss("read", q),
                expected_uids=(os.geteuid(),), stop_at=base, clock_fn=lambda: 4_000_000_000_000,
            )
            self.assertEqual(expired["status"], "INCOMPLETE")
            self.assertTrue(expired["row_manifest_consistent"])
            self.assertFalse(expired["current_qualified"])
            self.assertEqual(expired["reason"], "binding_window_expired_or_future")

            replaced = candidate_binding.reconcile_binding(
                qualification_ref=qref, manifest_path=base / "cfg" / "deployment.json",
                installed_identities=sample_identities("f"), sql_reader=lambda q: sql_ack_loss("read", q),
                expected_uids=(os.geteuid(),), stop_at=base, clock_fn=lambda: 2000,
            )
            self.assertEqual(replaced["status"], "INCOMPLETE")
            self.assertTrue(replaced["row_manifest_consistent"])
            self.assertFalse(replaced["current_qualified"])
            self.assertEqual(replaced["reason"], "installed_identity_replaced")

    def test_missing_sql_row_or_readback_mismatch_returns_incomplete(self):
        with tempfile.TemporaryDirectory() as tmp:
            base = Path(tmp)
            os.chmod(base, 0o755)
            res = self._run_publish(base, lambda op, _r=None: False if op == "exists" else (_ for _ in ()).throw(RuntimeError("tx_rolled_back")))
            self.assertEqual(res["status"], "UNKNOWN")
            rec = candidate_binding.reconcile_binding(
                qualification_ref=res["qualification_ref"], manifest_path=base / "cfg" / "deployment.json",
                installed_identities=sample_identities("c"), sql_reader=lambda _q: None,
                expected_uids=(os.geteuid(),), stop_at=base, clock_fn=lambda: 2000,
            )
            self.assertEqual(rec["status"], "INCOMPLETE")
            self.assertEqual(rec["reason"], "sql_binding_unissued")

            bad_row = {**sample_identities("d"), "purpose": "LIVE", "run_profile": "autonomous-dns-genai-live-v1",
                       "resolver_ipv4": "9.9.9.9", "valid_from_ms": 1000, "valid_until_ms": 4_000_000_000_000, "manifest_sha256": "d" * 64}
            base2 = base / "b2"
            base2.mkdir()
            res2 = self._run_publish(base2, lambda op, row=None: False if op == "exists" else ({**bad_row, "qualification_ref": row} if op == "read" else None))
            self.assertEqual(res2["status"], "INCOMPLETE")
            self.assertEqual(res2["reason"], "sql_readback_mismatch")

    def test_receipt_persist_failure_forces_unknown_never_success(self):
        with tempfile.TemporaryDirectory() as tmp:
            base = Path(tmp)
            os.chmod(base, 0o755)
            rows = {}

            def good_sql(op: str, row=None):
                if op == "exists":
                    return row in rows
                if op == "insert":
                    rows[row["qualification_ref"]] = dict(row)
                    return None
                return rows.get(row)

            def fail_rcpt_only(s: Path, d: Path):
                if "receipt-" in d.name:
                    raise OSError("receipt_write_err")
                os.replace(s, d)

            res = self._run_publish(base, good_sql, replace_hook=fail_rcpt_only)
            self.assertEqual(res["status"], "UNKNOWN")
            self.assertIn("receipt_persist_failed", res["reason"])
