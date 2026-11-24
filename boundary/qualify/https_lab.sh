#!/usr/bin/env bash
# HTTPS LAB schema-0038 pinned-release qualification driver; inert when sourced.

HTTPS_LAB_QUALIFY_DIR=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd -P)
HTTPS_LAB_FIXED_SOURCE=/srv/duskweave/src/https-lab-r1
HTTPS_LAB_BUILD_ROOT=/srv/duskweave-build/https-lab-r1
HTTPS_LAB_REPORT_ROOT=/srv/duskweave/reports/https-lab-r1
HTTPS_LAB_BUNDLE_DIR=$HTTPS_LAB_REPORT_ROOT/bundle
HTTPS_LAB_IMAGE_REF=dw-https-lab:r1
HTTPS_LAB_CONTAINER=dw-live-dns-20261030-pg
HTTPS_LAB_DB=dw_native_https_lab_r1_20261119
HTTPS_LAB_ROLE_PREFIX=dw_hlab_r1
HTTPS_LAB_CFG=/etc/duskweave/m1/https-lab-r1
HTTPS_LAB_ROOT_PEM_DST=/etc/duskweave/https-lab-root-v1.pem
HTTPS_LAB_ROOT_PEM_SHA256=af8714b55edd3a0a47cf24a1b24a6e99d9026ed4dd074b5a972a60618103d1dc

https_lab_refuse() {
    printf '%s\n' "$1" >&2
    exit 1
}

https_lab_require_offline_env() {
    local name env_out
    env_out=$(env) || return $?
    while IFS='=' read -r name _; do
        case $name in
            DW_TEST_FIXTURE_SUPERVISOR) ;;
            DEEPSEEK_API_KEY|DW_*) https_lab_refuse "https_lab_env_refused:$name"; return 1 ;;
        esac
    done <<<"$env_out"
}

https_lab_require_clean_env() {
    local name env_out
    env_out=$(env) || return $?
    while IFS='=' read -r name _; do
        case $name in
            DEEPSEEK_API_KEY|DW_*|*_FILE|HTTPS_LAB_*|CANDIDATE_*|SOURCE|SOURCE_ROOT|BUILD|PACKAGE_DIR|BUNDLE_DIR|\
            CARGO_*|RUSTC|RUSTC_*|RUSTDOC|RUSTDOC_*|RUSTFLAGS|CARGO_TARGET_DIR|\
            GOFLAGS|GOCACHE|GOPATH|GOMODCACHE|GOTMPDIR|GOPROXY|GOOS|GOARCH|CGO_*|\
            CC|CC_*|CXX|CXX_*|LD|AR|DOCKER_HOST|HTTP_PROXY|http_proxy|HTTPS_PROXY|https_proxy|ALL_PROXY|all_proxy|LD_*)
                https_lab_refuse "https_lab_env_refused:$name"
                return 1
                ;;
        esac
    done <<<"$env_out"
}

https_lab_require_host() {
    local sys_name mach_name owner_val
    sys_name=$(uname -s) || { https_lab_refuse host_architecture_refused; return 1; }
    mach_name=$(uname -m) || { https_lab_refuse host_architecture_refused; return 1; }
    [[ $sys_name == Linux && $mach_name == aarch64 ]] || { https_lab_refuse host_architecture_refused; return 1; }
    (( EUID == 0 )) || { https_lab_refuse root_privilege_required; return 1; }
    [[ $HTTPS_LAB_QUALIFY_DIR == "$HTTPS_LAB_FIXED_SOURCE/boundary/qualify" ]] || { https_lab_refuse privileged_driver_outside_fixed_source; return 1; }
    local owner=/etc/duskweave/m1/.live-dns-owner
    [[ -f $owner && ! -L $owner ]] || { https_lab_refuse deployment_owner_refused; return 1; }
    owner_val=$(cat "$owner") || { https_lab_refuse deployment_owner_refused; return 1; }
    [[ $owner_val == DW-IMPLEMENT-M1-LIVE-DNS-GENAI ]] || { https_lab_refuse deployment_owner_refused; return 1; }
}

https_lab_verify_drivers() {
    local ec="" et="" arg prev=""
    for arg in "$@"; do
        [[ $prev == --expected-driver-commit ]] && ec=$arg
        [[ $prev == --expected-driver-tree ]] && et=$arg
        prev=$arg
    done
    [[ $ec =~ ^[0-9a-f]{40}$ && $et =~ ^[0-9a-f]{40}$ ]] || { https_lab_refuse driver_pin_malformed; return 1; }
    python3 -B - "$HTTPS_LAB_QUALIFY_DIR" "$ec" "$et" <<'PY' || return $?
import pathlib, sys
sys.path.insert(0, sys.argv[1])
from https_lab_host import verify_https_lab_driver_custody
verify_https_lab_driver_custody(pathlib.Path(sys.argv[1]), sys.argv[2], sys.argv[3])
PY
}

https_lab_build() {
    local ec="" et=""
    while [[ $# -gt 0 ]]; do
        case $1 in
            --expected-driver-commit) ec=${2:-}; shift 2 || { https_lab_refuse https_lab_arguments_refused; return 1; } ;;
            --expected-driver-tree) et=${2:-}; shift 2 || { https_lab_refuse https_lab_arguments_refused; return 1; } ;;
            *) https_lab_refuse https_lab_arguments_refused; return 1 ;;
        esac
    done
    [[ $ec =~ ^[0-9a-f]{40}$ && $et =~ ^[0-9a-f]{40}$ ]] || { https_lab_refuse driver_pin_malformed; return 1; }
    source "$HTTPS_LAB_QUALIFY_DIR/deployment_custody.sh" || return $?
    source "$HTTPS_LAB_QUALIFY_DIR/candidate.sh" || return $?
    local src=$HTTPS_LAB_FIXED_SOURCE br=$HTTPS_LAB_BUILD_ROOT bdir=$HTTPS_LAB_BUNDLE_DIR img=$HTTPS_LAB_IMAGE_REF
    check_candidate_build_storage "${BUILD:-/srv/duskweave-build}" "$br" || return $?
    [[ ! -e $br && ! -L $br ]] || { https_lab_refuse build_root_collision; return 1; }
    [[ ! -e $bdir && ! -L $bdir ]] || { https_lab_refuse https_lab_package_collision; return 1; }
    ! docker image inspect "$img" >/dev/null 2>&1 || { https_lab_refuse build_image_collision; return 1; }
    python3 -B "$HTTPS_LAB_QUALIFY_DIR/https_lab_release.py" verify-source \
        --source-root "$src" --expected-commit "$ec" --expected-tree "$et" \
        --expected-driver-commit "$ec" --expected-driver-tree "$et" || return $?
    install -d -m 0700 "$HTTPS_LAB_REPORT_ROOT" || return $?
    export PATH=/srv/duskweave/toolchains/rust-1.94.1/bin:/srv/duskweave/toolchains/go1.27.3/bin:/usr/sbin:/usr/bin:/sbin:/bin
    export CARGO_HOME=/srv/duskweave/build/cargo-home CARGO_TARGET_DIR=$br/normal
    export GOCACHE=$br/go-cache GOTMPDIR=$br/tmp/go TMPDIR=$br/tmp/build GOPROXY=off GOTOOLCHAIN=local
    local dev sub sub_dev output=$br/go-release context=$br/boundary-release arch digest artifact_dir=$br/tmp/build/pkg-artifacts
    dev=$(stat -c %d "${BUILD:-/srv/duskweave-build}") || return $?
    for sub in normal go-cache tmp tmp/go tmp/build go-release boundary-release; do
        install -d -m 0755 "$br/$sub" || return $?
        sub_dev=$(stat -c %d "$br/$sub") || return $?
        [[ $sub_dev == "$dev" ]] || { https_lab_refuse build_storage_nested_mount; return 1; }
    done
    (cd "$src" && cargo build --locked --offline --release --features fixture-supervisor --bin duskweave --bin dw-boundary-launcher) || return $?
    (
        cd "$src/boundary" || exit $?
        for arch in arm64 amd64; do
            CGO_ENABLED=0 GOOS=linux GOARCH=$arch go build -trimpath -o "$output/dw-dns-capture-$arch" ./cmd/dw-dns-capture || exit $?
            CGO_ENABLED=0 GOOS=linux GOARCH=$arch go build -trimpath -o "$output/dw-dns-exporter-$arch" ./cmd/dw-dns-exporter || exit $?
            CGO_ENABLED=0 GOOS=linux GOARCH=$arch go build -trimpath -o "$output/httpslab-$arch" ./qualify/httpslab || exit $?
        done
    ) || return $?
    install -m 0555 "$output/dw-dns-capture-arm64" "$context/dw-dns-capture" || return $?
    install -m 0555 "$output/dw-dns-exporter-arm64" "$context/dw-dns-exporter" || return $?
    install -m 0444 "$src/boundary/Dockerfile.https-lab" "$context/Dockerfile" || return $?
    install -m 0444 "$src/boundary/fixtures/https-lab-root-v1.pem" "$context/https-lab-root-v1.pem" || return $?
    docker build --network none --tag "$img" "$context" || return $?
    digest=$(docker image inspect --format '{{.Id}}' "$img") || return $?
    [[ -n $digest ]] || { https_lab_refuse build_image_inspect_failed; return 1; }
    install -d -m 0755 "$artifact_dir" || return $?
    cp "$br/normal/release/duskweave" "$artifact_dir/duskweave" || return $?
    cp "$br/normal/release/dw-boundary-launcher" "$artifact_dir/dw-boundary-launcher" || return $?
    cp "$output/dw-dns-capture-arm64" "$artifact_dir/dw-dns-capture" || return $?
    cp "$output/dw-dns-exporter-arm64" "$artifact_dir/dw-dns-exporter" || return $?
    chmod 0555 "$output/httpslab-arm64" || return $?
    python3 -B "$HTTPS_LAB_QUALIFY_DIR/https_lab_release.py" record-responder \
        --binary-path "$output/httpslab-arm64" --provenance-path "$output/httpslab-arm64.provenance.json" \
        --build-source-commit "$ec" --build-source-tree "$et" --driver-commit "$ec" --driver-tree "$et" || return $?
    python3 -B "$HTTPS_LAB_QUALIFY_DIR/https_lab_release.py" package \
        --package-dir "$bdir" --source-root "$src" --artifact-dir "$artifact_dir" \
        --image-ref "$img" --image-digest "$digest" \
        --build-source-commit "$ec" --build-source-tree "$et" --driver-commit "$ec" --driver-tree "$et" || return $?
}

https_lab_provision_db() {
    python3 -B "$HTTPS_LAB_QUALIFY_DIR/https_lab_host.py" provision-db "$@" || return $?
}

https_lab_install() {
    python3 -B "$HTTPS_LAB_QUALIFY_DIR/https_lab_host.py" install "$@" || return $?
}

https_lab_main() {
    [[ $# -ge 1 ]] || { https_lab_refuse https_lab_arguments_refused; return 1; }
    local subcmd=$1
    shift
    case "$subcmd" in
        package|verify-descriptor|verify-source|record-responder|verify-responder)
            https_lab_require_offline_env || return $?
            python3 -B "$HTTPS_LAB_QUALIFY_DIR/https_lab_release.py" "$subcmd" "$@" || return $?
            ;;
        render-binding)
            https_lab_require_offline_env || return $?
            python3 -B "$HTTPS_LAB_QUALIFY_DIR/https_lab_binding.py" render-binding "$@" || return $?
            ;;
        build-storage)
            https_lab_require_host || return $?
            https_lab_require_clean_env || return $?
            https_lab_verify_drivers "$@" || return $?
            source "$HTTPS_LAB_QUALIFY_DIR/deployment_custody.sh" || return $?
            source "$HTTPS_LAB_QUALIFY_DIR/candidate.sh" || return $?
            check_candidate_build_storage "${BUILD:-/srv/duskweave-build}" "$HTTPS_LAB_BUILD_ROOT" || return $?
            ;;
        build)
            https_lab_require_host || return $?
            https_lab_require_clean_env || return $?
            https_lab_verify_drivers "$@" || return $?
            https_lab_build "$@" || return $?
            ;;
        provision-db)
            https_lab_require_host || return $?
            https_lab_require_clean_env || return $?
            https_lab_verify_drivers "$@" || return $?
            https_lab_provision_db "$@" || return $?
            ;;
        install)
            https_lab_require_host || return $?
            https_lab_require_clean_env || return $?
            https_lab_verify_drivers "$@" || return $?
            https_lab_install "$@" || return $?
            ;;
        bind|reconcile)
            https_lab_require_host || return $?
            https_lab_require_clean_env || return $?
            https_lab_verify_drivers "$@" || return $?
            python3 -B "$HTTPS_LAB_QUALIFY_DIR/https_lab_binding.py" "$subcmd" "$@" || return $?
            ;;
        *)
            https_lab_refuse https_lab_arguments_refused
            return 1
            ;;
    esac
}

if [[ ${BASH_SOURCE[0]} == "$0" ]]; then
    set -euo pipefail
    https_lab_main "$@"
fi
