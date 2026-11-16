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
TESTS_DIR = ROOT / "tests"
sys.path.insert(0, str(QUALIFY_DIR))
sys.path.insert(0, str(TESTS_DIR))

import candidate_binding  # noqa: E402
import m1_candidate_qualification as m1_qual  # noqa: E402


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
        base_bind = [
            "--expected-driver-commit", "a" * 40, "--expected-driver-tree", "b" * 40,
            "--expected-descriptor-sha256", "c" * 64, "--resolver-ipv4", "1.1.1.1",
            "--purpose", "LIVE", "--run-profile", "autonomous-dns-genai-live-v1",
            "--valid-from-ms", "1000", "--valid-until-ms", "4000000000000",
        ]
        for flag, val in (
            ("--now-ms", "2000"),
            ("--boot-id", "11111111-1111-4111-8111-111111111111"),
            ("--core-uid", "1001"),
        ):
            res = subprocess.run(
                [sys.executable, str(QUALIFY_DIR / "candidate_binding.py"), "bind", *base_bind, flag, val],
                capture_output=True, text=True, timeout=10,
            )
            self.assertEqual(res.returncode, 2)
            self.assertIn(f"unrecognized arguments: {flag}", res.stderr)
        rc = candidate_binding.main(["bind", *base_bind])
        self.assertEqual(rc, 1)

    def test_verify_refusals_rejects_success_single_attempt_and_propagates_inspect_profile(self):
        with tempfile.TemporaryDirectory() as tmp:
            base = Path(tmp)
            mp = base / "deployment.json"
            mp.write_text("{}\n", encoding="utf-8")
            with mock.patch.object(m1_qual.candidate_binding, "pg_sql_executor", return_value=lambda *_a: None):
                with mock.patch.object(m1_qual.subprocess, "run", return_value=mock.Mock(returncode=1)):
                    with mock.patch.object(
                        m1_qual.candidate_binding, "publish_binding",
                        side_effect=[{"status": "SUCCESS", "reason": ""}, {"status": "REFUSED", "reason": "qualification_ref_collision"}],
                    ) as pub_mock:
                        with self.assertRaises(AssertionError):
                            m1_qual._verify_refusals("dwpg", "db", "dsn", mp, "q", base)
                        self.assertEqual(pub_mock.call_count, 1)
                    with mock.patch.object(
                        m1_qual.candidate_binding, "publish_binding",
                        return_value={"status": "REFUSED", "reason": "qualification_ref_collision"},
                    ) as ok_pub:
                        app, sql = m1_qual._verify_refusals("dwpg", "db", "dsn", mp, "q", base)
                        self.assertEqual(ok_pub.call_count, 1)
                        self.assertEqual(app, ["duplicate_qualification_ref_precheck_refused"])
                        self.assertEqual(len(sql), 2)
                with mock.patch.object(
                    m1_qual.candidate_binding, "publish_binding",
                    return_value={"status": "REFUSED", "reason": "qualification_ref_collision"},
                ), mock.patch.object(m1_qual.subprocess, "run", return_value=mock.Mock(returncode=0)):
                    with self.assertRaises(AssertionError):
                        m1_qual._verify_refusals("dwpg", "db", "dsn", mp, "q", base)
            cfg = base / "cfg"
            cfg.mkdir()
            (cfg / "environment.json").write_text('{"DW_DATABASE_CONFIG_MODE":"file"}', encoding="utf-8")
            with mock.patch.dict(os.environ, {"LLVM_PROFILE_FILE": "/tmp/w0-%p.profraw"}, clear=False):
                self.assertNotIn("LLVM_PROFILE_FILE", m1_qual._clean_env())
                with mock.patch.object(m1_qual.subprocess, "run", return_value=mock.Mock(returncode=0, stdout="inspect result=empty\n")) as ins_mock:
                    self.assertEqual(m1_qual._verify_core_inspect(base / "duskweave", cfg), "inspect result=empty")
                self.assertEqual(ins_mock.call_args.kwargs["env"]["LLVM_PROFILE_FILE"], "/tmp/w0-%p.profraw")


class OrderedPublicationAndReconcileTests(unittest.TestCase):
    def _base_env(self):
        return {"PATH": "/usr/bin:/bin", "HOME": "/root", "LANG": "C.UTF-8"}

    def _run_publish(self, base: Path, sql_exec, installed_check=None, replace_hook=os.replace, **extra):
        os.chmod(base, 0o755)
        cfg = base / "cfg"
        cfg.mkdir(mode=0o755, exist_ok=True)
        os.chmod(cfg, 0o755)
        pub_kw = {"now_ms": 2000, **extra}
        return candidate_binding.publish_binding(
            identities=sample_identities("c"),
            installed_identities=sample_identities("c") if installed_check is None else installed_check,
            resolver_ipv4="9.9.9.9", valid_from_ms=1000, valid_until_ms=4_000_000_000_000,
            manifest_path=cfg / "deployment.json", intent_dir=base / "intent", sql_executor=sql_exec,
            env=self._base_env(), expected_uids=(os.geteuid(),), stop_at=base, replacer=replace_hook, **pub_kw,
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

    def test_sql_precheck_timeout_returns_bounded_receipt_with_zero_inserts(self):
        with tempfile.TemporaryDirectory() as tmp:
            base = Path(tmp)
            os.chmod(base, 0o755)
            cfg = base / "cfg"
            cfg.mkdir(mode=0o755)
            os.chmod(cfg, 0o755)
            prior = cfg / "deployment.json"
            prior.write_bytes(b'{"prior":true}\n')
            os.chmod(prior, 0o444)
            state = {"inserts": 0}

            def precheck_timeout(op: str, _arg=None):
                if op == "exists":
                    raise TimeoutError("postgresql://admin:secretpw@127.0.0.1:5432/dw")
                state["inserts"] += 1
                return None

            res = self._run_publish(base, precheck_timeout, descriptor_sha256="e" * 64)
            self.assertEqual(res["status"], "UNKNOWN")
            self.assertFalse(res["current_qualified"])
            self.assertFalse(res["row_manifest_consistent"])
            self.assertEqual(res["last_phase"], "sql_precheck")
            self.assertEqual(state["inserts"], 0)
            self.assertEqual(prior.read_bytes(), b'{"prior":true}\n')
            self.assertNotIn("secretpw", res["reason"])
            self.assertNotIn("postgresql://", res["reason"])
            receipt_file = base / "intent" / f"receipt-{res['qualification_ref']}.json"
            self.assertTrue(receipt_file.exists())

    def test_post_insert_clock_and_host_failures_contained(self):
        with tempfile.TemporaryDirectory() as tmp:
            base = Path(tmp)
            os.chmod(base, 0o755)
            for idx, fail_mode in enumerate(("clock", "host")):
                sub = base / f"case-{idx}"
                sub.mkdir(mode=0o755)
                os.chmod(sub, 0o755)
                state = {"rows": {}, "writes": 0, "ticks": 0}

                def sql_ok(op: str, row=None, st=state):
                    if op == "exists":
                        return row in st["rows"]
                    if op == "insert":
                        st["writes"] += 1
                        st["rows"][row["qualification_ref"]] = dict(row)
                        return None
                    return st["rows"].get(row)

                def tick(st=state, mode=fail_mode):
                    st["ticks"] += 1
                    if st["ticks"] > 1 and mode == "clock":
                        raise RuntimeError("clock_unavailable")
                    return 2000

                reobs = (lambda: (_ for _ in ()).throw(candidate_binding.HostObservationError("UNKNOWN", "reobserve_failed"))) if fail_mode == "host" else None
                res = self._run_publish(sub, sql_ok, clock_fn=tick, host_reobserver=reobs, now_ms=None)
                self.assertIn(res["status"], ("INCOMPLETE", "UNKNOWN"))
                self.assertFalse(res["current_qualified"])
                self.assertTrue(res["row_manifest_consistent"])
                self.assertEqual(state["writes"], 1)
                self.assertEqual(
                    res["last_phase"],
                    "final_clock_check" if fail_mode == "clock" else "final_host_reobserve",
                )
                self.assertTrue((sub / "intent" / f"receipt-{res['qualification_ref']}.json").exists())

    def test_receipt_persist_oserror_after_commit_cannot_return_success(self):
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

            def fail_rcpt(s: Path, d: Path):
                if "receipt-" in d.name:
                    raise OSError("postgresql://leak:pw@127.0.0.1/db")
                os.replace(s, d)

            res = self._run_publish(base, good_sql, replace_hook=fail_rcpt)
            self.assertEqual(res["status"], "UNKNOWN")
            self.assertFalse(res["current_qualified"])
            self.assertIn("receipt_persist_failed", res["reason"])
            self.assertNotIn("postgresql://", res["reason"])

    def test_reconcile_rejects_lab_extra_dup_missing_and_bad_types_even_with_matching_row(self):
        with tempfile.TemporaryDirectory() as tmp:
            base = Path(tmp)
            os.chmod(base, 0o755)
            qref = "22222222-2222-4222-8222-222222222222"
            base_obj = {
                **sample_identities("c"),
                "qualification_ref": qref,
                "purpose": "LIVE",
                "run_profile": "autonomous-dns-genai-live-v1",
                "resolver_ipv4": "9.9.9.9",
                "valid_from_ms": 1000,
                "valid_until_ms": 4_000_000_000_000,
            }
            cases = [
                json.dumps({**base_obj, "purpose": "LAB", "run_profile": "autonomous-dns-genai-lab-v1", "extra": 1}).encode(),
                json.dumps({**base_obj, "unknown_field": True}).encode(),
                ('{"purpose":"LIVE","purpose":"LAB",' + json.dumps({k: v for k, v in base_obj.items() if k != "purpose"})[1:]).encode(),
                json.dumps({k: v for k, v in base_obj.items() if k != "resolver_ipv4"}).encode(),
                json.dumps({**base_obj, "core_uid": True}).encode(),
                json.dumps({**base_obj, "valid_from_ms": True}).encode(),
                json.dumps({**base_obj, "resolver_ipv4": "192.88.99.1"}).encode(),
                json.dumps({**base_obj, "boot_id": "00000000-0000-0000-0000-000000000000"}).encode(),
            ]
            for idx, raw in enumerate(cases):
                sub = base / f"bad-{idx}"
                sub.mkdir(mode=0o755)
                os.chmod(sub, 0o755)
                mpath = sub / "deployment.json"
                mpath.write_bytes(raw)
                os.chmod(mpath, 0o444)
                m_sha = hashlib.sha256(raw).hexdigest()
                parsed = json.loads(raw.decode("utf-8"))
                row = {k: parsed.get(k) for k in candidate_binding.SQL_COLUMNS if k != "manifest_sha256"}
                row["manifest_sha256"] = m_sha
                res = candidate_binding.reconcile_binding(
                    qualification_ref=qref, manifest_path=mpath, installed_identities=sample_identities("c"),
                    sql_reader=lambda _q, r=row: r, expected_uids=(os.geteuid(),), stop_at=sub, clock_fn=lambda: 2000,
                )
                self.assertEqual(res["status"], "INCOMPLETE", f"case {idx}: {res}")
                self.assertFalse(res["row_manifest_consistent"], f"case {idx}: {res}")
                self.assertFalse(res["current_qualified"], f"case {idx}: {res}")

            good_raw = json.dumps(base_obj, sort_keys=True, separators=(",", ":")).encode()
            good_dir = base / "clock-fail"
            good_dir.mkdir(mode=0o755)
            os.chmod(good_dir, 0o755)
            good_path = good_dir / "deployment.json"
            good_path.write_bytes(good_raw)
            os.chmod(good_path, 0o444)
            good_row = {**{k: base_obj[k] for k in candidate_binding.SQL_COLUMNS if k != "manifest_sha256"}, "manifest_sha256": hashlib.sha256(good_raw).hexdigest()}
            clk_res = candidate_binding.reconcile_binding(
                qualification_ref=qref, manifest_path=good_path, installed_identities=sample_identities("c"),
                sql_reader=lambda _q: good_row, expected_uids=(os.geteuid(),), stop_at=good_dir,
                clock_fn=lambda: (_ for _ in ()).throw(RuntimeError("clock_err")),
            )
            self.assertEqual(clk_res["status"], "INCOMPLETE")
            self.assertTrue(clk_res["row_manifest_consistent"])
            self.assertFalse(clk_res["current_qualified"])

    def test_quiescence_reconcile_sql_timeout_nan_and_oversize_contained(self):
        with tempfile.TemporaryDirectory() as tmp:
            base = Path(tmp)
            os.chmod(base, 0o755)
            state = {"writes": 0}
            def no_write_sql(op: str, _row=None):
                if op == "exists":
                    return False
                state["writes"] += 1
                return None
            q_res = self._run_publish(base, no_write_sql, quiescence_checker=lambda: (_ for _ in ()).throw(TimeoutError("postgresql://q:pw@h/db")))
            self.assertEqual(q_res["status"], "UNKNOWN")
            self.assertEqual(q_res["last_phase"], "quiescence_recheck")
            self.assertEqual(state["writes"], 0)
            self.assertNotIn("postgresql://", q_res["reason"])

            qref = "33333333-3333-4333-8333-333333333333"
            for label, payload in (("nan", b'{"valid_from_ms":NaN}'), ("big", b'{"extra":"' + b"a" * 16380 + b'"}')):
                sub = base / label
                sub.mkdir(mode=0o755)
                os.chmod(sub, 0o755)
                mp = sub / "deployment.json"
                mp.write_bytes(payload)
                os.chmod(mp, 0o444)
                r = candidate_binding.reconcile_binding(qualification_ref=qref, manifest_path=mp, installed_identities=sample_identities("c"), sql_reader=lambda _q: None, expected_uids=(os.geteuid(),), stop_at=sub)
                self.assertEqual(r["status"], "INCOMPLETE")
                self.assertFalse(r["row_manifest_consistent"])
                self.assertFalse(r["current_qualified"])

            ok_dir = base / "sql-timeout"
            ok_dir.mkdir(mode=0o755)
            os.chmod(ok_dir, 0o755)
            ok_p = ok_dir / "deployment.json"
            ok_raw = json.dumps({**sample_identities("c"), "qualification_ref": qref, "purpose": "LIVE", "run_profile": "autonomous-dns-genai-live-v1", "resolver_ipv4": "9.9.9.9", "valid_from_ms": 1000, "valid_until_ms": 4_000_000_000_000}, sort_keys=True, separators=(",", ":")).encode()
            ok_p.write_bytes(ok_raw)
            os.chmod(ok_p, 0o444)
            s_res = candidate_binding.reconcile_binding(qualification_ref=qref, manifest_path=ok_p, installed_identities=sample_identities("c"), sql_reader=lambda _q: (_ for _ in ()).throw(TimeoutError("postgresql://r:pw@h/db")), expected_uids=(os.geteuid(),), stop_at=ok_dir)
            self.assertEqual(s_res["status"], "UNKNOWN")
            self.assertFalse(s_res["row_manifest_consistent"])
            self.assertNotIn("postgresql://", s_res["reason"])
