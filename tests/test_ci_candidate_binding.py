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

        bad_ids = sample_identities("b")
        bad_ids["core_uid"] = 0
        with self.assertRaises(candidate_binding.BindingError):
            candidate_binding.render_binding(
                identities=bad_ids, resolver_ipv4="1.1.1.1",
                valid_from_ms=1000, valid_until_ms=4_000_000_000_000, now_ms=2000,
            )


class OrderedPublicationAndReconcileTests(unittest.TestCase):
    def _base_env(self):
        return {"PATH": "/usr/bin:/bin", "HOME": "/root", "LANG": "C.UTF-8"}

    def _run_publish(self, base: Path, sql_exec, installed_check=None, replace_hook=None):
        cfg = base / "cfg"
        cfg.mkdir(mode=0o755, exist_ok=True)
        intent_dir = base / "intent"
        intent_dir.mkdir(mode=0o755, exist_ok=True)
        manifest_path = cfg / "deployment.json"
        return candidate_binding.publish_binding(
            identities=sample_identities("c"),
            installed_identities=sample_identities("c") if installed_check is None else installed_check,
            resolver_ipv4="9.9.9.9",
            valid_from_ms=1000,
            valid_until_ms=4_000_000_000_000,
            now_ms=2000,
            manifest_path=manifest_path,
            intent_dir=intent_dir,
            sql_executor=sql_exec,
            env=self._base_env(),
            replacer=replace_hook,
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
                qualification_ref=qref,
                manifest_path=base / "cfg" / "deployment.json",
                installed_identities=sample_identities("c"),
                sql_reader=lambda q: sql_ack_loss("read", q),
            )
            self.assertEqual(reconciled["status"], "SUCCESS")
            self.assertEqual(reconciled["qualification_ref"], qref)
            self.assertEqual(state["writes"], 1)
            self.assertGreaterEqual(state["reads"], 1)

    def test_missing_sql_row_or_readback_mismatch_returns_incomplete(self):
        with tempfile.TemporaryDirectory() as tmp:
            base = Path(tmp)
            os.chmod(base, 0o755)

            def sql_unissued(op: str, row=None):
                if op == "exists":
                    return False
                if op == "insert":
                    raise RuntimeError("tx_rolled_back")
                return None

            res = self._run_publish(base, sql_unissued)
            self.assertEqual(res["status"], "INCOMPLETE")
            rec = candidate_binding.reconcile_binding(
                qualification_ref=res["qualification_ref"],
                manifest_path=base / "cfg" / "deployment.json",
                installed_identities=sample_identities("c"),
                sql_reader=lambda _q: None,
            )
            self.assertEqual(rec["status"], "INCOMPLETE")
            self.assertEqual(rec["reason"], "sql_binding_unissued")

            def sql_mismatch(op: str, row=None):
                if op == "exists":
                    return False
                if op == "insert":
                    return None
                if op == "read":
                    bad = sample_identities("d")
                    bad["qualification_ref"] = row
                    bad["purpose"] = "LIVE"
                    bad["run_profile"] = "autonomous-dns-genai-live-v1"
                    bad["resolver_ipv4"] = "9.9.9.9"
                    bad["valid_from_ms"] = 1000
                    bad["valid_until_ms"] = 4_000_000_000_000
                    bad["manifest_sha256"] = "d" * 64
                    return bad
                return None

            res2 = self._run_publish(base, sql_mismatch)
            self.assertEqual(res2["status"], "INCOMPLETE")
            self.assertEqual(res2["reason"], "sql_readback_mismatch")
