#!/usr/bin/env python3
"""Offline diagnostic validator (DW-DIAGNOSE-M1-SCHEMA-DEADLINE).

Consumes schema-vectors.json exported by tests/m1_schema_contract.rs and
validates every synthetic proposal against the ACTUAL emitted provider
schema with the locally installed `jsonschema` package (explicit draft
resolved from the validator class; no remote $ref retrieval, no network).

Usage: python tests/support/m1_schema_validate.py <schema-vectors.json>
Writes schema-result.json beside the input (exclusive create) and prints a
safe summary. Missing dependency/input is BLOCKED, never a skipped PASS.
"""
import importlib.metadata
import json
import pathlib
import sys

try:
    import jsonschema
except ImportError:
    print(json.dumps({"result": "BLOCKED", "reason": "jsonschema_missing"}))
    sys.exit(2)


def main(argv):
    if len(argv) != 2:
        print("usage: m1_schema_validate.py <schema-vectors.json>", file=sys.stderr)
        return 2
    source = pathlib.Path(argv[1])
    if not source.is_file():
        print(json.dumps({"result": "BLOCKED", "reason": "vectors_absent"}))
        return 2
    vectors = json.loads(source.read_text(encoding="utf-8"))
    schema = vectors["schema"]
    validator_cls = jsonschema.validators.validator_for(schema)
    meta = validator_cls.META_SCHEMA
    validator = validator_cls(schema)
    cases = []
    disagreements = []
    for case in vectors["cases"]:
        errors = sorted(validator.iter_errors(case["proposal"]),
                        key=lambda e: list(e.absolute_path))
        valid = not errors
        expect = case.get("expect_schema_valid")
        agree = expect is None or expect == valid
        if not agree:
            disagreements.append(case["name"])
        cases.append({
            "name": case["name"],
            "schema_valid": valid,
            "expect_schema_valid": expect,
            "agree": agree,
            "errors": [{"validator": e.validator,
                        "path": [str(p) for p in e.absolute_path]}
                       for e in errors[:4]],
            "rust_disposition": case["rust_disposition"],
            "native_extra": case["native_extra"],
        })
    result = {
        "result": "SCHEMA_MATRIX_COMPLETE",
        "validator": {
            "package": "jsonschema",
            "version": importlib.metadata.version("jsonschema"),
            "draft_id": meta.get("$id") or meta.get("id"),
            "title": meta.get("title"),
        },
        "pack_bytes": vectors.get("pack_bytes"),
        "pack_reference_ids": vectors.get("pack_reference_ids"),
        "cases": cases,
        "summary": {
            "total": len(cases),
            "schema_valid": sum(1 for c in cases if c["schema_valid"]),
            "expectation_disagreements": disagreements,
        },
    }
    target = source.with_name("schema-result.json")
    if target.exists():
        print(json.dumps({"result": "BLOCKED",
                          "reason": "schema-result_collision"}))
        return 2
    target.write_text(json.dumps(result, indent=1), encoding="utf-8")
    print(json.dumps({"result": result["result"],
                      "validator": result["validator"],
                      "summary": result["summary"]}, indent=1))
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
