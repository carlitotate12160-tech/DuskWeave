#!/usr/bin/env python3
"""McCabe own-complexity gate for tracked DuskWeave Rust sources.

CLI owner of the gate: pinned analyzer qualification, tracked src/**/*.rs
inventory via `git ls-files`, analyzer invocation
(`-p src -m -O json -o <raw> -j 1 -w`), candidate provenance, fixed cap
assignment and the reviewed dispatch binding. Report codec, validation and
projection live in sibling `complexity_reports.py`.

Fixed caps (QUALITY_BAR.md): 7 business; the unique reviewed pure-dispatch
`run` (direct unit child of src/main.rs bound to the reviewed source digest)
keeps 10. Spans >50 emit REVIEW_TRIGGER without failing. Exit 0 = compliance,
1 = cap violations, 2 = tool/report/input failure.
"""

import argparse
import hashlib
import os
import shutil
import subprocess
import sys

import complexity_reports as reports

CAP_DEFAULT = 7
CAP_REVIEWED_DISPATCH = 10
REVIEW_TRIGGER_SPAN = 50
SUBPROCESS_TIMEOUT_SECONDS = 300.0
TOOL_NAME = "rust-code-analysis-cli"
REQUIRED_VERSION = "rust-code-analysis-cli 0.0.25"
DISPATCH_FILE = "src/main.rs"
DISPATCH_NAME = "run"
REVIEWED_RUN_SHA256 = (
    "64881a442ba29203316643f66e86a1981eb1bfc66d299d17065fb314fdd00740"
)


def _subprocess_timeout():
    """Bounded wait for analyzer/git children; overridable for tests."""
    raw = os.environ.get("DW_COMPLEXITY_TIMEOUT")
    if raw is None:
        return SUBPROCESS_TIMEOUT_SECONDS
    try:
        value = float(raw)
    except ValueError:
        raise reports.CheckError(
            f"DW_COMPLEXITY_TIMEOUT must be a number of seconds, got {raw!r}"
        ) from None
    if not 0 < value <= SUBPROCESS_TIMEOUT_SECONDS:
        raise reports.CheckError(
            f"DW_COMPLEXITY_TIMEOUT must be within "
            f"(0, {SUBPROCESS_TIMEOUT_SECONDS:g}] seconds, got {value!r}"
        )
    return value


def _run(argv):
    try:
        return subprocess.run(
            argv, capture_output=True, timeout=_subprocess_timeout()
        )
    except subprocess.TimeoutExpired as exc:
        raise reports.CheckError(
            f"{argv[0]!r} timed out after {exc.timeout}s"
        ) from exc
    except OSError as exc:
        raise reports.CheckError(f"cannot execute {argv[0]!r}: {exc}") from exc


def _git(*args):
    proc = _run(["git", *args])
    if proc.returncode != 0:
        detail = proc.stderr.decode("utf-8", "replace").strip()
        raise reports.CheckError(
            f"git {' '.join(args)} failed: {detail or proc.returncode}"
        )
    return proc.stdout


def _require_absolute(path, flag):
    if not os.path.isabs(path):
        raise reports.CheckError(f"{flag} must be an absolute path")


def check_analyzer(analyzer):
    _require_absolute(analyzer, "--analyzer")
    if not os.path.isfile(analyzer):
        raise reports.CheckError(f"analyzer binary not found: {analyzer}")
    proc = _run([analyzer, "--version"])
    version = proc.stdout.decode("utf-8", "replace").strip()
    if proc.returncode != 0 or version != REQUIRED_VERSION:
        raise reports.CheckError(
            f"analyzer --version must be exactly {REQUIRED_VERSION!r}, "
            f"got {version!r} (exit {proc.returncode})"
        )
    return version


def inventory():
    out = _git("ls-files", "-z", "--", "src/*.rs", "src/**/*.rs")
    files = sorted(
        {
            p.replace("\\", "/")
            for p in out.decode("utf-8", "replace").split("\0")
            if p.startswith("src/") and p.endswith(".rs")
        }
    )
    if not files:
        raise reports.CheckError("empty tracked src/**/*.rs inventory")
    return files


def candidate_sha():
    proc = _run(["git", "rev-parse", "--verify", "HEAD"])
    if proc.returncode != 0:
        return None
    return proc.stdout.decode("ascii", "replace").strip() or None


def run_analyzer(analyzer, raw_dir, diag_dir):
    if os.path.exists(raw_dir):
        shutil.rmtree(raw_dir)
    os.makedirs(raw_dir)
    proc = _run(
        [analyzer, "-p", "src", "-m", "-O", "json", "-o", raw_dir, "-j", "1", "-w"]
    )
    for name, data in (
        ("analyzer.stdout.log", proc.stdout),
        ("analyzer.stderr.log", proc.stderr),
    ):
        with open(os.path.join(diag_dir, name), "wb") as fh:
            fh.write(data)
    if proc.returncode != 0:
        tail = proc.stderr.decode("utf-8", "replace").strip().splitlines()
        raise reports.CheckError(
            f"analyzer exited {proc.returncode}: {tail[-1] if tail else 'no stderr'}"
        )


def _measure_all(raw_dir, files):
    report_paths = reports.reconcile_reports(raw_dir, files)
    measurements = []
    for rel in files:
        report = reports.load_report(report_paths[rel])
        for m in reports.measure_report(report, rel, rel):
            m["cap"] = CAP_DEFAULT
            m["allowance"] = None
            measurements.append(m)
    return measurements


def body_digest(src_path, start, end):
    try:
        with open(src_path, "rb") as fh:
            text = fh.read().decode("utf-8")
    except (OSError, UnicodeDecodeError) as exc:
        raise reports.CheckError(
            f"cannot read source for digest {src_path}: {exc}"
        )
    lines = text.replace("\r\n", "\n").replace("\r", "\n").split("\n")
    if end > len(lines):
        raise reports.CheckError(
            f"{src_path}: span {start}-{end} exceeds file length"
        )
    body = "\n".join(lines[start - 1 : end]) + "\n"
    return hashlib.sha256(body.encode("utf-8")).hexdigest()


def _eligible_runs(measurements):
    return [
        m
        for m in measurements
        if m["file"] == DISPATCH_FILE
        and m["name"] == DISPATCH_NAME
        and m["depth"] == 1
    ]


def _bind_reviewed(node, digest_of):
    digest = digest_of(node)
    if digest == REVIEWED_RUN_SHA256:
        node["cap"] = CAP_REVIEWED_DISPATCH
        node["allowance"] = "reviewed-dispatch"
        return {
            "status": "granted",
            "identity": node["identity"],
            "sha256": digest,
        }
    node["allowance"] = "digest-mismatch"
    return {
        "status": "digest_mismatch",
        "identity": node["identity"],
        "expected": REVIEWED_RUN_SHA256,
        "actual": digest,
        "detail": "run body changed since review; cap stays 7",
    }


def resolve_run_allowance(measurements, digest_of):
    """Bind cap 10 to the unique reviewed direct unit-child run only."""
    eligible = _eligible_runs(measurements)
    if not eligible:
        return {
            "status": "absent",
            "detail": "no direct unit-child function run in src/main.rs",
        }
    if len(eligible) > 1:
        return {
            "status": "ambiguous",
            "detail": f"{len(eligible)} unit-child run candidates in "
            f"{DISPATCH_FILE}; none receive cap {CAP_REVIEWED_DISPATCH}",
        }
    return _bind_reviewed(eligible[0], digest_of)


def evaluate(measurements):
    violations = [m for m in measurements if m["own"] > m["cap"]]
    triggers = [m for m in measurements if m["span"] > REVIEW_TRIGGER_SPAN]
    return violations, triggers


def _emit(version, sha, files, measurements, triggers, allowance):
    print(
        f"tool={version} candidate={sha or 'unknown'} "
        f"files={len(files)} functions={len(measurements)}"
    )
    for line in reports.measurement_lines(measurements):
        print(line)
    for m in triggers:
        print(
            f"REVIEW_TRIGGER {m['identity']} span {m['span']} "
            f"> {REVIEW_TRIGGER_SPAN}"
        )
    detail = allowance.get("detail", "")
    print(f"run allowance: {allowance['status']} {detail}".rstrip())


def main(argv=None):
    parser = argparse.ArgumentParser(description="McCabe complexity gate")
    parser.add_argument("--analyzer", required=True)
    parser.add_argument("--report-dir", required=True)
    args = parser.parse_args(argv)
    try:
        _require_absolute(args.report_dir, "--report-dir")
        version = check_analyzer(args.analyzer)
        files = inventory()
        sha = candidate_sha()
        os.makedirs(args.report_dir, exist_ok=True)
        raw_dir = os.path.join(args.report_dir, "raw")
        run_analyzer(args.analyzer, raw_dir, args.report_dir)
        measurements = _measure_all(raw_dir, files)
        allowance = resolve_run_allowance(
            measurements,
            lambda m: body_digest(m["file"], m["start_line"], m["end_line"]),
        )
        violations, triggers = evaluate(measurements)
        _emit(version, sha, files, measurements, triggers, allowance)
        summary = reports.build_summary(
            TOOL_NAME, "0.0.25", sha, files, measurements,
            violations, triggers, allowance,
        )
        reports.write_summaries(args.report_dir, summary)
        if violations:
            print(f"Failed: {len(violations)} complexity violations.")
            return 1
        print("Success: 0 complexity violations.")
        return 0
    except (reports.CheckError, OSError) as exc:
        print(f"complexity check failed: {exc}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    sys.exit(main())
