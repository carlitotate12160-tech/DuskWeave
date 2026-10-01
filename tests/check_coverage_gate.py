#!/usr/bin/env python3
"""Coverage gate for M0A production Rust sources.

Evaluates a cargo-llvm-cov JSON report against fixed floors:
90% total production line coverage and 80% line coverage in every
reported production source file. Only src/**/*.rs may enter the
denominator; tests, dependencies, generated code and build output are
rejected, not silently dropped. A production source missing from the
report fails the gate unless the file is declarations-only (module
declarations, uses, type aliases, data declarations that emit no
coverable lines); any doubt fails closed.

Usage:
  check_coverage_gate.py <report.json> [--root <repo>]
  check_coverage_gate.py --selftest

Thresholds are constants by design: no environment variable or CLI input
may lower them. Line coverage is not branch coverage and does not
establish correctness; the scenario assertions remain mandatory.
"""

import argparse
import json
import os
import re
import sys
import tempfile

TOTAL_FLOOR = 90
PER_FILE_FLOOR = 80

DECL_HEADER = re.compile(
    r"^\s*(pub\b([^\n]*?\s+)?)?(mod|use|type|enum|struct|union|trait)\b"
)
PUNCT_ONLY = re.compile(r"^\s*[,;}{()\[\]]+\s*$")
FIELD_OR_VARIANT = re.compile(
    r"^\s*[A-Za-z_]\w*(\s*<[^>]*>)?\s*(\([^)]*\)|\{[^}]*\}|\s*:[^,]*)?\s*,?\s*$"
)


def declarations_only(path):
    """True iff every non-comment line is a Rust item that emits no
    coverable lines: attributes, mod/use declarations, type aliases,
    data declarations and their fields/variants. Anything that could
    produce a line counter (fn, impl, static, const init, expressions,
    macro calls) returns False so the file must appear in the report."""
    try:
        with open(path, encoding="utf-8") as fh:
            lines = fh.read().split("\n")
    except OSError:
        return False
    in_block = False
    for line in lines:
        text = line.strip()
        while True:
            if in_block:
                if "*/" in text:
                    text = text.split("*/", 1)[1].strip()
                    in_block = False
                    continue
                break
            if "/*" in text:
                text = text.split("/*", 1)[0].strip()
                in_block = True
                continue
            break
        if not text or text.startswith("//"):
            continue
        if in_block:
            continue
        if text.startswith("#"):
            continue
        if DECL_HEADER.match(text) or PUNCT_ONLY.match(text):
            continue
        if FIELD_OR_VARIANT.match(text):
            continue
        return False
    return True


def iter_production(root):
    out = []
    src = os.path.join(root, "src")
    for dirpath, _dirs, files in os.walk(src):
        for name in files:
            if name.endswith(".rs"):
                rel = os.path.relpath(os.path.join(dirpath, name), root)
                out.append(rel.replace(os.sep, "/"))
    return sorted(out)


def relativize(filename, root):
    path = filename.replace("\\", "/")
    if not os.path.isabs(path):
        path = os.path.join(root, path)
    rel = os.path.relpath(path, root).replace(os.sep, "/")
    return rel


def evaluate(report_path, root):
    """Returns (problems, lines) — problems is non-empty on failure."""
    problems, out = [], []
    try:
        with open(report_path, encoding="utf-8") as fh:
            doc = json.load(fh)
    except (OSError, json.JSONDecodeError) as exc:
        return [f"cannot read report {report_path}: {exc}"], out

    reported = {}
    for entry in doc.get("data", [{}])[0].get("files", []):
        rel = relativize(entry.get("filename", ""), root)
        if rel.startswith("../") or not rel.startswith("src/") or not rel.endswith(".rs"):
            problems.append(f"non-production file in report: {entry.get('filename')}")
            continue
        summ = entry.get("summary", {}).get("lines", {})
        reported[rel] = (int(summ.get("count", 0)), int(summ.get("covered", 0)))

    for rel in iter_production(root):
        if rel not in reported:
            full = os.path.join(root, rel)
            if declarations_only(full):
                out.append(f"{rel:<30} n/a (no coverable lines)")
            else:
                problems.append(f"production source missing from report: {rel}")

    total_count = total_covered = 0
    for rel in sorted(reported):
        count, covered = reported[rel]
        total_count += count
        total_covered += covered
        if count == 0:
            out.append(f"{rel:<30} n/a (0 counted lines)")
            continue
        pct = covered * 100.0 / count
        mark = "" if covered * 100 >= PER_FILE_FLOOR * count else "  <-- below 80% file floor"
        if mark:
            problems.append(f"{rel}: {pct:.2f}% below {PER_FILE_FLOOR}% per-file floor")
        out.append(f"{rel:<30} {pct:6.2f}%  ({covered}/{count}){mark}")

    if total_count == 0:
        problems.append("report contains no counted production lines")
        total_pct = 0.0
    else:
        total_pct = total_covered * 100.0 / total_count
        if total_covered * 100 < TOTAL_FLOOR * total_count:
            problems.append(
                f"total {total_pct:.2f}% below {TOTAL_FLOOR}% production floor"
            )
    out.append(f"{'TOTAL':<30} {total_pct:6.2f}%  ({total_covered}/{total_count})")
    return problems, out


DECL_SRC = (
    "#![forbid(unsafe_code)]\n//! declarations only\n"
    "pub mod a;\n#[derive(Debug)]\npub enum E {\n    A(&'static str),\n}\n"
    "pub type R<T> = Result<T, E>;\n"
)
CODE_SRC = "pub fn f() -> i32 {\n    1\n}\n"


def _fixture(root, entries):
    files = [
        {
            "filename": name if os.path.isabs(name) else os.path.join(root, name),
            "summary": {"lines": {"count": c, "covered": v,
                                  "percent": (v * 100.0 / c) if c else 0.0}},
        }
        for name, c, v in entries
    ]
    doc = {"data": [{"files": files, "totals": {}}]}
    path = os.path.join(root, "report.json")
    with open(path, "w", encoding="utf-8") as fh:
        json.dump(doc, fh)
    return path


def selftest():
    """Regression check for the gate itself using synthetic fixtures.
    Fixtures are gate inputs only; they are not application coverage."""
    with tempfile.TemporaryDirectory() as root:
        os.makedirs(os.path.join(root, "src"))
        cases = []

        def write(rel, text):
            with open(os.path.join(root, rel), "w", encoding="utf-8") as fh:
                fh.write(text)

        write("src/a.rs", CODE_SRC)
        write("src/b.rs", CODE_SRC)
        write("src/decl.rs", DECL_SRC)

        cases.append((
            "meets both floors; declarations-only file absent",
            [("src/a.rs", 10, 10), ("src/b.rs", 10, 9)],
            True,
        ))
        cases.append((
            "per-file below 80%",
            [("src/a.rs", 10, 10), ("src/b.rs", 20, 15), ("src/decl.rs", 5, 5)],
            False,
        ))
        cases.append((
            "total below 90%",
            [("src/a.rs", 10, 10), ("src/b.rs", 100, 79)],
            False,
        ))
        cases.append((
            "production source with code missing from report",
            [("src/a.rs", 10, 10)],
            False,
        ))
        cases.append((
            "non-production file in report",
            [("src/a.rs", 10, 10), ("src/b.rs", 10, 9),
             ("tests/registration_cli.rs", 50, 50)],
            False,
        ))

        failed = 0
        for name, entries, expect_ok in cases:
            path = _fixture(root, entries)
            problems, _lines = evaluate(path, root)
            ok = not problems
            status = "ok" if ok == expect_ok else "UNEXPECTED"
            if ok != expect_ok:
                failed += 1
            print(f"selftest[{name}]: expected={'pass' if expect_ok else 'fail'} "
                  f"got={'pass' if ok else 'fail'} -> {status}")
        print(f"selftest: {len(cases) - failed}/{len(cases)} cases as expected")
        return 1 if failed else 0


def main():
    p = argparse.ArgumentParser()
    p.add_argument("report", nargs="?")
    p.add_argument("--root", default=None)
    p.add_argument("--selftest", action="store_true")
    a = p.parse_args()
    if a.selftest:
        return selftest()
    if not a.report:
        p.error("report path required")
    root = a.root or os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
    problems, lines = evaluate(a.report, root)
    for line in lines:
        print(line)
    for prob in problems:
        print(f"FAIL  {prob}")
    if problems:
        return 1
    print(f"coverage gate OK: total >= {TOTAL_FLOOR}%, every reported "
          f"production file >= {PER_FILE_FLOOR}%")
    return 0


if __name__ == "__main__":
    sys.exit(main())
