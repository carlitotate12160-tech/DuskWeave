"""Select worker evidence artifact IDs for the rust coverage aggregator.

Transport identity only: which uploaded ``dw-rust-worker-*`` artifacts carry
each requested worker's evidence for this checkout/run. Content judgement —
manifest, partition, object-set, receipt and profile binding — stays with
``scripts/ci_rust_shards.py``; this selector never inspects archive contents.

Worker artifacts are immutable per attempt, so a re-run of the aggregator or
of a failed worker may leave valid evidence behind under an earlier attempt.
For each requested worker the newest artifact not newer than the current
attempt is selected. A selected artifact that is expired, malformed,
duplicated or bound to another candidate fails closed; there is no fallback
to older evidence after rejection.
"""

import argparse
import json
import re
import sys
from pathlib import Path

FAMILY = "dw-rust-worker-"
NAME_RE = re.compile(r"^dw-rust-worker-(\d+)-([0-9a-f]{40})-(\d+)-(\d+)$")
SHA_RE = re.compile(r"[0-9a-f]{40}")
WORKER_COUNTS = (1, 2)


class SelectionError(Exception):
    """Invalid inputs or evidence that cannot be trusted; fail closed."""


def fail(message):
    raise SelectionError(message)


def positive_int(value, what):
    if isinstance(value, bool) or not isinstance(value, int):
        try:
            value = int(value, 10)
        except (TypeError, ValueError):
            fail(f"{what} is not an integer: {value!r}")
    if value <= 0:
        fail(f"{what} must be a positive integer: {value!r}")
    return value


def load_pages(path):
    """Slurped paginated inventory: a list of page objects with ``artifacts``."""
    try:
        data = json.loads(Path(path).read_text(encoding="utf-8"))
    except OSError as exc:
        fail(f"cannot read artifact inventory: {exc}")
    except json.JSONDecodeError as exc:
        fail(f"artifact inventory is not JSON: {exc}")
    pages = data if isinstance(data, list) else [data]
    if not pages:
        fail("artifact inventory has no pages")
    for index, page in enumerate(pages):
        if not isinstance(page, dict) or not isinstance(
                page.get("artifacts"), list):
            fail(f"inventory page {index} has no artifacts list")
    return pages


def iter_records(pages):
    """Validate record shape and ID uniqueness while streaming all pages."""
    seen_ids = set()
    for index, page in enumerate(pages):
        for record in page["artifacts"]:
            if not isinstance(record, dict):
                fail(f"inventory page {index}: artifact record not an object")
            name = record.get("name")
            if not isinstance(name, str) or not name:
                fail(f"inventory page {index}: artifact without a name")
            aid = record.get("id")
            if not isinstance(aid, int) or isinstance(aid, bool) or aid <= 0:
                fail(f"inventory page {index}: artifact {name!r} "
                     f"has invalid id {aid!r}")
            if aid in seen_ids:
                fail(f"duplicate artifact id {aid} ({name!r})")
            seen_ids.add(aid)
            yield record


def select(pages, sha, run_id, attempt, workers):
    """Newest usable (worker, attempt, id, name) per requested worker."""
    candidates = {w: {} for w in range(workers)}
    for record in iter_records(pages):
        name = record["name"]
        if not name.startswith(FAMILY):
            continue
        match = NAME_RE.fullmatch(name)
        if not match:
            fail(f"malformed worker artifact identity: {name!r}")
        worker_s, artifact_sha, artifact_run, attempt_s = match.groups()
        if artifact_sha != sha or int(artifact_run) != run_id:
            continue
        att = int(attempt_s)
        binding = record.get("workflow_run")
        if binding is not None:
            bound = binding.get("id") if isinstance(binding, dict) else None
            if positive_int(bound, "workflow_run.id") != run_id:
                fail(f"artifact {name!r}: workflow_run binding mismatch")
        worker = int(worker_s)
        if worker >= workers:
            fail(f"unassigned worker artifact for this fan-out: {name!r}")
        if att < 1 or att > attempt:
            fail(f"attempt {att} outside 1..{attempt} for this run: {name!r}")
        if att in candidates[worker]:
            fail(f"duplicate worker artifact identity: {name!r}")
        expired = record.get("expired")
        if not isinstance(expired, bool):
            fail(f"artifact {name!r}: missing or invalid expired flag")
        candidates[worker][att] = (record["id"], name, expired)
    chosen = []
    for worker in range(workers):
        if not candidates[worker]:
            fail(f"no worker {worker} evidence artifact for this "
                 "checkout/run")
        att = max(candidates[worker])
        aid, name, expired = candidates[worker][att]
        if expired:
            fail(f"selected artifact expired with no fallback: {name!r}")
        chosen.append((worker, att, aid, name))
    return chosen


def cmd_select(args):
    sha = args.sha.lower()
    if not SHA_RE.fullmatch(sha):
        fail("checkout sha must be a 40-digit lowercase hex commit")
    run_id = positive_int(args.run_id, "run id")
    attempt = positive_int(args.attempt, "run attempt")
    workers = positive_int(args.workers, "worker count")
    if workers not in WORKER_COUNTS:
        fail(f"unsupported worker count: {workers}")
    chosen = select(load_pages(args.inventory), sha, run_id, attempt, workers)
    lines = []
    for worker, att, aid, name in chosen:
        print(f"selected worker={worker} attempt={att} id={aid} name={name}")
        lines.append(f"worker_{worker}_id={aid}")
    if args.output:
        with open(args.output, "a", encoding="utf-8") as fh:
            fh.write("\n".join(lines) + "\n")
    return 0


def main(argv=None):
    parser = argparse.ArgumentParser(
        description="select worker evidence artifact IDs for the "
                    "rust coverage aggregator")
    sub = parser.add_subparsers(dest="command", required=True)
    sel = sub.add_parser("select", help="select artifact IDs per worker")
    sel.add_argument("--inventory", required=True,
                     help="slurped paginated artifacts API JSON path")
    sel.add_argument("--sha", required=True, help="actual checkout SHA")
    sel.add_argument("--run-id", required=True, help="workflow run id")
    sel.add_argument("--attempt", required=True, help="current run attempt")
    sel.add_argument("--workers", required=True, help="worker count (1 or 2)")
    sel.add_argument("--output", help="append worker_N_id lines to this file")
    sel.set_defaults(func=cmd_select)
    args = parser.parse_args(argv)
    try:
        return args.func(args)
    except SelectionError as exc:
        print(f"ci_worker_artifacts: {exc}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    sys.exit(main())
