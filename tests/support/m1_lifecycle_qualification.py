"""Qualify the fixed Windows lifecycle fixture plan in one credential lifetime."""
import argparse
import json
import os
from pathlib import Path
import platform
import re
import subprocess
import sys

import m1_native_bootstrap as bootstrap

NORMAL = "D:/DuskWeave-build/rust-1.94.1/normal"
COVERAGE = "D:/DuskWeave-build/rust-1.94.1/coverage/inference-lifecycle"
PLAN = (
    ("ordinary_binary", "test", "--bin", "duskweave", 57, NORMAL),
    ("ordinary_lifecycle", "test", "--test", "m1_inference_lifecycle", 18, NORMAL),
    ("instrumented_binary", "llvm-cov", "--bin", "duskweave", 57, COVERAGE),
    ("instrumented_lifecycle", "llvm-cov", "--test", "m1_inference_lifecycle", 18, COVERAGE),
)
SETUP_CATEGORIES = frozenset((
    "native_identifier_refused", "native_database_collision", "native_login_collision",
    "native_service_unqualified", "qualification_report_refused", "qualification_platform_refused",
    "qualification_tool_unavailable", "qualification_prerequisite_failed",
    "qualification_toolchain_refused", "qualification_source_dirty", "qualification_service_refused",
))


def setup_category(error):
    category = str(error)
    return category if category in SETUP_CATEGORIES else "qualification_setup_failed"


def validate_report(report):
    """Refuse collisions and unsafe locations before any fixture provisioning."""
    if not report.is_absolute() or report.exists():
        raise RuntimeError("qualification_report_refused")
    for parent in (report, *report.parents):
        if parent.is_symlink() or os.path.isjunction(parent):
            raise RuntimeError("qualification_report_refused")
    resolved = report.resolve()
    for protected in (bootstrap.ROOT, Path(NORMAL), Path(COVERAGE)):
        if resolved.is_relative_to(protected.resolve()) or protected.resolve().is_relative_to(resolved):
            raise RuntimeError("qualification_report_refused")


def probe(args):
    try:
        result = subprocess.run(args, cwd=bootstrap.ROOT, capture_output=True, text=True, timeout=60)
    except (OSError, subprocess.SubprocessError):
        raise RuntimeError("qualification_tool_unavailable") from None
    if result.returncode:
        raise RuntimeError("qualification_prerequisite_failed")
    return result.stdout.strip()


def supervisor_prerequisite():
    """The development supervisor helper is a required qualification tool."""
    path = os.environ.get(bootstrap.SUPERVISOR_VARIABLE, "")
    if not path or not Path(path).is_absolute():
        raise RuntimeError("qualification_tool_unavailable")
    return path


def preflight():
    """Closed Windows/toolchain/service checks; never export container configuration."""
    if sys.platform != "win32" or platform.machine().lower() not in ("amd64", "x86_64"):
        raise RuntimeError("qualification_platform_refused")
    rust = probe(["rustc", "+1.94.1", "-vV"])
    cov = probe(["cargo", "+1.94.1", "llvm-cov", "--version"])
    if "release: 1.94.1\n" not in rust or "host: x86_64-pc-windows-msvc\n" not in rust:
        raise RuntimeError("qualification_toolchain_refused")
    if cov != "cargo-llvm-cov 0.9.1":
        raise RuntimeError("qualification_toolchain_refused")
    head = probe(["git", "rev-parse", "HEAD"])
    if probe(["git", "status", "--porcelain", "--untracked-files=normal"]):
        raise RuntimeError("qualification_source_dirty")
    supervisor = supervisor_prerequisite()
    bootstrap.service_qualified("dwpg-r3", version_exact="17.11")
    return {"head": head, "platform": sys.platform, "architecture": platform.machine(),
            "rust": rust.splitlines()[0], "host": "x86_64-pc-windows-msvc", "llvm_cov": cov,
            "python": platform.python_version(), "postgres": "17.11", "image": bootstrap.IMAGE,
            "ordinary_target": NORMAL, "instrumented_target": COVERAGE,
            "fixture_supervisor": supervisor,
            "harness_ceiling_seconds": 900, "policy": "conservative supervision; not measured fit"}


def action_environment(fixture, target):
    for name in ("DEEPSEEK_API_KEY", "DW_TEST_READINESS_OBSERVE", "DW_TEST_RUN_BINARY",
                 "CARGO_LLVM_COV_TARGET_DIR", "LLVM_PROFILE_FILE"):
        fixture.env.pop(name, None)
    fixture.env["CARGO_TARGET_DIR"] = target
    fixture.env["DW_TEST_RUN_BINARY"] = str(Path(target) / "debug/duskweave.exe")
    fixture.env["DW_TEST_PYTHON"] = sys.executable
    supervisor = os.environ.get(bootstrap.SUPERVISOR_VARIABLE)
    if supervisor:
        fixture.env[bootstrap.SUPERVISOR_VARIABLE] = supervisor
    if target == COVERAGE:
        fixture.env["CARGO_LLVM_COV_TARGET_DIR"] = target


def action_result(report, label, code, expected):
    metadata = json.loads((report / f"{label}.result.json").read_text(encoding="utf-8"))
    status = metadata.get("status")
    if status == "harness_timeout":
        return "harness_timeout", code or 1
    if code:
        return "action_failed", code
    if status != "completed" or metadata.get("exit") != 0:
        return "action_failed", 1
    output = (report / f"{label}.log").read_text(encoding="utf-8")
    summaries = re.findall(r"^test result: (\w+)\. (\d+) passed; (\d+) failed; (\d+) ignored; "
                           r"(\d+) measured; (\d+) filtered out;", output, re.M)
    full = ("ok", str(expected), "0", "0", "0", "0")
    # T8 emits a real nested one-test summary; it cannot replace the full suite.
    if summaries.count(full) != 1 or any(s[0] != "ok" or s[2] != "0" for s in summaries):
        return "summary_refused", 1
    return "PASS", 0


SNAPSHOT = """
BEGIN READ ONLY;
SET LOCAL statement_timeout='5s';
SET LOCAL lock_timeout='500ms';
SELECT coalesce(jsonb_agg(jsonb_build_object(
 'run_id', r.run_id,
 'decisions',(SELECT count(*) FROM cognition.reasoning_decisions d
   WHERE (d.e,d.c)=(r.e,r.c) AND d.d IN(r.decision_one,r.decision_two)),
 'effects',(SELECT count(*) FROM execution.fixture_dns_results f WHERE f.run_id=r.run_id),
 'diagnostics',(SELECT coalesce(jsonb_agg(jsonb_build_object(
   'phase', CASE WHEN phase IN('decision_one','decision_two','acquisition') THEN phase ELSE 'unknown' END,
   'cause', CASE WHEN payload->>'cause' IN('child_error','child_exit','readiness_refused',
     'readiness_deadline','phase_deadline','local_stop','supervisor_unavailable','unavailable')
     THEN payload->>'cause' ELSE 'unknown' END,
   'reported_guard', CASE WHEN payload->>'reported_guard' IN('transport_hard_deadline',
     'decision_envelope','permit_expiry','command_watchdog') THEN payload->>'reported_guard' END,
   'readiness_observed', payload->'readiness_observed',
   'exit_code', payload->'exit_code', 'elapsed_ms', payload->'elapsed_ms') ORDER BY phase),'[]')
   FROM execution.run_phase_diagnostics x WHERE x.run_id=r.run_id)) ORDER BY r.run_id),'[]')
FROM execution.autonomous_runs r;
COMMIT;
"""


def snapshot(fixture, report):
    """One best-effort read through existing provisioning access; no replay/polling."""
    try:
        raw = fixture.sql(SNAPSHOT)
        # psql command tags can surround the single JSON row.
        rows = [line for line in raw.splitlines() if line.startswith("[")]
        data = json.loads(rows[0])
        (report / "failure-snapshot.json").write_text(json.dumps(data, indent=2), encoding="utf-8")
        return "available"
    except (RuntimeError, OSError, ValueError, IndexError, subprocess.SubprocessError):
        return "unavailable"


def write_result(report, result):
    (report / "qualification.json").write_text(json.dumps(result, indent=2), encoding="utf-8")


def qualify(database, report):
    """One fixture, fixed serial plan, original failure and NOT_RUN remainder."""
    try:
        bootstrap.database_name("fixture", database)
        validate_report(report)
        environment = preflight()
        report.mkdir(parents=True, exist_ok=False)
    except (RuntimeError, OSError, ValueError, KeyError, subprocess.SubprocessError) as error:
        print(json.dumps({"status": "setup_failed", "category": setup_category(error)}), flush=True)
        return 1
    result = {"status": "NOT_READY", "database": database, "environment": environment,
              "actions": [{"action": p[0], "status": "NOT_RUN"} for p in PLAN]}
    write_result(report, result)
    try:
        fixture = bootstrap.Fixture("dwpg-r3", "fixture", database=database, schema_revision="0021")
    except (RuntimeError, OSError, ValueError, KeyError, subprocess.SubprocessError) as error:
        result.update(status="setup_failed", category=setup_category(error))
        write_result(report, result)
        return 1
    for index, (label, mode, selector, name, expected, target) in enumerate(PLAN):
        action_environment(fixture, target)
        args = ["cargo", "+1.94.1", mode, "--locked"]
        if mode == "llvm-cov":
            args += ["--no-report"]
        args += [selector, name, "--", "--test-threads=1", "--nocapture"]
        try:
            code = fixture.run(label, args, report, max_duration_seconds=900)
            status, code = action_result(report, label, code, expected)
        except subprocess.TimeoutExpired:
            # A defensive legacy timeout must retain harness_timeout attribution;
            # a bare exit 124 from the child alone proves no product guard.
            status, code = "harness_timeout", 124
        except (RuntimeError, OSError, ValueError, subprocess.SubprocessError):
            status, code = "action_failed", 1
        result["actions"][index].update(status=status, exit=code)
        if code:
            result["snapshot"] = snapshot(fixture, report)
            write_result(report, result)
            return code
        write_result(report, result)
    result["status"] = "PASS"
    write_result(report, result)
    return 0


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--database-name", required=True)
    parser.add_argument("--report", type=Path, required=True)
    options = parser.parse_args()
    return qualify(options.database_name, options.report)


if __name__ == "__main__":
    raise SystemExit(main())
