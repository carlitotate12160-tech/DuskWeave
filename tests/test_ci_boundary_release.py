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
trap 'rm -f "$LOG" "$ERR"' EXIT
RC=0
ERRTXT=""

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
    install_release() { note install_release; }
    prepare_service() { note prepare_service; }
    prepare_lab_database() { note prepare_lab_database; }
    bind_lab_release() { note bind_lab_release; }
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
    for forbidden in install_release prepare_service prepare_lab_database bind_lab_release; do
        ! grep -qxF "$forbidden" "$LOG" || fail "reached_$forbidden"
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
        env = dict(os.environ, QUALIFY=QUALIFY.as_posix(), CASE=case)
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
