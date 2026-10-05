"""Dependency-gate negative controls for the M0 hardening slice.

Executed in the doc-check `dependencies` job on Linux. Every tool path and
input arrives through DW_* environment variables; fixtures are disposable
directories under the system temp root. Controls exercise the real helper
script, the real deny.toml policy and the real locked project graph — a
tool/config/network error cannot satisfy a rejection control.
"""

import json
import os
import subprocess
import tempfile
import unittest
from pathlib import Path

ENV = (
    "DW_AUDIT_HELPER", "DW_AUDIT_ARCHIVE", "DW_DENY_BIN", "DW_DENY_CONFIG",
    "DW_PROJECT_ROOT", "DW_ADVISORY_DB", "DW_EVIDENCE_DIR",
)
BASH = os.environ.get("DW_BASH") or "bash"
HELPER, ARCHIVE, DENY_BIN, DENY_CONFIG, PROJECT, ADVISORY_DB, EVIDENCE = (
    str(Path(os.environ[k]).resolve()) if os.environ.get(k) else None
    for k in ENV)


def shp(p):
    """POSIX-style path for arguments handed to the bash helper."""
    return Path(p).as_posix()

VULN_LOCK = """version = 4

[[package]]
name = "vulnfixture"
version = "0.0.0"
dependencies = [
 "thread_local",
]

[[package]]
name = "thread_local"
version = "1.1.3"
source = "registry+https://github.com/rust-lang/crates.io-index"
checksum = "8018d24e04c95ac8790716a5987d0fec4f8b27249ffa0f7d33f1369bdfb88cbd"
dependencies = [
 "cfg-if",
]

[[package]]
name = "cfg-if"
version = "1.0.5"
source = "registry+https://github.com/rust-lang/crates.io-index"
checksum = "4e7648175b45a9a48536d676f68d918270699102aa8dab5496df06904c914600"
"""

def mkcrate(path, name, deps=""):
    """Disposable minimal crate dir with a real manifest and lib target."""
    path = Path(path)
    (path / "src").mkdir(parents=True)
    (path / "src" / "lib.rs").write_text("pub fn f() {}\n")
    (path / "Cargo.toml").write_text(
        f'[package]\nname = "{name}"\nversion = "0.0.0"\n'
        f'edition = "2021"\npublish = false\n{deps}')
    return path


def run(cmd, cwd=None, env=None):
    merged = dict(os.environ)
    if env:
        merged.update(env)
    return subprocess.run(cmd, cwd=cwd, env=merged, capture_output=True,
                          text=True, encoding="utf-8", errors="replace")


class DependencyGateControls(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        missing = [k for k in ENV if not os.environ.get(k)]
        if missing:
            raise RuntimeError(f"missing env: {', '.join(missing)}")
        cls.tmp = tempfile.mkdtemp(prefix="dw-depsec-")
        os.makedirs(EVIDENCE, exist_ok=True)

    def deny(self, which, cwd):
        return run(
            [DENY_BIN, "--config", DENY_CONFIG, "-L", "info", "check",
             *which.split(), "--hide-inclusion-graph"], cwd=cwd)

    def keep(self, name, proc):
        Path(EVIDENCE, name).write_text(
            f"$ {' '.join(map(str, proc.args))}\nrc={proc.returncode}\n"
            f"{proc.stdout}{proc.stderr}", encoding="utf-8")
        return proc

    def test_pinned_archive_passes_verifier(self):
        p = self.keep("verify-archive.log",
                      run([BASH, shp(HELPER), "verify-archive",
                           shp(ARCHIVE)]))
        self.assertEqual(p.returncode, 0, p.stderr)

    def test_corrupted_archive_fails_before_extraction(self):
        for name, mutate in (
            ("truncated", lambda d: d[: len(d) // 2]),
            ("modified", lambda d: d[:4096] + b"\x00" + d[4097:]),
        ):
            with self.subTest(kind=name):
                bad = Path(self.tmp, f"bad-{name}.tgz")
                bad.write_bytes(mutate(Path(ARCHIVE).read_bytes()))
                p = self.keep(f"verify-{name}.log",
                              run([BASH, shp(HELPER), "verify-archive",
                                   shp(bad)]))
                self.assertNotEqual(p.returncode, 0)
                self.assertIn("SHA256 mismatch", p.stderr)
                self.assertFalse(
                    list(Path(self.tmp).rglob("cargo-audit")),
                    "unverified bytes were extracted")

    def test_policy_accepts_project_graph(self):
        p = self.keep("deny-project.log",
                      self.deny("licenses bans sources", PROJECT))
        self.assertEqual(p.returncode, 0, p.stderr)
        for check in ("licenses", "bans", "sources"):
            self.assertIn(f"checking {check}", p.stderr)
        self.assertNotIn("error[", p.stderr)

    def test_policy_rejects_unallowed_license(self):
        fix = mkcrate(Path(self.tmp, "lic"), "licfixture",
                      '\n[dependencies]\nencoding_rs = "=0.8.35"\n')
        gen = run(["cargo", "generate-lockfile"], cwd=fix)
        self.assertEqual(gen.returncode, 0, gen.stderr)
        p = self.keep("deny-license-reject.log", self.deny("licenses", fix))
        self.assertNotEqual(p.returncode, 0)
        self.assertIn("encoding_rs", p.stderr)
        self.assertIn("BSD-3-Clause", p.stderr)

    def test_policy_rejects_git_source_offline(self):
        dep = mkcrate(Path(self.tmp, "gitdep"), "gitdep")
        run(["git", "init", "-q"], cwd=dep)
        run(["git", "add", "-A"], cwd=dep)
        run(["git", "-c", "user.email=t@t", "-c", "user.name=t",
             "commit", "-qm", "x"], cwd=dep)
        # file:// git transport is local-only: no registry dep exists, so
        # generating the lock touches no network path at all.
        fix = mkcrate(Path(self.tmp, "gitsrc"), "gitfixture",
                      f'\n[dependencies]\ngitdep = {{ git = "{dep.as_uri()}" }}\n')
        gen = run(["cargo", "generate-lockfile"], cwd=fix)
        self.assertEqual(gen.returncode, 0, gen.stderr)
        p = self.keep("deny-git-reject.log", self.deny("sources", fix))
        self.assertNotEqual(p.returncode, 0)
        self.assertIn("source-not-allowed", p.stderr)

    def test_vulnerable_lockfile_fails_through_helper(self):
        fix = Path(self.tmp, "vuln")
        ev = Path(EVIDENCE, "vuln-audit")
        fix.mkdir()
        (fix / "Cargo.lock").write_text(VULN_LOCK)
        p = self.keep("audit-vuln.log", run(
            [BASH, shp(HELPER), "audit", shp(fix), shp(ADVISORY_DB),
             shp(ev)]))
        self.assertNotEqual(p.returncode, 0, "vulnerable lockfile audited clean")
        report = json.loads((ev / "audit.json").read_text())
        hits = {(v["advisory"]["id"], v["package"]["name"])
                for v in report["vulnerabilities"]["list"]}
        self.assertIn(("RUSTSEC-2022-0006", "thread_local"), hits)
        self.assertTrue((ev / "evidence.txt").exists())

    def test_project_audits_clean_through_helper(self):
        ev = Path(EVIDENCE, "project-audit")
        p = self.keep("audit-project.log", run(
            [BASH, shp(HELPER), "audit", shp(PROJECT), shp(ADVISORY_DB),
             shp(ev)]))
        self.assertEqual(p.returncode, 0, p.stdout + p.stderr)
        report = json.loads((ev / "audit.json").read_text())
        self.assertEqual(report["vulnerabilities"]["found"], False)


if __name__ == "__main__":
    unittest.main()
