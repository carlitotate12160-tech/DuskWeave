#!/usr/bin/env python3
"""McCabe own-complexity gate for tracked DuskWeave Rust sources.

Inventories tracked src/**/*.rs via `git ls-files`, runs the pinned
rust-code-analysis-cli binary as `-p src -m -O json -o <raw> -j 1 -w`, and
evaluates every kind="function" node: own = node cyclomatic.sum minus the
immediate-child sums. Fixed caps (QUALITY_BAR.md): 7 business; the unique
reviewed pure-dispatch `run` (direct unit child of src/main.rs bound to the
reviewed source digest) keeps 10. Spans >50 emit REVIEW_TRIGGER without
failing. Exit 0 = compliance, 1 = cap violations, 2 = tool/report/input
failure.
"""

import argparse
import hashlib
import json
import os
import shutil
import subprocess
import sys

CAP_DEFAULT = 7
CAP_REVIEWED_DISPATCH = 10
REVIEW_TRIGGER_SPAN = 50
TOOL_NAME = "rust-code-analysis-cli"
REQUIRED_VERSION = "rust-code-analysis-cli 0.0.25"
DISPATCH_FILE = "src/main.rs"
DISPATCH_NAME = "run"
REVIEWED_RUN_SHA256 = (
    "558499af2367afeb051d01e4c31e82e20087d491fee936c1fa83549bd1bd6d7a"
)


class CheckError(Exception):
    """Tool, report or input failure; maps to exit code 2."""


def _run(argv):
    try:
        return subprocess.run(argv, capture_output=True)
    except OSError as exc:
        raise CheckError(f"cannot execute {argv[0]!r}: {exc}")


def _git(*args):
    proc = _run(["git", *args])
    if proc.returncode != 0:
        detail = proc.stderr.decode("utf-8", "replace").strip()
        raise CheckError(f"git {' '.join(args)} failed: {detail or proc.returncode}")
    return proc.stdout


def check_analyzer(analyzer):
    if not os.path.isabs(analyzer):
        raise CheckError("--analyzer must be an absolute path")
    if not os.path.isfile(analyzer):
        raise CheckError(f"analyzer binary not found: {analyzer}")
    proc = _run([analyzer, "--version"])
    version = proc.stdout.decode("utf-8", "replace").strip()
    if proc.returncode != 0 or version != REQUIRED_VERSION:
        raise CheckError(
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
        raise CheckError("empty tracked src/**/*.rs inventory")
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
        raise CheckError(
            f"analyzer exited {proc.returncode}: {tail[-1] if tail else 'no stderr'}"
        )


def reconcile_reports(raw_dir, files):
    """Exactly one report per inventoried source; else fail closed."""
    expected = {f"{rel}.json" for rel in files}
    found = set()
    for dirpath, _, names in os.walk(raw_dir):
        for name in names:
            if name.endswith(".json"):
                rel = os.path.relpath(os.path.join(dirpath, name), raw_dir)
                found.add(rel.replace(os.sep, "/"))
    missing, extra = sorted(expected - found), sorted(found - expected)
    if missing or extra:
        raise CheckError(
            f"report inventory mismatch: missing={missing} unexpected={extra}"
        )
    return {rel: os.path.join(raw_dir, *f"{rel}.json".split("/")) for rel in files}


def _no_dupes(pairs):
    out = {}
    for key, value in pairs:
        if key in out:
            raise CheckError(f"duplicate JSON key {key!r}")
        out[key] = value
    return out


def load_report(path):
    try:
        with open(path, "r", encoding="utf-8") as fh:
            data = json.load(fh, object_pairs_hook=_no_dupes)
    except CheckError:
        raise
    except (OSError, UnicodeDecodeError, json.JSONDecodeError) as exc:
        raise CheckError(f"invalid JSON report {path}: {exc}")
    if not isinstance(data, dict):
        raise CheckError(f"report {path} root is not an object")
    return data


def _integral_sum(node, at):
    metrics = node.get("metrics")
    cyc = metrics.get("cyclomatic") if isinstance(metrics, dict) else None
    value = cyc.get("sum") if isinstance(cyc, dict) else None
    bad = (
        isinstance(value, bool)
        or not isinstance(value, (int, float))
        or not float(value).is_integer()
        or value < 0
    )
    if bad:
        raise CheckError(f"{at}: invalid cyclomatic.sum {value!r}")
    return int(value)


def _span(node, at):
    start, end = node.get("start_line"), node.get("end_line")
    if (
        isinstance(start, bool)
        or not isinstance(start, int)
        or isinstance(end, bool)
        or not isinstance(end, int)
        or start < 1
        or end < start
    ):
        raise CheckError(f"{at}: invalid span {start}-{end}")
    return start, end


def measure_report(report, rel_src):
    """Return measured function nodes for one parsed unit report."""
    metrics = report.get("metrics")
    nom = metrics.get("nom") if isinstance(metrics, dict) else None
    total = nom.get("total") if isinstance(nom, dict) else None
    if (
        isinstance(total, bool)
        or not isinstance(total, (int, float))
        or not float(total).is_integer()
        or total < 0
    ):
        raise CheckError(f"{rel_src}: invalid metrics.nom.total {total!r}")
    spaces = report.get("spaces")
    if not isinstance(spaces, list):
        raise CheckError(f"{rel_src}: missing spaces list")
    results, identities = [], set()

    def visit(node, ancestry, index, depth):
        name, kind = node.get("name"), node.get("kind")
        if not isinstance(name, str) or not isinstance(kind, str):
            raise CheckError(f"{rel_src}: node missing string kind/name")
        ident = f"{ancestry}::{name}@{index}"
        children = node.get("spaces")
        if not isinstance(children, list):
            raise CheckError(f"{rel_src} {ident}: spaces is not a list")
        if kind == "function":
            start, end = _span(node, f"{rel_src} {ident}")
            total_cyc = _integral_sum(node, f"{rel_src} {ident}")
            child_sum = sum(
                _integral_sum(kid, f"{rel_src} {ident}") for kid in children
            )
            own = total_cyc - child_sum
            if own < 1:
                raise CheckError(
                    f"{rel_src} {ident}: own {own} < 1 (immediate children "
                    f"sum {child_sum} > node {total_cyc})"
                )
            if ident in identities:
                raise CheckError(f"{rel_src} {ident}: duplicate node identity")
            identities.add(ident)
            results.append(
                {
                    "file": rel_src,
                    "name": name,
                    "identity": f"{rel_src} {ident}",
                    "start_line": start,
                    "end_line": end,
                    "span": end - start + 1,
                    "own": own,
                    "depth": depth,
                    "cap": CAP_DEFAULT,
                    "allowance": None,
                }
            )
        for idx, kid in enumerate(children):
            visit(kid, ident, idx, depth + 1)

    for idx, top in enumerate(spaces):
        visit(top, f"{rel_src}@0", idx, 1)
    if len(results) != int(total):
        raise CheckError(
            f"{rel_src}: counted {len(results)} function nodes but "
            f"metrics.nom.total={int(total)}"
        )
    return results


def body_digest(src_path, start, end):
    try:
        with open(src_path, "rb") as fh:
            text = fh.read().decode("utf-8")
    except (OSError, UnicodeDecodeError) as exc:
        raise CheckError(f"cannot read source for digest {src_path}: {exc}")
    lines = text.replace("\r\n", "\n").replace("\r", "\n").split("\n")
    if end > len(lines):
        raise CheckError(f"{src_path}: span {start}-{end} exceeds file length")
    body = "\n".join(lines[start - 1 : end]) + "\n"
    return hashlib.sha256(body.encode("utf-8")).hexdigest()


def resolve_run_allowance(measurements, digest_of):
    """Bind cap 10 to the unique reviewed direct unit-child run only."""
    eligible = [
        m
        for m in measurements
        if m["file"] == DISPATCH_FILE
        and m["name"] == DISPATCH_NAME
        and m["depth"] == 1
    ]
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
    node = eligible[0]
    digest = digest_of(node)
    if digest == REVIEWED_RUN_SHA256:
        node["cap"] = CAP_REVIEWED_DISPATCH
        node["allowance"] = "reviewed-dispatch"
        return {"status": "granted", "identity": node["identity"], "sha256": digest}
    node["allowance"] = "digest-mismatch"
    return {
        "status": "digest_mismatch",
        "identity": node["identity"],
        "expected": REVIEWED_RUN_SHA256,
        "actual": digest,
        "detail": "run body changed since review; cap stays 7",
    }


def evaluate(measurements):
    violations = [m for m in measurements if m["own"] > m["cap"]]
    triggers = [m for m in measurements if m["span"] > REVIEW_TRIGGER_SPAN]
    return violations, triggers


def _brief(m):
    return {
        "identity": m["identity"],
        "start_line": m["start_line"],
        "end_line": m["end_line"],
        "own": m["own"],
        "cap": m["cap"],
    }


def main(argv=None):
    parser = argparse.ArgumentParser(description="McCabe complexity gate")
    parser.add_argument("--analyzer", required=True)
    parser.add_argument("--report-dir", required=True)
    args = parser.parse_args(argv)
    try:
        if not os.path.isabs(args.report_dir):
            raise CheckError("--report-dir must be an absolute path")
        version = check_analyzer(args.analyzer)
        files = inventory()
        sha = candidate_sha()
        os.makedirs(args.report_dir, exist_ok=True)
        raw_dir = os.path.join(args.report_dir, "raw")
        run_analyzer(args.analyzer, raw_dir, args.report_dir)
        report_paths = reconcile_reports(raw_dir, files)
        measurements = []
        for rel in files:
            measurements.extend(measure_report(load_report(report_paths[rel]), rel))
        allowance = resolve_run_allowance(
            measurements,
            lambda m: body_digest(m["file"], m["start_line"], m["end_line"]),
        )
        violations, triggers = evaluate(measurements)

        print(
            f"tool={version} candidate={sha or 'unknown'} "
            f"files={len(files)} functions={len(measurements)}"
        )
        for m in measurements:
            tag = " VIOLATION" if m["own"] > m["cap"] else ""
            note = f" [{m['allowance']}]" if m["allowance"] else ""
            print(
                f"{m['identity']} (lines {m['start_line']}-{m['end_line']}): "
                f"own={m['own']} cap={m['cap']}{note}{tag}"
            )
        for m in triggers:
            print(
                f"REVIEW_TRIGGER {m['identity']} span {m['span']} "
                f"> {REVIEW_TRIGGER_SPAN}"
            )
        print(f"run allowance: {allowance['status']} "
              f"{allowance.get('detail', '')}".rstrip())

        summary = {
            "tool": TOOL_NAME,
            "tool_version": "0.0.25",
            "candidate_sha": sha,
            "files": len(files),
            "functions": len(measurements),
            "violation_count": len(violations),
            "review_trigger_count": len(triggers),
            "violations": [_brief(m) for m in violations],
            "review_triggers": [_brief(m) for m in triggers],
            "run_allowance": allowance,
            "measurements": measurements,
        }
        with open(
            os.path.join(args.report_dir, "summary.json"), "w", encoding="utf-8"
        ) as fh:
            json.dump(summary, fh, indent=2)
            fh.write("\n")
        with open(
            os.path.join(args.report_dir, "summary.txt"), "w", encoding="utf-8"
        ) as fh:
            fh.write(f"tool={version} candidate={sha or 'unknown'}\n")
            fh.write(f"files={len(files)} functions={len(measurements)}\n")
            fh.write(f"violations={len(violations)}\n")
            for m in violations:
                b = _brief(m)
                fh.write(
                    f"VIOLATION {b['identity']} lines {b['start_line']}-"
                    f"{b['end_line']} own={b['own']} cap={b['cap']}\n"
                )
            fh.write(f"review_triggers={len(triggers)}\n")
            for m in triggers:
                fh.write(f"REVIEW_TRIGGER {m['identity']} span {m['span']}\n")
            fh.write(f"run_allowance={allowance['status']}\n")

        if violations:
            print(f"Failed: {len(violations)} complexity violations.")
            return 1
        print("Success: 0 complexity violations.")
        return 0
    except (CheckError, OSError) as exc:
        print(f"complexity check failed: {exc}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    sys.exit(main())
