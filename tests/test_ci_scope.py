"""Synthetic Git histories and gate results; no services or credentials."""
import json
import os
from pathlib import Path
import re
import subprocess
import sys
import tempfile
import unittest

SCRIPT = Path(__file__).resolve().parents[1] / "scripts/ci_scope.py"
JOBS = ("changes", "links", "structure", "metrics", "rust", "codeql", "dependencies")


class ScopeHistory(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.root = Path(self.tmp.name) / "repo"
        self.root.mkdir()
        self.git("init", "-q")
        self.git("config", "user.name", "Synthetic CI")
        self.git("config", "user.email", "ci@example.invalid")
        self.write("README.md", "baseline\n")
        self.commit()
        self.base = self.git("rev-parse", "HEAD").strip()

    def git(self, *args):
        return subprocess.run(["git", *args], cwd=self.root, check=True,
                              capture_output=True, text=True).stdout

    def write(self, name, text):
        path = self.root / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(text, encoding="utf-8")

    def commit(self):
        self.git("add", "-A")
        self.git("commit", "-qm", "synthetic change")

    def classify(self, event="pull_request", base=None):
        event_file = self.root.parent / "event.json"
        revision = self.base if base is None else base
        event_file.write_text(json.dumps({"before": revision,
            "pull_request": {"base": {"sha": revision}}}), encoding="utf-8")
        output = self.root.parent / "outputs"
        env = dict(os.environ, GITHUB_EVENT_NAME=event,
                   GITHUB_EVENT_PATH=str(event_file), GITHUB_OUTPUT=str(output))
        result = subprocess.run([sys.executable, str(SCRIPT), "classify"],
                                cwd=self.root, env=env, capture_output=True, text=True)
        self.assertEqual(result.returncode, 0, result.stderr)
        return output.read_text(encoding="utf-8").splitlines()[-1]

    def test_document_only_change_selects_document_lane(self):
        self.write("docs/adr/ADR-001.md", "synthetic design\n")
        self.commit()
        self.assertEqual(self.classify(), "scope=docs")

    def test_known_skill_markdown_with_root_docs_selects_document_lane(self):
        for name in (
            ".agents/skills/build-duskweave/SKILL.md",
            ".agents/skills/duskweave-engineering/SKILL.md",
            ".agents/skills/duskweave-engineering/references/packet-template.md",
            ".agents/skills/duskweave-adversarial-review/SKILL.md",
        ):
            with self.subTest(name=name):
                self.write(name, "synthetic skill prose\n")
                self.write("QUALITY_BAR.md", f"synthetic policy prose {name}\n")
                self.commit()
                self.assertEqual(set(self.git("diff", "--name-only", self.base,
                                              "HEAD").splitlines()),
                                 {name, "QUALITY_BAR.md"})
                self.assertEqual(self.classify(), "scope=docs")
                self.base = self.git("rev-parse", "HEAD").strip()

    def test_mixed_or_unknown_inputs_require_full_verification(self):
        for name in ("src/lib.rs", "migrations/001.sql", "Cargo.lock",
                     "tests/t.rs", ".github/workflows/test.yml",
                     "docs/contracts/fixture.json", "unknown.md", ".gitignore",
                     "Cargo.toml", "rust-toolchain.toml", ".cargo/config.toml",
                     "scripts/ci_scope.py", "tests/test_ci_scope.py",
                     ".agents/skills/build-duskweave/check.py",
                     ".agents/skills/build-duskweave/references/unknown.md",
                     ".agents/skills/unknown/SKILL.md"):
            with self.subTest(name=name):
                self.write(name, "synthetic\n")
                skill = ".agents/skills/build-duskweave/SKILL.md"
                self.write(skill, f"synthetic skill prose {name}\n")
                self.commit()
                self.assertEqual(set(self.git("diff", "--name-only", self.base,
                                              "HEAD").splitlines()), {name, skill})
                self.assertEqual(self.classify(), "scope=full")
                self.base = self.git("rev-parse", "HEAD").strip()

    def test_runtime_renamed_to_document_still_requires_full(self):
        self.write("src/lib.rs", "runtime\n")
        self.commit()
        self.base = self.git("rev-parse", "HEAD").strip()
        target = ".agents/skills/duskweave-engineering/SKILL.md"
        (self.root / target).parent.mkdir(parents=True)
        self.git("mv", "src/lib.rs", target)
        self.commit()
        self.assertEqual(self.classify(), "scope=full")

    def test_missing_history_empty_change_and_schedule_require_full(self):
        self.assertEqual(self.classify(), "scope=full")
        self.assertEqual(self.classify(base="0" * 40), "scope=full")
        self.assertEqual(self.classify(base="1" * 40), "scope=full")
        self.assertEqual(self.classify(event="schedule"), "scope=full")

    def test_document_to_runtime_rename_cannot_enter_document_lane(self):
        self.git("mv", "README.md", "build.rs")
        self.commit()
        self.assertEqual(self.classify(), "scope=full")

    def test_executable_document_requires_full(self):
        for name in ("docs/adr/ADR-002.md", ".agents/skills/build-duskweave/SKILL.md"):
            with self.subTest(name=name):
                self.write(name, "document\n")
                self.git("add", name)
                self.git("update-index", "--chmod=+x", name)
                self.git("commit", "-qm", "executable document")
                self.assertEqual(self.classify(), "scope=full")
                self.base = self.git("rev-parse", "HEAD").strip()

    def test_push_uses_before_and_docs_deletion_is_classified(self):
        self.git("rm", "README.md")
        self.commit()
        self.assertEqual(self.classify(event="push"), "scope=docs")

    def test_document_symlink_requires_full_without_following_it(self):
        for name in ("docs/link.md", ".agents/skills/build-duskweave/SKILL.md"):
            with self.subTest(name=name):
                self.write(name, "../../src/lib.rs")
                self.git("add", name)
                blob = self.git("hash-object", name).strip()
                self.git("update-index", "--cacheinfo", f"120000,{blob},{name}")
                self.git("commit", "-qm", "synthetic symlink")
                self.assertEqual(self.classify(), "scope=full")
                self.base = self.git("rev-parse", "HEAD").strip()


class RequiredGate(unittest.TestCase):
    def run_gate(self, scope="full", states=None, missing=None):
        needs = {name: {"result": "success"} for name in JOBS}
        needs["changes"]["outputs"] = {"scope": scope}
        for name, value in (states or {}).items():
            needs[name]["result"] = value
        if missing:
            del needs[missing]
        return subprocess.run([sys.executable, str(SCRIPT), "gate"],
            env=dict(os.environ, NEEDS=json.dumps(needs)), capture_output=True, text=True)

    def test_explicit_docs_skip_and_full_success_pass(self):
        self.assertEqual(self.run_gate().returncode, 0)
        self.assertEqual(self.run_gate("docs", {"rust": "skipped"}).returncode, 0)

    def test_missing_failed_cancelled_and_unclassified_results_fail(self):
        cases = [("full", {"rust": "skipped"}, None),
                 ("docs", {"rust": "failure"}, None),
                 ("docs", {"rust": "cancelled"}, None),
                 ("docs", {"changes": "failure", "rust": "skipped"}, None),
                 ("docs", {"codeql": "skipped"}, None),
                 ("unknown", {}, None), ("full", {}, "rust")]
        for scope, states, missing in cases:
            with self.subTest(scope=scope, states=states, missing=missing):
                self.assertNotEqual(self.run_gate(scope, states, missing).returncode, 0)


def job_blocks(workflow_path):
    """Split `jobs:` into {name: body} using indentation only (no YAML dep)."""
    text = Path(workflow_path).read_text(encoding="utf-8")
    start = text.index("\njobs:\n") + len("\njobs:\n")
    body = text[start:]
    blocks, name, current = {}, None, []
    for line in body.splitlines():
        if line and not line.startswith(" "):
            break
        key = re.match(r"^  ([A-Za-z0-9_-]+):\s*$", line)
        if key:
            if name:
                blocks[name] = "\n".join(current)
            name, current = key.group(1), []
        elif name:
            current.append(line)
    if name:
        blocks[name] = "\n".join(current)
    return blocks


def needs_of(block):
    inline = re.search(r"^    needs: \[([^\]]+)\]", block, re.M)
    if inline:
        return [n.strip() for n in inline.group(1).split(",")]
    listed = re.findall(r"^      - (\S+)\s*$", block, re.M)
    single = re.search(r"^    needs: (\S+)\s*$", block, re.M)
    return listed or ([single.group(1)] if single else [])


def step_body(block, name):
    """Extract the run: | body of a named step inside a job block."""
    start = block.index(f"- name: {name}")
    seg = block[start:start + 8000]
    match = re.search(r"run: \|\r?\n((?:[ ]{10,}.*\r?\n?)+)", seg)
    assert match, f"no run body for step {name}"
    return "\n".join(line[10:] if line.startswith(" " * 10) else line
                     for line in match.group(1).splitlines()) + "\n"


class WorkflowWiring(unittest.TestCase):
    """Structural wiring of the sharded rust pipeline in doc-check.yml."""

    @classmethod
    def setUpClass(cls):
        cls.jobs = job_blocks(Path(__file__).resolve().parents[1]
                              / ".github/workflows/doc-check.yml")

    def test_rust_pipeline_shape(self):
        for name in ("rust-build", "rust-tests", "rust"):
            self.assertIn(name, self.jobs)
        self.assertEqual(needs_of(self.jobs["rust-build"]), ["changes"])
        self.assertEqual(set(needs_of(self.jobs["rust-tests"])),
                         {"changes", "rust-build"})
        self.assertEqual(set(needs_of(self.jobs["rust"])),
                         {"changes", "rust-build", "rust-tests"})

    def test_ci_ok_gate_set_is_unchanged(self):
        self.assertEqual(set(needs_of(self.jobs["ci-ok"])), set(JOBS))

    def test_docs_scope_skip_and_failure_propagation_guards(self):
        for name in ("rust-build", "rust-tests", "rust"):
            block = self.jobs[name]
            guard = re.search(r"^    if: (.*)$", block, re.M).group(1)
            self.assertIn("!cancelled()", guard)
            self.assertIn("needs.changes.outputs.scope != 'docs'", guard)
            self.assertIn("needs.changes.result != 'success'", guard)
        # rust-tests additionally requires the shared build to succeed.
        self.assertIn("needs.rust-build.result == 'success'",
                      self.jobs["rust-tests"])
        # rust cannot silently pass when build or workers failed.
        self.assertIn("needs.rust-build.result", self.jobs["rust"])
        self.assertIn("needs.rust-tests.result", self.jobs["rust"])

    def test_two_worker_matrix_and_designated_prelude(self):
        tests = self.jobs["rust-tests"]
        self.assertIn("matrix:", tests)
        self.assertIn("fromJSON(needs.changes.outputs.rust_workers_matrix)",
                      tests)
        self.assertIn("if: matrix.worker == 0", tests)
        self.assertIn("prelude.jsonl", tests)
        self.assertIn("LLVM_PROFILE_FILE", tests)

    def test_helper_and_gate_wiring(self):
        for name in ("rust-build", "rust-tests", "rust"):
            self.assertIn("scripts/ci_rust_shards.py", self.jobs[name])
        self.assertIn("enumerate", self.jobs["rust-build"])
        self.assertIn("verify", self.jobs["rust-tests"])
        self.assertIn("aggregate", self.jobs["rust"])
        rust = self.jobs["rust"]
        self.assertIn("cargo llvm-cov report", rust)
        self.assertIn("tests/check_coverage_gate.py", rust)
        # `report` merges profdata + objects only: its commands must carry
        # the JSON/text output paths and the object dir but none of the
        # build-only flags (hosted run rejected --features for 'report').
        commands = [c.strip() for c in re.sub(r"\\\s*\n\s*", " ", rust)
                    .splitlines() if "cargo llvm-cov report" in c]
        self.assertEqual(len(commands), 2)
        for command in commands:
            self.assertNotIn("--features", command)
            self.assertNotIn("--all-targets", command)
        self.assertTrue(any("--json" in c and "m0a-coverage.json" in c
                            for c in commands))
        self.assertTrue(any("--text" in c and "m0a-coverage.txt" in c
                            for c in commands))
        self.assertIn("CARGO_LLVM_COV_TARGET_DIR", rust)
        # The instrumented build, not the report, selects the feature graph.
        build = self.jobs["rust-build"]
        self.assertIn("cargo test --locked --all-targets "
                      "--features fixture-supervisor --no-run", build)
        # Instrumented build happens exactly once, not per worker.
        self.assertIn("cargo llvm-cov show-env", build)
        self.assertNotIn("cargo llvm-cov show-env", self.jobs["rust-tests"])
        self.assertNotIn("cargo test --no-run", self.jobs["rust-tests"])

    def test_worker_evidence_download_modes_are_exclusive(self):
        rust = self.jobs["rust"]
        # Serial mode: a single artifact lands without its named child dir on
        # the pinned action, so it is addressed by exact name into its root.
        serial = re.search(
            r"- name: Download single-worker evidence \(serial mode\)\n"
            r"(.*?)- name:", rust, re.S).group(1)
        self.assertIn("rust_workers == '1'", serial)
        self.assertIn("name: dw-rust-worker-0-", serial)
        self.assertIn("/dw-evidence/dw-rust-worker-0-", serial)
        sharded = re.search(
            r"- name: Download worker evidence and raw profiles "
            r"\(sharded mode\)\n(.*?)- name:", rust, re.S).group(1)
        self.assertIn("rust_workers == '2'", sharded)
        self.assertIn("pattern: dw-rust-worker-*-", sharded)
        self.assertIn("merge-multiple: false", sharded)
        # Staging binds the dir count to the requested worker count and
        # requires nonempty per-worker profiles before invoking aggregate.
        staging = step_body(rust, "Validate complete worker evidence "
                                "and stage profiles")
        self.assertIn("evidence+=(", staging)
        self.assertIn('#evidence[@]}" -ne "$expected"', staging)
        self.assertIn('profraw/*.profraw', staging)
        self.assertIn('"${evidence[@]}"', staging)


class MetricsBaseResolution(unittest.TestCase):
    """Execute the real 'Resolve diff base' body in synthetic git histories."""

    @classmethod
    def setUpClass(cls):
        jobs = job_blocks(Path(__file__).resolve().parents[1]
                          / ".github/workflows/doc-check.yml")
        cls.body = step_body(jobs["metrics"], "Resolve diff base")

    def _repo(self, name="repo"):
        repo = Path(self.tmp.name) / name
        repo.mkdir()
        run = lambda *args: subprocess.run(
            args, cwd=repo, check=True, capture_output=True).stdout
        run("git", "init", "-b", "master")
        run("git", "config", "user.email", "t@example.com")
        run("git", "config", "user.name", "t")
        return repo, run

    def _commit(self, repo, run, marker):
        (repo / "f.txt").write_text(marker)
        run("git", "add", "-A")
        run("git", "commit", "-m", marker)
        return run("git", "rev-parse", "HEAD").decode().strip()

    def _resolve(self, repo, event, ref="refs/heads/feat",
                 pr_base="", before="", sha=""):
        body = (self.body
                .replace("${{ github.event_name }}", event)
                .replace("${{ github.event.pull_request.base.sha }}", pr_base)
                .replace("${{ github.event.before }}", before)
                .replace("${{ github.ref }}", ref)
                .replace("${{ github.sha }}", sha or "0" * 40)
                .replace("$GITHUB_OUTPUT", "ghout"))
        script = repo / "resolve.sh"
        script.write_bytes(body.encode())  # LF only, like hosted bash -e {0}
        out = repo / "ghout"
        out.write_text("")
        r = subprocess.run(["bash", "resolve.sh"], cwd=repo,
                           capture_output=True, text=True)
        base = ""
        if "base=" in out.read_text():
            base = out.read_text().strip().rsplit("base=", 1)[1]
        return r.returncode, base

    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)

    def test_dispatch_branch_uses_master_merge_base(self):
        repo, run = self._repo()
        base_sha = self._commit(repo, run, "master-a")
        run("git", "update-ref", "refs/remotes/origin/master", base_sha)
        run("git", "checkout", "-b", "feat")
        self._commit(repo, run, "b")
        self._commit(repo, run, "c")
        rc, base = self._resolve(repo, "workflow_dispatch",
                                 ref="refs/heads/feat")
        self.assertEqual(rc, 0)
        self.assertEqual(base, base_sha)  # whole branch, not HEAD^

    def test_dispatch_master_uses_parent(self):
        repo, run = self._repo()
        self._commit(repo, run, "a")
        head_parent = self._commit(repo, run, "b")
        self._commit(repo, run, "c")
        rc, base = self._resolve(repo, "workflow_dispatch",
                                 ref="refs/heads/master")
        self.assertEqual(rc, 0)
        self.assertEqual(base, head_parent)  # wait: HEAD^ is c's parent = b

    def test_dispatch_branch_missing_master_fails(self):
        repo, run = self._repo()
        self._commit(repo, run, "a")
        run("git", "checkout", "-b", "feat")
        self._commit(repo, run, "b")
        rc, base = self._resolve(repo, "workflow_dispatch",
                                 ref="refs/heads/feat")
        self.assertNotEqual(rc, 0)
        self.assertEqual(base, "")

    def test_pull_request_push_schedule_bases_unchanged(self):
        repo, run = self._repo()
        first = self._commit(repo, run, "a")
        head = self._commit(repo, run, "b")
        rc, base = self._resolve(repo, "pull_request", pr_base=first)
        self.assertEqual(rc, 0)
        self.assertEqual(base, first)
        rc, base = self._resolve(repo, "push", before=head)
        self.assertEqual(base, head)
        rc, base = self._resolve(repo, "schedule")
        self.assertEqual(base, first)  # HEAD^ after b

    def test_push_without_before_and_no_parent_fails(self):
        repo, run = self._repo()
        self._commit(repo, run, "a")
        rc, base = self._resolve(repo, "push", before="0" * 40)
        self.assertNotEqual(rc, 0)
        self.assertEqual(base, "")


if __name__ == "__main__":
    unittest.main()
