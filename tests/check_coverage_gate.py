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
PUBLIC_NAMED_FIELD = re.compile(
    r"^\s*pub(?:\((?:crate|self|super)\))?\s+[A-Za-z_]\w*\s*:\s*[^,]+,?\s*$"
)
EXEC_TOKENS = re.compile(
    r"\b(fn|impl|const|let|unsafe|async|move|for|while|loop|match|if"
    r"|else|return|break|continue|macro_rules)\b"
    r"|(?<!')\bstatic\b"
    r"|[A-Za-z_]\w*!\s*[\(\{\[]|=>"
)
LITERAL = (
    r"(?:"
    r'""|b""'
    r"|'(\\.|[^\\'])'|b'(\\.|[^\\'])'"
    r"|true|false"
    r"|-?(?:0x[0-9a-fA-F_]+|0b[01_]+|0o[0-7_]+|[0-9][0-9_]*)(?:[ui](?:8|16|32|64|128|size))?"
    r"|-?[0-9][0-9_]*\.[0-9_]+(?:[eE][+-]?[0-9_]+)?(?:f32|f64)?"
    r"|\(\)|\[\]"
    r")"
)
LITERAL_CONST = re.compile(
    r"^\s*(pub\b([^\n]*?\s+)?)?const\s+[A-Za-z_]\w*\s*:\s*[^=;]+\s*=\s*"
    + LITERAL
    + r"\s*;\s*$"
)
ABSTRACT_CONST = re.compile(
    r"^\s*(pub\b([^\n]*?\s+)?)?const\s+[A-Za-z_]\w*\s*:\s*[^=;]+\s*;\s*$"
)
ABSTRACT_FN = re.compile(
    r'^\s*(pub\b([^\n]*?\s+)?)?(async\s+)?(unsafe\s+)?(extern(\s+""|\s+[A-Za-z_]\w*)?\s+)?fn\s+[A-Za-z_]\w*(\s*<[^>]*>)?\s*\([^;{}]*\)(\s*->\s*[^;{}]+)?(\s+where\s+[^;{}]+)?\s*;\s*$'
)


def declarations_only(path):
    """True iff the file contains only Rust items that emit no coverable
    lines: attributes, mod/use declarations, type aliases, data
    declarations and their fields/variants, literal constants, and abstract
    trait declarations. Any executable construct — fn with body, trait defaults,
    executable const initializers, expressions, macro calls — even packed onto
    a shared line returns False, so the file must appear in the report.
    Uncertainty fails closed."""
    try:
        with open(path, encoding="utf-8") as fh:
            text = fh.read()
    except OSError:
        return False
    text = re.sub(r'"(?:[^"\\\n]|\\.)*"', '""', text)
    text = re.sub(r"/\*.*?\*/", "", text, flags=re.S)
    for raw in text.split("\n"):
        t = raw.split("//", 1)[0].strip()
        if not t:
            continue
        if re.match(r"^#!?\[.*\]\s*$", t):
            continue
        if t.startswith("#"):
            t = re.sub(r"^(#!?\[.*?\]\s*)+", "", t)
        if LITERAL_CONST.match(t) or ABSTRACT_CONST.match(t) or ABSTRACT_FN.match(t):
            continue
        if EXEC_TOKENS.search(t):
            return False
        if DECL_HEADER.match(t) or PUNCT_ONLY.match(t):
            continue
        if FIELD_OR_VARIANT.match(t) or PUBLIC_NAMED_FIELD.match(t):
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

    data = doc.get("data") or []
    if not data:
        return [f"report {report_path} has no coverage data"], out

    reported = {}
    for entry in data[0].get("files", []):
        rel = relativize(entry.get("filename", ""), root)
        if rel.startswith("../") or not rel.startswith("src/") or not rel.endswith(".rs"):
            problems.append(f"non-production file in report: {entry.get('filename')}")
            continue
        if not os.path.isfile(os.path.join(root, rel)):
            problems.append(f"reported file absent from source tree: {rel}")
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
            [], True,
        ))
        cases.append((
            "per-file below 80%",
            [("src/a.rs", 10, 10), ("src/b.rs", 20, 15), ("src/decl.rs", 5, 5)],
            [], False,
        ))
        cases.append((
            "total below 90%",
            [("src/a.rs", 10, 10), ("src/b.rs", 100, 79)],
            [], False,
        ))
        cases.append((
            "production source with code missing from report",
            [("src/a.rs", 10, 10)],
            [], False,
        ))
        cases.append((
            "non-production file in report",
            [("src/a.rs", 10, 10), ("src/b.rs", 10, 9),
             ("tests/registration_cli.rs", 50, 50)],
            [], False,
        ))
        cases.append((
            "missing file hiding single-line executable code",
            [("src/a.rs", 10, 10), ("src/b.rs", 10, 9)],
            [("src/sneaky.rs", "pub mod m { pub fn f() {} }\n")], False,
        ))
        cases.append((
            "reported file absent from source tree",
            [("src/a.rs", 10, 10), ("src/b.rs", 10, 9),
             ("src/ghost.rs", 10, 10)],
            [], False,
        ))
        cases.append((
            "declarations-only file with literal constants and abstract trait absent",
            [("src/a.rs", 10, 10), ("src/b.rs", 10, 9)],
            [("src/decl_const_trait.rs",
              "pub const SCHEMA: &str = \"m1\";\n"
              "pub const LIMIT: usize = 100;\n"
              "pub const FLAG: bool = true;\n"
              "pub trait Port {\n"
              "    fn call(&self) -> bool;\n"
              "    fn query(&mut self, id: u64) -> Result<(), ()>;\n"
              "}\n")],
            True,
        ))
        cases.append((
            "missing file with executable const initializer (call)",
            [("src/a.rs", 10, 10), ("src/b.rs", 10, 9)],
            [("src/exec_const_call.rs", "pub const X: i32 = calc();\n")],
            False,
        ))
        cases.append((
            "missing file with executable const block",
            [("src/a.rs", 10, 10), ("src/b.rs", 10, 9)],
            [("src/exec_const_block.rs", "pub const X: i32 = { 42 };\n")],
            False,
        ))
        cases.append((
            "missing file with trait default method body",
            [("src/a.rs", 10, 10), ("src/b.rs", 10, 9)],
            [("src/trait_default.rs", "pub trait T { fn f(&self) -> i32 { 1 } }\n")],
            False,
        ))
        cases.append((
            "missing file with multi-line trait default method",
            [("src/a.rs", 10, 10), ("src/b.rs", 10, 9)],
            [("src/trait_default_multi.rs",
              "pub trait T {\n    fn f(&self) -> i32\n    {\n        1\n    }\n}\n")],
            False,
        ))
        cases.append((
            "missing file hiding single-line code after literal const",
            [("src/a.rs", 10, 10), ("src/b.rs", 10, 9)],
            [("src/sneaky_const.rs", "pub const X: i32 = 1; pub fn f() {}\n")],
            False,
        ))
        cases.append((
            "missing file hiding single-line code after abstract fn",
            [("src/a.rs", 10, 10), ("src/b.rs", 10, 9)],
            [("src/sneaky_fn.rs", "pub trait T { fn f(&self); pub fn g() {} }\n")],
            False,
        ))
        cases.append((
            "declarations-only file with const string containing fn and comment containing fn",
            [("src/a.rs", 10, 10), ("src/b.rs", 10, 9)],
            [("src/misleading.rs",
              "pub const S: &str = \"fn sneaky() {}\";\n"
              "pub const N: u32 = 5; // pub fn f() {}\n")],
            True,
        ))
        cases.append((
            "missing file hiding executable code on attribute line",
            [("src/a.rs", 10, 10), ("src/b.rs", 10, 9)],
            [("src/sneaky_attr.rs", "#[inline] fn sneaky() { let x = 1; }\n")],
            False,
        ))
        cases.append((
            "public named fields absent from report",
            [("src/a.rs", 10, 10), ("src/b.rs", 10, 9)],
            [("src/public_fields.rs",
              "use serde::Serialize;\n"
              "#[derive(Clone, Debug, Serialize)]\n"
              "pub struct Usage {\n"
              "    input_tokens: Option<u64>,\n"
              "    pub output_tokens: Option<u64>,\n"
              "    pub(crate) reasoning_tokens: Option<u64>,\n"
              "    pub(self) response_bytes: usize,\n"
              "    pub(super) wall_time_ms: u64,\n"
              "    pub requested_model: &'static str,\n"
              "}\n"
              "#[serde(rename_all = \"snake_case\")]\n"
              "pub enum ResponseStatus { Completed, Missing, }\n")],
            True,
        ))
        cases.append((
            "public fields with a function body",
            [("src/a.rs", 10, 10), ("src/b.rs", 10, 9)],
            [("src/pub_fn.rs",
              "pub struct Usage { pub requested_model: &'static str, }\n"
              "pub fn witness() -> i32 { 1 }\n")],
            False,
        ))
        cases.append((
            "public fields with an impl block",
            [("src/a.rs", 10, 10), ("src/b.rs", 10, 9)],
            [("src/pub_impl.rs",
              "pub struct Usage { pub requested_model: &'static str, }\n"
              "impl Usage { fn ready(&self) -> bool { true } }\n")],
            False,
        ))
        cases.append((
            "public fields with a non-literal const",
            [("src/a.rs", 10, 10), ("src/b.rs", 10, 9)],
            [("src/pub_const.rs",
              "pub struct Usage { pub requested_model: &'static str, }\n"
              "pub const X: i32 = calc();\n")],
            False,
        ))
        cases.append((
            "public fields with a static",
            [("src/a.rs", 10, 10), ("src/b.rs", 10, 9)],
            [("src/pub_static.rs",
              "pub struct Usage { pub requested_model: &'static str, }\n"
              "pub static LABEL: &str = \"fixture\";\n")],
            False,
        ))
        cases.append((
            "public fields with an attribute-adjacent macro",
            [("src/a.rs", 10, 10), ("src/b.rs", 10, 9)],
            [("src/pub_macro.rs",
              "pub struct Usage { pub requested_model: &'static str, }\n"
              "#[serde(flatten)] include_str!(\"x\");\n")],
            False,
        ))
        cases.append((
            "unsupported pub(in) visibility",
            [("src/a.rs", 10, 10), ("src/b.rs", 10, 9)],
            [("src/pub_in.rs",
              "pub struct Usage {\n"
              "    pub(in crate) requested_model: &'static str,\n"
              "}\n")],
            False,
        ))

        failed = 0
        for name, entries, extra_src, expect_ok in cases:
            for rel, text in extra_src:
                write(rel, text)
            path = _fixture(root, entries)
            problems, _lines = evaluate(path, root)
            for rel, _text in extra_src:
                os.remove(os.path.join(root, rel))
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
