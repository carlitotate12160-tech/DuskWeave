#!/usr/bin/env python3
"""rust-code-analysis JSON report codec, validation and projection.

Owns the report boundary for the complexity gate: discovery/reconciliation
of emitted reports, strict JSON decoding (duplicate exact keys rejected;
case-distinct keys such as Halstead n1/N1 are valid), unit schema
qualification, span validation against the real inventoried source,
recursive own-metric extraction and summary projection. CheckError is the
narrow failure type consumed by the CLI boundary.

Pinned Rust space kinds are unit, function, impl and trait; anything else is
a measurement failure. No CLI, Git, threshold or dispatch policy lives here.
"""

import json
import os

ALLOWED_KINDS = frozenset({"unit", "function", "impl", "trait"})


class CheckError(Exception):
    """Report or input failure; the CLI boundary maps it to exit 2."""


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
    except (
        OSError,
        UnicodeDecodeError,
        ValueError,
        RecursionError,
    ) as exc:
        raise CheckError(f"invalid JSON report {path}: {exc}")
    if not isinstance(data, dict):
        raise CheckError(f"report {path} root is not an object")
    return data


def _report_paths(raw_dir):
    found = set()
    for dirpath, _, names in os.walk(raw_dir):
        for name in names:
            if name.endswith(".json"):
                rel = os.path.relpath(os.path.join(dirpath, name), raw_dir)
                found.add(rel.replace(os.sep, "/"))
    return found


def reconcile_reports(raw_dir, files):
    """Exactly one report per inventoried source; else fail closed."""
    expected = {f"{rel}.json" for rel in files}
    found = _report_paths(raw_dir)
    missing, extra = sorted(expected - found), sorted(found - expected)
    if missing or extra:
        raise CheckError(
            f"report inventory mismatch: missing={missing} unexpected={extra}"
        )
    return {rel: os.path.join(raw_dir, *f"{rel}.json".split("/")) for rel in files}


def _nonneg(value, at):
    if value < 0:
        raise CheckError(f"{at}: negative value {value!r}")
    return value


def _as_count(value, at):
    """Integral nonnegative metric; ints stay ints, floats must be finite."""
    if isinstance(value, bool):
        raise CheckError(f"{at}: non-numeric value {value!r}")
    if isinstance(value, int):
        return _nonneg(value, at)
    if isinstance(value, float) and value.is_integer():
        return _nonneg(int(value), at)
    raise CheckError(f"{at}: invalid integral value {value!r}")


def _node_fields(node, at):
    if not isinstance(node, dict):
        raise CheckError(f"{at}: node is not an object")
    kind = node.get("kind")
    if not isinstance(kind, str) or kind not in ALLOWED_KINDS:
        raise CheckError(f"{at}: unrecognized kind {kind!r}")
    name = node.get("name")
    if not isinstance(name, str):
        raise CheckError(f"{at}: node name is not a string")
    children = node.get("spaces")
    if not isinstance(children, list):
        raise CheckError(f"{at}: spaces is not a list")
    return kind, name, children


def _span_values(node, at):
    start, end = node.get("start_line"), node.get("end_line")
    if (
        not isinstance(start, int)
        or isinstance(start, bool)
        or not isinstance(end, int)
        or isinstance(end, bool)
    ):
        raise CheckError(f"{at}: non-integral span {start}-{end}")
    return start, end


def _span_bounds(start, end, at, source_lines, parent_span):
    if start < 1 or end < start or end > source_lines:
        raise CheckError(
            f"{at}: span {start}-{end} outside source ({source_lines} lines)"
        )
    if parent_span and (start < parent_span[0] or end > parent_span[1]):
        raise CheckError(f"{at}: span {start}-{end} escapes parent {parent_span}")


def _source_line_count(source_path, rel_src):
    try:
        with open(source_path, "rb") as fh:
            text = fh.read().decode("utf-8").replace("\r\n", "\n")
    except (OSError, UnicodeDecodeError) as exc:
        raise CheckError(f"{rel_src}: cannot read source: {exc}")
    if not text or text.endswith("\n"):
        return text.count("\n")
    return text.count("\n") + 1


def _nom_total(report, rel_src):
    metrics = report.get("metrics")
    nom = metrics.get("nom") if isinstance(metrics, dict) else None
    value = nom.get("total") if isinstance(nom, dict) else None
    return _as_count(value, f"{rel_src} metrics.nom.total")


def _root_span(report, rel_src, nlines, children):
    start, end = _span_values(report, f"{rel_src} <root>")
    if nlines == 0:
        if children:
            raise CheckError(f"{rel_src}: empty source cannot contain nodes")
        return (0, 0)
    if start < 1 or end < start or end > nlines:
        raise CheckError(
            f"{rel_src}: root span {start}-{end} outside source ({nlines} lines)"
        )
    return (start, end)


def _qualify_root(report, rel_src, source_path):
    kind, name, children = _node_fields(report, f"{rel_src} <root>")
    if kind != "unit":
        raise CheckError(f"{rel_src}: root kind {kind!r} is not 'unit'")
    if name.replace("\\", "/") != rel_src:
        raise CheckError(f"{rel_src}: report name {name!r} != source path")
    nlines = _source_line_count(source_path, rel_src)
    return _root_span(report, rel_src, nlines, children), _nom_total(report, rel_src), nlines


def _integral_sum(node, at):
    if not isinstance(node, dict):
        raise CheckError(f"{at}: node is not an object")
    metrics = node.get("metrics")
    cyc = metrics.get("cyclomatic") if isinstance(metrics, dict) else None
    value = cyc.get("sum") if isinstance(cyc, dict) else None
    return _as_count(value, f"{at} metrics.cyclomatic.sum")


def _record_function(node, ident, rel_src, start, end, depth, children, out, seen):
    at = f"{rel_src} {ident}"
    child_sum = sum(
        _integral_sum(k, f"{at} child@{i}") for i, k in enumerate(children)
    )
    own = _integral_sum(node, at) - child_sum
    if own < 1:
        raise CheckError(
            f"{at}: own {own} < 1 (immediate children exceed node sum)"
        )
    if ident in seen:
        raise CheckError(f"{at}: duplicate node identity")
    seen.add(ident)
    out.append(
        {
            "file": rel_src,
            "name": node["name"],
            "identity": f"{rel_src} {ident}",
            "start_line": start,
            "end_line": end,
            "span": end - start + 1,
            "own": own,
            "depth": depth,
        }
    )


def measure_report(report, rel_src, source_path):
    """One unit report -> measured function nodes with own complexity."""
    root_span, total, nlines = _qualify_root(report, rel_src, source_path)
    results, seen = [], set()

    def visit(node, ancestry, index, depth, parent_span):
        at = f"{rel_src} node depth={depth} index={index}"
        if depth > 200:
            raise CheckError(f"{at}: nesting depth exceeds 200")
        kind, name, children = _node_fields(node, at)
        start, end = _span_values(node, at)
        _span_bounds(start, end, at, nlines, parent_span)
        ident = f"{ancestry}::{name}@{index}"
        if kind == "function":
            _record_function(
                node, ident, rel_src, start, end, depth, children, results, seen
            )
        for idx, kid in enumerate(children):
            visit(kid, ident, idx, depth + 1, (start, end))

    for idx, top in enumerate(report["spaces"]):
        visit(top, f"{rel_src}@0", idx, 1, root_span)
    if len(results) != total:
        raise CheckError(
            f"{rel_src}: counted {len(results)} function nodes but "
            f"metrics.nom.total={total}"
        )
    return results


def brief(m):
    return {
        "identity": m["identity"],
        "start_line": m["start_line"],
        "end_line": m["end_line"],
        "own": m["own"],
        "cap": m["cap"],
    }


def build_summary(tool, version, sha, files, measurements, violations, triggers, allowance):
    return {
        "tool": tool,
        "tool_version": version,
        "candidate_sha": sha,
        "files": len(files),
        "functions": len(measurements),
        "violation_count": len(violations),
        "review_trigger_count": len(triggers),
        "violations": [brief(m) for m in violations],
        "review_triggers": [brief(m) for m in triggers],
        "run_allowance": allowance,
        "measurements": measurements,
    }


def measurement_lines(measurements):
    for m in measurements:
        tag = " VIOLATION" if m["own"] > m["cap"] else ""
        note = f" [{m['allowance']}]" if m["allowance"] else ""
        yield (
            f"{m['identity']} (lines {m['start_line']}-{m['end_line']}): "
            f"own={m['own']} cap={m['cap']}{note}{tag}"
        )


def _txt_lines(summary):
    yield f"tool={summary['tool']} {summary['tool_version']} candidate={summary['candidate_sha'] or 'unknown'}\n"
    yield f"files={summary['files']} functions={summary['functions']}\n"
    yield f"violations={summary['violation_count']}\n"
    for m in summary["violations"]:
        yield (
            f"VIOLATION {m['identity']} lines {m['start_line']}-"
            f"{m['end_line']} own={m['own']} cap={m['cap']}\n"
        )
    yield f"review_triggers={summary['review_trigger_count']}\n"
    for m in summary["review_triggers"]:
        yield f"REVIEW_TRIGGER {m['identity']} span {m['end_line'] - m['start_line'] + 1}\n"
    yield f"run_allowance={summary['run_allowance']['status']}\n"


def write_summaries(report_dir, summary):
    with open(
        os.path.join(report_dir, "summary.json"), "w", encoding="utf-8"
    ) as fh:
        json.dump(summary, fh, indent=2)
        fh.write("\n")
    with open(
        os.path.join(report_dir, "summary.txt"), "w", encoding="utf-8"
    ) as fh:
        fh.writelines(_txt_lines(summary))
