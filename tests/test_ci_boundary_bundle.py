"""Focused real-validator tests for install_bundle bundle/custody checks.

The driver sources the real qualify.sh (which sources deployment_custody.sh),
builds a complete seven-member synthetic bundle fixture, registers test-local
root ownership/modes and invokes the ACTUAL install_bundle validator with only
install_release replaced by a spy. Every negative leg asserts the exact closed
refusal token and proves the installer spy ran zero times. The pinned digest
map is replaced by fixture digests so sha256sum stays the real consumer; the
production map is proven read-only against the real frozen bundle on Oracle.
Requires Bash on Linux/WSL.
"""
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]
QUALIFY = ROOT / "boundary" / "qualify" / "qualify.sh"
MEMBERS = ("duskweave", "dw-boundary-launcher", "dw-dns-capture",
           "dw-dns-exporter", "third_party/genai/LICENSE-APACHE",
           "third_party/genai/LICENSE-MIT",
           "third_party/genai/source-manifest.json")

DRIVER = r"""set -euo pipefail
. "$QUALIFY"   # real script + custody module; sourcing must not dispatch

LOG=$(mktemp); ERR=$(mktemp); STAT_REGISTRY=$(mktemp)
trap 'rm -f "$LOG" "$ERR" "$STAT_REGISTRY"' EXIT
RC=0; ERRTXT=""
fail() { printf 'FAIL %s\n' "$*" >&2; exit 1; }
note() { printf '%s\n' "$*" >>"$LOG"; }

stat() {
    if [[ $1 == -c && -f $STAT_REGISTRY ]]; then
        local fmt=$2 path=$3 meta u g m n
        meta=$(awk -F'|' -v p="$path" '$1 == p { v=$2 } END { print v }' "$STAT_REGISTRY")
        if [[ -n $meta ]]; then
            IFS=: read -r u g m n <<<"$meta"
            case $fmt in
                %u) echo "$u"; return ;; %g) echo "$g"; return ;;
                %a) echo "$m"; return ;; %h) echo "$n"; return ;;
            esac
        fi
    fi
    command stat "$@"
}
register() {
    local p=$1 u=${2:-0} g=${3:-0} m n
    m=${4:-$(command stat -c %a "$p")}; n=${5:-$(command stat -c %h "$p")}
    printf '%s|%s:%s:%s:%s\n' "$p" "$u" "$g" "$m" "$n" >>"$STAT_REGISTRY"
}
register_ancestors() {
    local p=$1
    while [[ $p == /tmp/?* ]]; do register "$p"; p=${p%/*}; done
    register /tmp 0 0 755 1
}

install_release() { note "install_release${1:+ $1}"; }

BUNDLE=""
build_fixture() {
    BUNDLE=$(mktemp -d)/bundle
    mkdir -p "$BUNDLE/third_party/genai"
    local name
    for name in duskweave dw-boundary-launcher dw-dns-capture dw-dns-exporter; do
        printf 'fixture-%s-bytes\n' "$name" > "$BUNDLE/$name"
    done
    printf 'fixture-apache-2.0\n' > "$BUNDLE/third_party/genai/LICENSE-APACHE"
    printf 'fixture-mit\n' > "$BUNDLE/third_party/genai/LICENSE-MIT"
    printf '{"source":"fixture"}\n' > "$BUNDLE/third_party/genai/source-manifest.json"
    register_ancestors "$BUNDLE"
    while IFS= read -r entry; do register "$entry"; done < <(find "$BUNDLE" -mindepth 1 | sort)
    DEPLOYMENT_BUNDLE_SUMS=$(cd -- "$BUNDLE" && sha256sum duskweave \
        dw-boundary-launcher dw-dns-capture dw-dns-exporter \
        third_party/genai/LICENSE-APACHE third_party/genai/LICENSE-MIT \
        third_party/genai/source-manifest.json)
}
run_bundle() {
    : >"$ERR"; ERRTXT=""
    if ( install_bundle "$BUNDLE" ) 2>"$ERR"; then RC=0; else RC=$?; fi
    ERRTXT=$(cat -- "$ERR")
}
expect_refused() {
    [[ $RC != 0 ]] || fail "unexpected_success"
    [[ $ERRTXT == "$1" ]] || fail "stderr=$ERRTXT expected=$1"
    expect_quiet
}
expect_quiet() { [[ ! -s $LOG ]] || fail "installer_reached:$(cat "$LOG")"; }

case_valid_bundle_installs_once() {
    build_fixture
    run_bundle
    [[ $RC == 0 ]] || fail "rc=$RC err=$ERRTXT"
    [[ $(cat "$LOG") == "install_release $BUNDLE" ]] || fail "log:$(cat "$LOG")"
}

case_extra_top_level_file_refused() {
    build_fixture
    : > "$BUNDLE/stray"
    register "$BUNDLE/stray"
    run_bundle; expect_refused bundle_membership_refused
}
case_extra_top_level_dir_refused() {
    build_fixture
    mkdir "$BUNDLE/extra"
    register "$BUNDLE/extra"
    run_bundle; expect_refused bundle_membership_refused
}
case_nested_extra_refused() {
    build_fixture
    : > "$BUNDLE/third_party/genai/EXTRA.md"
    register "$BUNDLE/third_party/genai/EXTRA.md"
    run_bundle; expect_refused bundle_membership_refused
}
case_hidden_entry_refused() {
    build_fixture
    : > "$BUNDLE/.hidden"
    register "$BUNDLE/.hidden"
    run_bundle; expect_refused bundle_membership_refused
}
case_empty_extra_dir_refused() {
    build_fixture
    mkdir "$BUNDLE/third_party/empty"
    register "$BUNDLE/third_party/empty"
    run_bundle; expect_refused bundle_membership_refused
}
case_missing_member_refused() {
    build_fixture
    rm -- "$BUNDLE/dw-dns-exporter"
    run_bundle; expect_refused bundle_member_refused
}
case_symlink_member_refused() {
    build_fixture
    rm -- "$BUNDLE/duskweave"
    ln -s dw-boundary-launcher "$BUNDLE/duskweave"
    run_bundle; expect_refused bundle_member_refused
}
case_symlink_dir_refused() {
    # A symlinked member directory fails custody before membership runs.
    build_fixture
    rm -r "$BUNDLE/third_party"
    ln -s /tmp "$BUNDLE/third_party"
    run_bundle; expect_refused bundle_refused
}
case_wrong_member_uid_refused() {
    build_fixture
    register "$BUNDLE/duskweave" 1000 0
    run_bundle; expect_refused bundle_member_refused
}
case_unsafe_bundle_dir_mode_refused() {
    build_fixture
    register "$BUNDLE/third_party" 0 0 775
    run_bundle; expect_refused bundle_refused
}
case_unsafe_member_mode_refused() {
    build_fixture
    register "$BUNDLE/dw-dns-capture" 0 0 664
    run_bundle; expect_refused bundle_member_refused
}
case_hardlinked_member_refused() {
    build_fixture
    ln "$BUNDLE/dw-dns-capture" "$(dirname "$BUNDLE")/capture-link"
    register "$BUNDLE/dw-dns-capture" 0 0 555 2
    run_bundle; expect_refused bundle_member_refused
}
case_unsafe_ancestor_refused() {
    build_fixture
    register "$(dirname "$BUNDLE")" 0 0 775
    run_bundle; expect_refused bundle_refused
}
case_symlinked_ancestor_refused() {
    build_fixture
    local real=$(mktemp -d) link=$(mktemp -u)
    mv "$BUNDLE" "$real/bundle"
    ln -s "$real" "$link"
    BUNDLE=$link/bundle
    register "$BUNDLE" 0 0 755 1
    run_bundle; expect_refused bundle_refused
}
case_corrupt_member_digest_refused() {
    build_fixture
    printf 'tampered\n' > "$BUNDLE/dw-boundary-launcher"
    run_bundle; expect_refused bundle_digest_mismatch
}
case_notice_digest_mismatch_refused() {
    build_fixture
    printf 'fake-license\n' > "$BUNDLE/third_party/genai/LICENSE-MIT"
    run_bundle; expect_refused bundle_digest_mismatch
}
case_empty_member_digest_refused() {
    build_fixture
    : > "$BUNDLE/third_party/genai/source-manifest.json"
    run_bundle; expect_refused bundle_digest_mismatch
}
case_unregistered_member_uid_refused() {
    # One member keeps its REAL uid: non-root CI observes a genuine ownership
    # refusal, never a fake pass.
    build_fixture
    awk -F'|' -v p="$BUNDLE/dw-dns-exporter" '$1 != p' "$STAT_REGISTRY" > "$STAT_REGISTRY.tmp"
    mv "$STAT_REGISTRY.tmp" "$STAT_REGISTRY"
    if [[ $(id -u) == 0 ]]; then register "$BUNDLE/dw-dns-exporter" 1000 0; fi
    run_bundle; expect_refused bundle_member_refused
}

"case_$CASE"
"""


class BundleContractTest(unittest.TestCase):
    """Real install_bundle validator against synthetic bundle fixtures."""

    BASH = shutil.which("bash")

    def setUp(self):
        if self.BASH is None:
            self.fail("Bash is required; these contract tests run on "
                      "Linux/WSL only")
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.driver = Path(self.tmp.name) / "driver.sh"
        self.driver.write_bytes(DRIVER.encode("utf-8"))

    def run_case(self, case):
        env = dict(os.environ)
        env.pop("GOPROXY", None)
        env.update(QUALIFY=QUALIFY.as_posix(), CASE=case)
        result = subprocess.run([self.BASH, self.driver.as_posix()],
                                env=env, capture_output=True, text=True)
        self.assertEqual(result.returncode, 0,
                         f"{case}: rc={result.returncode} "
                         f"stderr={result.stderr!r} stdout={result.stdout!r}")
        return result

    def test_valid_bundle_installs_once(self):
        self.run_case("valid_bundle_installs_once")

    def test_extra_top_level_file_refused(self):
        self.run_case("extra_top_level_file_refused")

    def test_extra_top_level_dir_refused(self):
        self.run_case("extra_top_level_dir_refused")

    def test_nested_extra_refused(self):
        self.run_case("nested_extra_refused")

    def test_hidden_entry_refused(self):
        self.run_case("hidden_entry_refused")

    def test_empty_extra_dir_refused(self):
        self.run_case("empty_extra_dir_refused")

    def test_missing_member_refused(self):
        self.run_case("missing_member_refused")

    def test_symlink_member_refused(self):
        self.run_case("symlink_member_refused")

    def test_symlink_dir_refused(self):
        self.run_case("symlink_dir_refused")

    def test_wrong_member_uid_refused(self):
        self.run_case("wrong_member_uid_refused")

    def test_unsafe_bundle_dir_mode_refused(self):
        self.run_case("unsafe_bundle_dir_mode_refused")

    def test_unsafe_member_mode_refused(self):
        self.run_case("unsafe_member_mode_refused")

    def test_hardlinked_member_refused(self):
        self.run_case("hardlinked_member_refused")

    def test_unsafe_ancestor_refused(self):
        self.run_case("unsafe_ancestor_refused")

    def test_symlinked_ancestor_refused(self):
        self.run_case("symlinked_ancestor_refused")

    def test_corrupt_member_digest_refused(self):
        self.run_case("corrupt_member_digest_refused")

    def test_notice_digest_mismatch_refused(self):
        self.run_case("notice_digest_mismatch_refused")

    def test_empty_member_digest_refused(self):
        self.run_case("empty_member_digest_refused")

    def test_unregistered_member_uid_refused(self):
        self.run_case("unregistered_member_uid_refused")


if __name__ == "__main__":
    unittest.main()
