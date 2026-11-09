"""Worker-artifact selection and recovery regressions.

Synthetic paginated API fixtures, a fake `gh` shim and the real staging/
aggregate consumers. The hosted download action is never executed here;
archive contents are modeled only as synthetic fixture directories.
"""
import fnmatch
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import unittest

import test_ci_scope
import test_ci_rust_shards as shards

ROOT = Path(__file__).resolve().parents[1]
SCRIPT = ROOT / "scripts" / "ci_worker_artifacts.py"
WORKFLOW = ROOT / ".github" / "workflows" / "doc-check.yml"

# Observed incident identity (run 42579380886, attempt 3): worker 0's
# artifact belongs to the partial re-run attempt while worker 1's passing
# evidence kept attempt 2, so the attempt-scoped name filter dropped it.
SHA = "fbf9bd7793c1ca32e87ae06176747b9d005afe02"
RUN = 42579380886
W0_ID, W1_ID = 13659376953, 13657873360


def worker_name(worker, attempt, sha=SHA, run=RUN):
    return f"dw-rust-worker-{worker}-{sha}-{run}-{attempt}"


def artifact(name, aid, expired=False, workflow_run=RUN):
    record = {"id": aid, "name": name, "expired": expired,
              "size_in_bytes": 64, "digest": "sha256:0" * 8,
              "archive_download_url": "https://example.invalid/a"}
    if workflow_run is not None:
        record["workflow_run"] = {"id": workflow_run}
    return record


def page(*records):
    return {"total_count": len(records), "artifacts": list(records)}


class SelectorCase(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.out = Path(self.tmp.name) / "ghout"
        self.out.write_text("")

    def sel(self, inventory, attempt=3, workers=2, sha=SHA, run=RUN):
        inv = Path(self.tmp.name) / "inv.json"
        inv.write_text(json.dumps(inventory), encoding="utf-8")
        return subprocess.run(
            [sys.executable, str(SCRIPT), "select",
             "--inventory", str(inv), "--sha", str(sha),
             "--run-id", str(run), "--attempt", str(attempt),
             "--workers", str(workers), "--output", str(self.out)],
            capture_output=True, text=True)

    def outputs(self):
        return dict(line.split("=", 1) for line in
                    self.out.read_text().splitlines() if "=" in line)


class SelectionTest(SelectorCase):
    def test_full_run_same_attempt_selects_both_workers(self):
        inv = [page(artifact(f"dw-rust-build-{SHA}-{RUN}", 9),
                    artifact(worker_name(0, 4), 4001),
                    artifact(worker_name(1, 4), 4002))]
        r = self.sel(inv, attempt=4)
        self.assertEqual(r.returncode, 0, r.stderr)
        self.assertEqual(self.outputs(),
                         {"worker_0_id": "4001", "worker_1_id": "4002"})

    def test_serial_mode_selects_worker_0_only(self):
        inv = [page(artifact(worker_name(0, 2), 2001))]
        r = self.sel(inv, attempt=2, workers=1)
        self.assertEqual(r.returncode, 0, r.stderr)
        self.assertEqual(self.outputs(), {"worker_0_id": "2001"})

    def test_partial_rerun_selects_newest_per_worker(self):
        inv = [page(artifact(worker_name(0, 2), 1),
                    artifact(worker_name(1, 2), W1_ID),
                    artifact(worker_name(0, 3), W0_ID))]
        r = self.sel(inv)
        self.assertEqual(r.returncode, 0, r.stderr)
        self.assertEqual(self.outputs(), {"worker_0_id": str(W0_ID),
                                         "worker_1_id": str(W1_ID)})
        self.assertIn(f"worker=0 attempt=3 id={W0_ID}", r.stdout)
        self.assertIn(f"worker=1 attempt=2 id={W1_ID}", r.stdout)

    def test_aggregator_rerun_reuses_prior_attempt_artifacts(self):
        inv = [page(artifact(worker_name(0, 4), 5001),
                    artifact(worker_name(1, 4), 5002))]
        r = self.sel(inv, attempt=5)
        self.assertEqual(r.returncode, 0, r.stderr)
        self.assertEqual(self.outputs(), {"worker_0_id": "5001",
                                         "worker_1_id": "5002"})

    def test_pagination_and_shuffled_order_do_not_change_selection(self):
        decoys = [artifact(f"dw-rust-build-{SHA}-{RUN}", 7),
                  artifact(f"depsec-{SHA}-{RUN}-3", 8)]
        expected = {"worker_0_id": str(W0_ID), "worker_1_id": str(W1_ID)}
        w0, w1 = artifact(worker_name(0, 3), W0_ID), artifact(worker_name(1, 2), W1_ID)
        for inv in ([page(*decoys, w0, w1)], [page(decoys[0], w1), page(w0, decoys[1])],
                    [page(w1, decoys[1], w0, decoys[0])]):
            with self.subTest(inv=inv):
                r = self.sel(inv)
                self.assertEqual(r.returncode, 0, r.stderr)
                self.assertEqual(self.outputs(), expected)
                self.out.write_text("")

    def test_missing_unassigned_or_future_worker_fails_closed(self):
        cases = [
            [page(artifact(worker_name(0, 3), 1))],
            [page(artifact(worker_name(0, 3), 1),
                  artifact(worker_name(2, 2), 2))],
            [page(artifact(worker_name(0, 3), 1),
                  artifact(worker_name(1, 4), 2))],
        ]
        for inv in cases:
            with self.subTest(inv=inv):
                self.assertNotEqual(self.sel(inv).returncode, 0)
                self.assertNotIn("worker_", self.out.read_text())

    def test_duplicate_identity_or_id_fails(self):
        cases = [
            [page(artifact(worker_name(0, 3), 1),
                  artifact(worker_name(0, 3), 2),
                  artifact(worker_name(1, 3), 3))],
            [page(artifact(worker_name(0, 3), 1),
                  artifact(worker_name(1, 2), 1))],
        ]
        for inv in cases:
            with self.subTest(inv=inv):
                self.assertNotEqual(self.sel(inv).returncode, 0)

    def test_malformed_relevant_identities_fail(self):
        bad = [
            "dw-rust-worker-0",
            f"dw-rust-worker-0-{SHA}",
            f"dw-rust-worker-0-{SHA}-{RUN}",
            f"dw-rust-worker-0-{SHA}-{RUN}-x",
            f"dw-rust-worker-0-{SHA}-{RUN}-3-extra",
            f"dw-rust-worker-x-{SHA}-{RUN}-3",
            f"dw-rust-worker-0-{SHA[:-1]}z-{RUN}-3",
            f"dw-rust-worker--0-{SHA}-{RUN}-3",
        ]
        for name in bad:
            inv = [page(artifact(name, 1), artifact(worker_name(0, 3), 2),
                        artifact(worker_name(1, 2), 3))]
            with self.subTest(name=name):
                self.assertNotEqual(self.sel(inv).returncode, 0)

    def test_invalid_record_fields_fail(self):
        w1 = artifact(worker_name(1, 2), 3)
        w0 = artifact(worker_name(0, 3), 1)
        no_id = {k: v for k, v in w0.items() if k != "id"}
        cases = [{**w0, "id": 0}, {**w0, "id": "1"}, no_id,
                 {**w0, "expired": "no"},
                 {**w0, "workflow_run": {"id": RUN + 1}}]
        for record in cases:
            with self.subTest(record=record):
                self.assertNotEqual(self.sel([page(record, w1)]).returncode, 0)

    def test_attempt_zero_and_missing_expired_flag_fail(self):
        w1 = artifact(worker_name(1, 2), 3)
        for record in (
            artifact(worker_name(0, 0), 1),
            {k: v for k, v in artifact(worker_name(0, 3), 1).items()
             if k != "expired"},
        ):
            with self.subTest(record=record):
                self.assertNotEqual(self.sel([page(record, w1)]).returncode, 0)

    def test_older_expired_attempt_is_skipped_for_newer_valid(self):
        inv = [page({**artifact(worker_name(0, 2), 1), "expired": True},
                    artifact(worker_name(0, 3), W0_ID),
                    artifact(worker_name(1, 2), W1_ID))]
        r = self.sel(inv)
        self.assertEqual(r.returncode, 0, r.stderr)
        self.assertEqual(self.outputs(), {"worker_0_id": str(W0_ID),
                                         "worker_1_id": str(W1_ID)})

    def test_selected_expired_fails_without_fallback_to_older(self):
        inv = [page(artifact(worker_name(0, 2), 1),
                    artifact(worker_name(1, 2), 2),
                    artifact(worker_name(0, 3), 3, expired=True))]
        r = self.sel(inv)
        self.assertNotEqual(r.returncode, 0)
        self.assertNotIn("worker_", self.out.read_text())

    def test_other_checkout_or_run_cannot_fill_worker_slot(self):
        other = "a" * 40
        inv = [page(artifact(worker_name(0, 3), 1),
                    artifact(worker_name(1, 2, sha=other), 2),
                    artifact(worker_name(1, 2, run=RUN + 1), 3))]
        self.assertNotEqual(self.sel(inv).returncode, 0)

    def test_bad_arguments_and_unreadable_inventory_fail(self):
        inv = [page(artifact(worker_name(0, 3), 1),
                    artifact(worker_name(1, 2), 2))]
        for kw in ({"sha": "xyz"}, {"run": 0}, {"attempt": 0}, {"workers": 3}):
            with self.subTest(kw=kw):
                self.assertNotEqual(self.sel(inv, **kw).returncode, 0)
        inv_path = Path(self.tmp.name) / "inv.json"
        inv_path.write_text("not json", encoding="utf-8")
        for target in (inv_path, Path(self.tmp.name) / "absent.json"):
            with self.subTest(target=target.name):
                r = subprocess.run(
                    [sys.executable, str(SCRIPT), "select",
                     "--inventory", str(target), "--sha", SHA,
                     "--run-id", str(RUN), "--attempt", "3", "--workers", "2"],
                    capture_output=True, text=True)
                self.assertNotEqual(r.returncode, 0)

    def test_empty_or_shapeless_inventory_fails(self):
        for inv in ([], [{}], [page()], [["x"]], [{"artifacts": "x"}],
                    [{"artifacts": ["x"]}]):
            with self.subTest(inv=inv):
                self.assertNotEqual(self.sel(inv).returncode, 0)


class WorkflowSelectionTest(unittest.TestCase):
    """Run the real workflow selection shell against a synthetic `gh`."""

    BASH = shutil.which("bash")

    @classmethod
    def setUpClass(cls):
        cls.rust = test_ci_scope.job_blocks(WORKFLOW)["rust"]

    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.root = Path(self.tmp.name)

    def _body(self, step):
        return test_ci_scope.step_body(self.rust, step)

    def _shim(self, payload=None, fail=False):
        d = self.root / "shim"
        d.mkdir(exist_ok=True)
        shutil.copy(sys.executable, d / "python3.exe")
        gh = d / "gh"
        gh.write_text("#!/usr/bin/env bash\nexit 1\n" if fail else
                      "#!/usr/bin/env bash\ncat \"$GH_INVENTORY\"\n")
        os.chmod(gh, 0o755)
        if payload is not None:
            (self.root / "inventory.json").write_text(
                json.dumps(payload), encoding="utf-8")
        return d

    def _run(self, *steps, workers=2, attempt=3, shim=None):
        body = "".join(self._body(s) for s in steps)
        for key, value in {
            "${{ github.repository }}": "o/r",
            "${{ github.run_id }}": str(RUN),
            "${{ github.run_attempt }}": str(attempt),
            "${{ github.sha }}": SHA,
            "${{ needs.changes.outputs.rust_workers }}": str(workers),
            "${{ secrets.GITHUB_TOKEN }}": "synthetic-token",
        }.items():
            body = body.replace(key, value)
        script = self.root / "select.sh"
        script.write_bytes(body.encode())
        (self.root / "ghout").write_text("")
        env = dict(os.environ)
        env["RUNNER_TEMP"] = shards._posix(self.root)
        env["GITHUB_OUTPUT"] = shards._posix(self.root / "ghout")
        env["GH_INVENTORY"] = shards._posix(self.root / "inventory.json")
        if shim:
            env["PATH"] = str(shim) + os.pathsep + env["PATH"]
        return subprocess.run([self.BASH, script.as_posix()], cwd=ROOT,
                              env=env, capture_output=True, text=True)

    def test_listing_and_selection_produce_ids_for_partial_rerun(self):
        payload = [page(artifact(worker_name(1, 2), W1_ID),
                        artifact(worker_name(0, 2), 1)),
                   page(artifact(worker_name(0, 3), W0_ID))]
        r = self._run("List this run's artifact inventory",
                      "Select worker evidence artifact IDs",
                      shim=self._shim(payload))
        self.assertEqual(r.returncode, 0, r.stderr)
        out = (self.root / "ghout").read_text()
        self.assertIn(f"worker_0_id={W0_ID}", out)
        self.assertIn(f"worker_1_id={W1_ID}", out)

    def test_serial_fanout_selects_worker_0_only(self):
        payload = [page(artifact(worker_name(0, 2), 42))]
        r = self._run("List this run's artifact inventory",
                      "Select worker evidence artifact IDs",
                      workers=1, attempt=2, shim=self._shim(payload))
        self.assertEqual(r.returncode, 0, r.stderr)
        self.assertEqual((self.root / "ghout").read_text().strip(),
                         "worker_0_id=42")

    def test_failed_listing_stops_before_selection(self):
        r = self._run("List this run's artifact inventory",
                      "Select worker evidence artifact IDs",
                      shim=self._shim(fail=True))
        self.assertNotEqual(r.returncode, 0)
        self.assertEqual((self.root / "ghout").read_text(), "")


class BaselineContractTest(unittest.TestCase):
    """The incident contract: w0 attempt 3 plus w1 attempt 2 must recover."""

    @classmethod
    def setUpClass(cls):
        cls.rust = test_ci_scope.job_blocks(WORKFLOW)["rust"]

    def test_name_or_pattern_downloads_cannot_drop_prior_attempts(self):
        names = {worker_name(0, 3), worker_name(1, 2)}
        region = self.rust[self.rust.index("Extract the object set"):
                           self.rust.index("Verify candidate, tools")]
        for value in re_findall_download_names(region):
            pat = (value.replace("${{ github.sha }}", SHA)
                        .replace("${{ github.run_id }}", str(RUN))
                        .replace("${{ github.run_attempt }}", "3")
                        .replace("${{ matrix.worker }}", "0"))
            matched = {n for n in names if fnmatch.fnmatch(n, pat)}
            self.assertEqual(matched, names,
                f"attempt-scoped selection {pat!r} drops prior-attempt "
                "worker evidence")
        self.assertIn("artifact-ids:", region)


def re_findall_download_names(region):
    import re
    return [v for v in re.findall(r"(?:pattern|name): (\S+)", region)
            if "dw-rust-worker" in v]


class RecoveryTest(unittest.TestCase):
    """Selected evidence through the real staging/aggregate consumer."""

    BASH = shutil.which("bash")

    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.ws = shards.Workspace(self.tmp.name)
        self.ws.plan()
        (self.ws.root / "scripts").mkdir(exist_ok=True)
        shutil.copy(shards.SCRIPT,
                    self.ws.root / "scripts" / "ci_rust_shards.py")
        p = shards.read_json(self.ws.root / "ci-shards" / "partition-2.json")
        self.assign = {int(k): v for k, v in p["assignments"].items()}
        for worker, assignment in self.assign.items():
            ev = self.ws.evidence(worker, assignment)
            r = self.ws.run(
                "receipt", "--manifest",
                str(self.ws.root / "ci-shards" / "manifest.json"),
                "--partition",
                str(self.ws.root / "ci-shards" / "partition-2.json"),
                "--worker", str(worker), "--report", str(ev / "suite"),
                "--out", str(ev / "receipt.json"))
            self.assertEqual(r.returncode, 0, r.stderr)
        self.ev = Path(self.tmp.name) / "dw-evidence"

    def _place(self, worker, src=None, profraw=True):
        dst = self.ev / f"dw-rust-worker-{worker}"
        shutil.copytree(src or self.ws.root / f"evidence-w{worker}", dst)
        if profraw:
            (dst / "profraw").mkdir(exist_ok=True)
            (dst / "profraw" / f"w{worker}-1-2.profraw").write_bytes(b"p")
        return dst

    def _stage(self, workers):
        body = (shards._staging_body()
                .replace("${{ needs.changes.outputs.rust_workers }}",
                         str(workers))
                .replace("$(git rev-parse HEAD)", "a" * 40))
        script = Path(self.tmp.name) / "stage.sh"
        script.write_bytes(body.encode())
        shim = Path(self.tmp.name) / "shim"
        shim.mkdir(exist_ok=True)
        py3 = shim / "python3.exe"
        if not py3.exists():
            shutil.copy(sys.executable, py3)
        env = dict(os.environ)
        env["PATH"] = str(shim) + os.pathsep + env["PATH"]
        env["RUNNER_TEMP"] = shards._posix(self.tmp.name)
        env["GITHUB_WORKSPACE"] = shards._posix(self.ws.root)
        return subprocess.run([self.BASH, script.as_posix()],
                              cwd=self.ws.root, env=env,
                              capture_output=True, text=True)

    def test_selected_worker_roots_stage_and_aggregate(self):
        self._place(0)
        self._place(1)
        r = self._stage(2)
        self.assertEqual(r.returncode, 0, r.stderr)

    def test_serial_selected_root_stages(self):
        p1 = shards.read_json(self.ws.root / "ci-shards" / "partition-1.json")
        ev = self.ws.evidence(0, p1["assignments"]["0"])
        r = self.ws.run(
            "receipt", "--manifest",
            str(self.ws.root / "ci-shards" / "manifest.json"), "--partition",
            str(self.ws.root / "ci-shards" / "partition-1.json"),
            "--worker", "0", "--report", str(ev / "suite"),
            "--out", str(ev / "receipt.json"))
        self.assertEqual(r.returncode, 0, r.stderr)
        self._place(0, src=ev)
        r = self._stage(1)
        self.assertEqual(r.returncode, 0, r.stderr)

    def test_flattened_single_artifact_layout_is_rejected(self):
        # Observed baseline shape: a single selected artifact extracts into
        # the evidence root itself, leaving no named worker directory at all.
        self.ev.mkdir(exist_ok=True)
        src = self.ws.root / "evidence-w0"
        for item in src.iterdir():
            if item.is_dir():
                shutil.copytree(item, self.ev / item.name)
            else:
                shutil.copy2(item, self.ev / item.name)
        self._place(1)
        self.assertNotEqual(self._stage(2).returncode, 0)

    def test_newer_selected_bad_evidence_rejected_without_fallback(self):
        self._place(0)
        self._place(1)
        # A newer attempt's worker-0 artifact whose receipt does not bind
        # this manifest: the selector still picks the newest identity, and
        # the real consumer must reject it rather than reuse older evidence.
        bad = Path(self.tmp.name) / "bad-w0"
        shutil.copytree(self.ws.root / "evidence-w0", bad)
        receipt = shards.read_json(bad / "receipt.json")
        receipt["manifest_sha256"] = "0" * 64
        (bad / "receipt.json").write_text(json.dumps(receipt))
        bad_dir = self.ev / "dw-rust-worker-0"
        shutil.rmtree(bad_dir)
        shutil.copytree(bad, bad_dir)
        r = self._stage(2)
        self.assertNotEqual(r.returncode, 0)


if __name__ == "__main__":
    unittest.main()
