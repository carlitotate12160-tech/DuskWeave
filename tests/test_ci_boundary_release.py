"""Bash contract tests for boundary/qualify/qualify.sh command dispatch.

The driver below sources the real script definitions (sourcing must not run
the CLI) and invokes the actual ``main`` dispatcher with boundary doubles
for the host/ownership guard, storage check, docker and the build/install/
service/database owners. These tests prove dispatch, argument validation
and refusal ordering only; the real Oracle build-only invocation is the
bundle proof, not these doubles. The tests require Bash on Linux/WSL; an
unavailable Bash on native Windows fails here rather than passing.
"""
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]
QUALIFY = ROOT / "boundary" / "qualify" / "qualify.sh"
LEGACY_IMAGE = "dw-live-dns-20261030:candidate"
LEGACY_SOURCE = "/srv/duskweave/src/dns-exporter-attach-20261103"

DRIVER = r"""set -euo pipefail
# Source the real script; the direct-execution guard must keep this inert.
. "$QUALIFY"

LOG=$(mktemp)
ERR=$(mktemp)
STAT_REGISTRY=$(mktemp)
trap 'rm -f "$LOG" "$ERR" "$STAT_REGISTRY"' EXIT
RC=0
ERRTXT=""

# Test-local metadata doubles scoped to explicitly registered fixture paths:
# non-root CI runners get synthetic root ownership/modes while missing paths,
# symlinks and unregistered paths stay real observations.
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
    # register <path> [uid gid mode nlink]; default fakes ownership 0:0 and
    # keeps the path's real mode/nlink. Later registrations replace earlier.
    local p=$1 u=${2:-0} g=${3:-0} m n
    m=${4:-$(command stat -c %a "$p")}; n=${5:-$(command stat -c %h "$p")}
    printf '%s|%s:%s:%s:%s\n' "$p" "$u" "$g" "$m" "$n" >>"$STAT_REGISTRY"
}
register_ancestors() {
    # Register $1 and its ancestors down to /tmp; / itself stays a real
    # observation. /tmp is a declared unsafe-by-mode ancestor the double pins.
    local p=$1
    while [[ $p == /tmp/?* ]]; do register "$p"; p=${p%/*}; done
    register /tmp 0 0 755 1
}

fail() { printf 'FAIL %s\n' "$*" >&2; exit 1; }
note() { printf '%s\n' "$*" >>"$LOG"; }

invoke_main() {
    : >"$ERR"
    if ( main "$@" ) 2>"$ERR"; then RC=0; else RC=$?; fi
    ERRTXT=$(cat -- "$ERR")
}

double_boundary() {
    require_deployment_host() { return 0; }
    check_build_storage() { note check_build_storage; }
    docker() { note "docker $*"; return 1; }
    build_release() { note "build_release image=${1-} source=$SOURCE goproxy=${GOPROXY-unset}"; }
    install_release() { note "install_release${1:+ $1}"; }
    install_bundle() { note "install_bundle${1:+ $1}"; }
    prepare_service() { note prepare_service; }
    prepare_lab_database() { note "prepare_lab_database${1:+ $*}"; }
    bind_lab_release() { note "bind_lab_release${1:+ $*}"; }
}

expect_refused() {
    [[ $RC != 0 ]] || fail "unexpected_success"
    [[ $ERRTXT == "$1" ]] || fail "stderr=$ERRTXT expected=$1"
}

expect_quiet() { [[ ! -s $LOG ]] || fail "boundary_writes:$(cat "$LOG")"; }

case_sourceable_definitions() {
    declare -F build_only >/dev/null || fail build_only_missing
    declare -F build_release >/dev/null || fail build_release_missing
    declare -F main >/dev/null || fail main_missing
    printf 'sourced_ok\n'
}

case_build_only_dispatch() {
    double_boundary
    invoke_main build-only 'dw-candidate:a1_b2-c3.d4'
    [[ $RC == 0 ]] || fail "rc=$RC err=$ERRTXT"
    expected_source=$(cd -- "$(dirname -- "$QUALIFY")/../.." && pwd -P)
    [[ $(wc -l < "$LOG") == 3 ]] || fail "log:$(cat "$LOG")"
    [[ $(sed -n '1p' "$LOG") == check_build_storage ]] || fail storage_not_first
    [[ $(sed -n '2p' "$LOG") == "docker image inspect dw-candidate:a1_b2-c3.d4" ]] || fail image_unchecked
    [[ $(sed -n '3p' "$LOG") == "build_release image=dw-candidate:a1_b2-c3.d4 source=$expected_source goproxy=off" ]] \
        || fail "build_line:$(sed -n '3p' "$LOG")"
}

case_build_only_never_reaches_install_family() {
    double_boundary
    invoke_main build-only 'dw-isolation:t1'
    [[ $RC == 0 ]] || fail "rc=$RC err=$ERRTXT"
    for forbidden in install_release install_bundle prepare_service prepare_lab_database bind_lab_release; do
        ! grep -q "^$forbidden" "$LOG" || fail "reached_$forbidden"
    done
}

case_bad_command_shape_refused() {
    double_boundary
    invoke_main
    expect_refused qualification_arguments_refused
    invoke_main build-only
    expect_refused qualification_arguments_refused
    invoke_main build-only 'dw-x:t1' extra
    expect_refused qualification_arguments_refused
    invoke_main build-only ''
    expect_refused qualification_arguments_refused
    expect_quiet
}

case_bad_image_ref_refused() {
    double_boundary
    tag129=$(printf 't%.0s' {1..129})
    for ref in 'UPPER:t' 'Name:t' '-name:t' '.name:t' '_name:t' 'name:' ':tag' \
        'name:-tag' 'name:.tag' 'registry/name:t' 'name:tag/path' 'na me:t' \
        'name:ta g' 'name:t@sha256:aa' 'na$(id)me:t' 'name:t$(id)' 'name:t;id' \
        'name:tag,more' "name:$tag129"; do
        invoke_main build-only "$ref"
        expect_refused qualification_arguments_refused
    done
    expect_quiet
}

case_valid_image_refs_accepted() {
    double_boundary
    tag128=$(printf 't%.0s' {1..128})
    sha40=$(printf 'a%.0s' {1..40})
    for ref in 'dw:x' 'a.b-c_d:T.1_2-3' 'n-ame:t' 'na-me.x_y:z' \
        "dw-final-release-prep-20261109:$sha40" "name:$tag128"; do
        invoke_main build-only "$ref"
        [[ $RC == 0 ]] || fail "refused_valid=$ref err=$ERRTXT"
        : >"$LOG"
    done
}

case_existing_image_refused() {
    double_boundary
    docker() { note "docker $*"; return 0; }
    invoke_main build-only 'dw-exists:t1'
    expect_refused build_image_collision
    ! grep -q '^build_release ' "$LOG" || fail build_reached
}

case_storage_refusal_blocks_build() {
    require_deployment_host() { return 0; }
    BUILD=/nonexistent/dw-build-absent-$$
    docker() { note "docker $*"; }
    build_release() { note build_release; }
    install_release() { note install_release; }
    invoke_main build-only 'dw-store:t1'
    expect_refused build_storage_absent
    expect_quiet
}

case_host_guard_blocks_build_only() {
    hostname() { echo "dw-wrong-host-$$"; }
    build_release() { note build_release; }
    install_release() { note install_release; }
    invoke_main build-only 'dw-guard:t1'
    expect_refused host_identity_refused
    expect_quiet
}

case_owner_guard_blocks_build_only() {
    hostname() { echo pica-arm-01; }
    uname() { if [[ $* == -m ]]; then echo aarch64; else command uname "$@"; fi; }
    ROOT=$(mktemp -d)
    build_release() { note build_release; }
    install_release() { note install_release; }
    invoke_main build-only 'dw-guard:t1'
    [[ $RC != 0 ]] || fail unexpected_success
    [[ $ERRTXT == deployment_owner_refused || $ERRTXT == host_identity_refused ]] \
        || fail "stderr=$ERRTXT"
    expect_quiet
}

case_release_builds_then_installs() {
    double_boundary
    invoke_main release
    [[ $RC == 0 ]] || fail "rc=$RC err=$ERRTXT"
    [[ $(wc -l < "$LOG") == 2 ]] || fail "log:$(cat "$LOG")"
    [[ $(sed -n '1p' "$LOG") == "build_release image=LEGACYIMAGE source=LEGACYSOURCE goproxy=unset" ]] || fail "build:$(sed -n '1p' "$LOG")"
    [[ $(sed -n '2p' "$LOG") == install_release ]] || fail "install:$(sed -n '2p' "$LOG")"
}

case_other_commands_dispatch() {
    double_boundary
    invoke_main service
    [[ $RC == 0 ]] || fail "service:$ERRTXT"
    invoke_main lab-database
    [[ $RC == 0 ]] || fail "lab-database:$ERRTXT"
    invoke_main lab-bind
    [[ $RC == 0 ]] || fail "lab-bind:$ERRTXT"
    invoke_main build-storage
    [[ $RC == 0 ]] || fail "build-storage:$ERRTXT"
    for reached in prepare_service prepare_lab_database bind_lab_release check_build_storage; do
        grep -qxF "$reached" "$LOG" || fail "missing_$reached"
    done
    invoke_main definitely-not-a-command
    expect_refused qualification_arguments_refused
}

case_install_bundle_dispatch_and_shape_refusal() {
    double_boundary
    dir=$(mktemp -d)
    invoke_main install-bundle "$dir"
    [[ $RC == 0 ]] || fail "rc=$RC err=$ERRTXT"
    [[ $(cat "$LOG") == "install_bundle $dir" ]] || fail "log:$(cat "$LOG")"
    : >"$LOG"
    invoke_main install-bundle
    expect_refused qualification_arguments_refused
    invoke_main install-bundle "$dir" extra
    expect_refused qualification_arguments_refused
    invoke_main install-bundle relative/dir
    expect_refused qualification_arguments_refused
    invoke_main install-bundle /nonexistent/dw-bundle-$$
    expect_refused qualification_arguments_refused
    invoke_main install-bundle "$dir/file"
    expect_refused qualification_arguments_refused
    expect_quiet
}

lab_source_fixture() {
    # $1 gets a custody-valid explicit SOURCE: bootstrap + one migration SQL,
    # all registered as synthetic root-owned protected members.
    mkdir -p "$1/tests/support" "$1/migrations"
    : > "$1/tests/support/m1_native_bootstrap.py"
    printf 'SELECT 1;\n' > "$1/migrations/0001_fixture.sql"
    register_ancestors "$1/tests/support/m1_native_bootstrap.py"
    register "$1/migrations"
    register "$1/migrations/0001_fixture.sql"
}

case_lab_database_fresh_dispatch() {
    double_boundary
    src=$(mktemp -d)
    lab_source_fixture "$src"
    cfg=/tmp/dw-fresh-config-$$
    invoke_main lab-database "$src" dw_native_final_release_qual_20261110 dw_frq10 "$cfg"
    [[ $RC == 0 ]] || fail "rc=$RC err=$ERRTXT"
    [[ $(cat "$LOG") == "prepare_lab_database $src dw_native_final_release_qual_20261110 dw_frq10 $cfg" ]] \
        || fail "log:$(cat "$LOG")"
}

case_lab_database_fresh_refused() {
    double_boundary
    src=$(mktemp -d)
    lab_source_fixture "$src"
    plain=$(mktemp -d)
    taken=$(mktemp -d)/taken
    mkdir "$taken"
    invoke_main lab-database "$plain" dw_native_db_1 dw_pre /tmp/dw-cfg-$$
    expect_refused qualification_arguments_refused
    invoke_main lab-database relative/src dw_native_db_1 dw_pre /tmp/dw-cfg-$$
    expect_refused qualification_arguments_refused
    invoke_main lab-database "$src" not_native dw_pre /tmp/dw-cfg-$$
    expect_refused qualification_arguments_refused
    invoke_main lab-database "$src" dw_native_db_1 badprefix /tmp/dw-cfg-$$
    expect_refused qualification_arguments_refused
    invoke_main lab-database "$src" dw_native_db_1 dw_pre "$taken"
    expect_refused qualification_arguments_refused
    invoke_main lab-database "$src" dw_native_db_1 dw_pre relative/cfg
    expect_refused qualification_arguments_refused
    invoke_main lab-database "$src" dw_native_db_1 dw_pre /nonexistent-parent-$$/cfg
    expect_refused qualification_arguments_refused
    invoke_main lab-database "$src" dw_native_db_1
    expect_refused qualification_arguments_refused
    invoke_main lab-database "$src" dw_native_db_1 dw_pre /tmp/dw-cfg-$$ extra
    expect_refused qualification_arguments_refused
    expect_quiet
}

case_lab_bind_fresh_dispatch() {
    double_boundary
    register /tmp 0 0 755 1
    rpt=/tmp/dw-fresh-report-$$
    invoke_main lab-bind dw-img:t1 dw_native_db_2 "$rpt"
    [[ $RC == 0 ]] || fail "rc=$RC err=$ERRTXT"
    [[ $(cat "$LOG") == "bind_lab_release dw-img:t1 dw_native_db_2 $rpt" ]] || fail "log:$(cat "$LOG")"
}

case_lab_bind_fresh_refused() {
    double_boundary
    register /tmp 0 0 755 1
    kept=$(mktemp -d)
    chmod 0755 "$kept"
    register "$kept"
    invoke_main lab-bind 'reg/name:t' dw_native_db_2 /tmp/rpt-$$
    expect_refused qualification_arguments_refused
    invoke_main lab-bind 'dw-img:t1' legacy_db /tmp/rpt-$$
    expect_refused qualification_arguments_refused
    invoke_main lab-bind 'dw-img:t1' dw_native_db_2 relative/rpt
    expect_refused qualification_arguments_refused
    invoke_main lab-bind 'dw-img:t1' dw_native_db_2 "$kept"
    expect_refused qualification_arguments_refused
    invoke_main lab-bind 'dw-img:t1' dw_native_db_2 /tmp/rpt-$$ extra
    expect_refused qualification_arguments_refused
    expect_quiet
}

"case_$CASE"
""".replace("LEGACYIMAGE", LEGACY_IMAGE).replace("LEGACYSOURCE", LEGACY_SOURCE)


class BuildOnlyContractTest(unittest.TestCase):
    """Dispatch and refusal contract for the qualify.sh build-only command."""

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
        # Child gets a copy of this environment minus only GOPROXY: the legacy
        # release path then sees a deterministic unset baseline while the
        # build-only case passes GOPROXY=off itself, explicitly, to Go.
        env = dict(os.environ)
        env.pop("GOPROXY", None)
        env.update(QUALIFY=QUALIFY.as_posix(), CASE=case)
        result = subprocess.run([self.BASH, self.driver.as_posix()],
                                env=env, capture_output=True, text=True)
        self.assertEqual(result.returncode, 0,
                         f"{case}: rc={result.returncode} "
                         f"stderr={result.stderr!r} stdout={result.stdout!r}")
        return result

    def test_definitions_sourceable_without_dispatch(self):
        self.assertIn("sourced_ok",
                      self.run_case("sourceable_definitions").stdout)

    def test_build_only_source_and_image_reach_build_release(self):
        self.run_case("build_only_dispatch")

    def test_build_only_never_reaches_install_family(self):
        self.run_case("build_only_never_reaches_install_family")

    def test_bad_command_shape_refused(self):
        self.run_case("bad_command_shape_refused")

    def test_bad_image_ref_refused(self):
        self.run_case("bad_image_ref_refused")

    def test_valid_image_refs_accepted(self):
        self.run_case("valid_image_refs_accepted")

    def test_existing_image_refused(self):
        self.run_case("existing_image_refused")

    def test_storage_refusal_blocks_build(self):
        self.run_case("storage_refusal_blocks_build")

    def test_host_guard_blocks_build_only(self):
        self.run_case("host_guard_blocks_build_only")

    def test_owner_guard_blocks_build_only(self):
        self.run_case("owner_guard_blocks_build_only")

    def test_release_builds_then_installs_with_legacy_tag(self):
        self.run_case("release_builds_then_installs")

    def test_other_commands_dispatch(self):
        self.run_case("other_commands_dispatch")

    def test_install_bundle_dispatch_and_shape_refusal(self):
        self.run_case("install_bundle_dispatch_and_shape_refusal")

    def test_lab_database_fresh_dispatch(self):
        self.run_case("lab_database_fresh_dispatch")

    def test_lab_database_fresh_refused(self):
        self.run_case("lab_database_fresh_refused")

    def test_lab_bind_fresh_dispatch(self):
        self.run_case("lab_bind_fresh_dispatch")

    def test_lab_bind_fresh_refused(self):
        self.run_case("lab_bind_fresh_refused")

    def test_direct_invocation_refuses_off_host_or_unknown(self):
        # A directly executed script must run its real guard + dispatch chain:
        # off pica-arm-01 the host guard refuses; on it an unknown command is
        # refused by dispatch. Either way nothing builds.
        result = subprocess.run(
            [self.BASH, QUALIFY.as_posix(), "definitely-not-a-command"],
            capture_output=True, text=True)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn(result.stderr.strip(),
                      ("host_identity_refused", "qualification_arguments_refused"))


if __name__ == "__main__":
    unittest.main()
