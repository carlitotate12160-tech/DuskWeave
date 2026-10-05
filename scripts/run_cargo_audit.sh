#!/usr/bin/env bash
# DuskWeave dependency gate: one verified cargo-audit path.
#
#   verify-archive <archive.tgz>
#       Check a cargo-audit release archive against the fixed SHA256 below.
#       Never extracts or executes the payload; used by corruption controls.
#
#   audit <project-dir> <advisory-db-dir> <evidence-dir>
#       Download the pinned release asset, verify the same fixed digest
#       before any extraction/execution, extract into an isolated tool dir,
#       assert the exact version, then audit <project-dir>/Cargo.lock against
#       the supplied advisory-db checkout. audit.json, audit.log and
#       evidence.txt are written under <evidence-dir>; the command exits with
#       cargo-audit's own status. Advisory-db or fetch failures are
#       FAILED/UNVERIFIED, never a clean audit.
set -euo pipefail

AUDIT_VERSION="0.22.2"
AUDIT_ASSET="cargo-audit-x86_64-unknown-linux-musl-v${AUDIT_VERSION}.tgz"
AUDIT_URL="https://github.com/rustsec/rustsec/releases/download/cargo-audit%2Fv${AUDIT_VERSION}/${AUDIT_ASSET}"
AUDIT_SHA256="7fb9497f8594b389e5fce5ef9b92db08432996895b2e0c5a0167a69ed445c428"
EXPECTED_VERSION="cargo-audit ${AUDIT_VERSION}"

fail() {
    echo "run_cargo_audit: $*" >&2
    exit 2
}

verify_digest() {
    local archive="$1"
    [ -f "$archive" ] || fail "archive not found: $archive"
    echo "${AUDIT_SHA256}  ${archive}" | sha256sum --check --status ||
        fail "SHA256 mismatch for ${archive}"
}

cmd_verify_archive() {
    [ $# -eq 1 ] || fail "verify-archive takes exactly one archive path"
    verify_digest "$1"
    echo "verified: $1 == ${AUDIT_SHA256}"
}

cmd_audit() {
    [ $# -eq 3 ] || fail "audit takes: <project-dir> <advisory-db-dir> <evidence-dir>"
    local project_dir="$1" advisory_db="$2" evidence_dir="$3"
    local lockfile="${project_dir}/Cargo.lock"
    [ -f "$lockfile" ] || fail "no committed Cargo.lock under ${project_dir}"
    git -C "$advisory_db" rev-parse --verify HEAD >/dev/null ||
        fail "advisory-db checkout missing/unparseable: ${advisory_db}"
    local db_rev
    db_rev="$(git -C "$advisory_db" rev-parse HEAD)"

    local tool_dir="${evidence_dir}/tool"
    mkdir -p "$tool_dir" "$evidence_dir"
    local archive="${tool_dir}/${AUDIT_ASSET}"
    curl --fail --silent --show-error --location \
        --retry 3 --retry-all-errors --connect-timeout 10 --max-time 120 \
        -o "$archive" "$AUDIT_URL" || fail "download failed: ${AUDIT_URL}"
    verify_digest "$archive"

    local extract_dir="${tool_dir}/extract"
    mkdir -p "$extract_dir"
    tar --force-local -xzf "$archive" -C "$extract_dir"
    local audit_bin
    audit_bin="$(find "$extract_dir" -name cargo-audit -type f | head -n 1)"
    [ -n "$audit_bin" ] || fail "no cargo-audit binary in verified archive"
    local actual_version
    actual_version="$("$audit_bin" --version)"
    [ "$actual_version" = "$EXPECTED_VERSION" ] ||
        fail "unexpected tool version: ${actual_version}"

    local lock_sha
    lock_sha="$(sha256sum "$lockfile" | awk '{print $1}')"
    {
        echo "tool_version=${actual_version}"
        echo "archive_sha256=${AUDIT_SHA256}"
        echo "cargo_lock_sha256=${lock_sha}"
        echo "advisory_db_rev=${db_rev}"
    } | tee "${evidence_dir}/evidence.txt"

    local status
    set +e
    "$audit_bin" audit --file "$lockfile" --db "$advisory_db" --no-fetch \
        --json > "${evidence_dir}/audit.json" 2> "${evidence_dir}/audit.log"
    status=$?
    set -e
    cat "${evidence_dir}/audit.log"
    echo "cargo-audit exit status: ${status}"
    return "$status"
}

case "${1:-}" in
    verify-archive) shift; cmd_verify_archive "$@" ;;
    audit) shift; cmd_audit "$@" ;;
    *) fail "usage: run_cargo_audit.sh {verify-archive <tgz>|audit <project> <db> <evidence>}" ;;
esac
