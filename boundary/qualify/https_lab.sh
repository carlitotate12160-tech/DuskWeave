#!/usr/bin/env bash
# HTTPS LAB schema-0038 pinned-release qualification driver; inert when sourced.

HTTPS_LAB_QUALIFY_DIR=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd -P)
HTTPS_LAB_FIXED_SOURCE=/srv/duskweave/src/https-lab-r1
HTTPS_LAB_BUILD_ROOT=/srv/duskweave-build/https-lab-r1
HTTPS_LAB_REPORT_ROOT=/srv/duskweave/reports/https-lab-r1
HTTPS_LAB_BUNDLE_DIR=$HTTPS_LAB_REPORT_ROOT/bundle
HTTPS_LAB_IMAGE_REF=dw-https-lab:r1
HTTPS_LAB_CONTAINER=dw-live-dns-20261030-pg
HTTPS_LAB_DB=dw_native_https_lab_r1
HTTPS_LAB_ROLE_PREFIX=dw_hlr1
HTTPS_LAB_CFG=/etc/duskweave/m1/https-lab-r1
HTTPS_LAB_ROOT_PEM_DST=/etc/duskweave/https-lab-root-v1.pem
HTTPS_LAB_ROOT_PEM_SHA256=af8714b55edd3a0a47cf24a1b24a6e99d9026ed4dd074b5a972a60618103d1dc

https_lab_refuse() {
    printf '%s\n' "$1" >&2
    exit 1
}

https_lab_require_offline_env() {
    local name
    while IFS='=' read -r name _; do
        case $name in
            DW_TEST_FIXTURE_SUPERVISOR) ;;
            DEEPSEEK_API_KEY|DW_*) https_lab_refuse "https_lab_env_refused:$name" ;;
        esac
    done < <(env)
}

https_lab_require_clean_env() {
    local name
    while IFS='=' read -r name _; do
        case $name in
            DEEPSEEK_API_KEY|DW_*|*_FILE|HTTPS_LAB_*|CANDIDATE_*|SOURCE|SOURCE_ROOT|BUILD|PACKAGE_DIR|BUNDLE_DIR|\
            CARGO_*|RUSTC|RUSTC_*|RUSTDOC|RUSTDOC_*|RUSTFLAGS|CARGO_TARGET_DIR|\
            GOFLAGS|GOCACHE|GOPATH|GOMODCACHE|GOTMPDIR|GOPROXY|GOOS|GOARCH|CGO_*|\
            CC|CC_*|CXX|CXX_*|LD|AR|DOCKER_HOST|HTTP_PROXY|http_proxy|HTTPS_PROXY|https_proxy|ALL_PROXY|all_proxy|LD_*)
                https_lab_refuse "https_lab_env_refused:$name"
                ;;
        esac
    done < <(env)
}

https_lab_require_host() {
    [[ $(uname -s) == Linux && $(uname -m) == aarch64 ]] || https_lab_refuse host_architecture_refused
    (( EUID == 0 )) || https_lab_refuse root_privilege_required
    [[ $HTTPS_LAB_QUALIFY_DIR == "$HTTPS_LAB_FIXED_SOURCE/boundary/qualify" ]] || https_lab_refuse privileged_driver_outside_fixed_source
    local owner=/etc/duskweave/m1/.live-dns-owne
    [[ -f $owner && ! -L $owner && $(cat "$owner") == DW-IMPLEMENT-M1-LIVE-DNS-GENAI ]] || https_lab_refuse deployment_owner_refused
}

https_lab_verify_drivers() {
    local ec="" et="" arg prev=""
    for arg in "$@"; do
        [[ $prev == --expected-driver-commit ]] && ec=$arg
        [[ $prev == --expected-driver-tree ]] && et=$arg
        prev=$arg
    done
    [[ $ec =~ ^[0-9a-f]{40}$ && $et =~ ^[0-9a-f]{40}$ ]] || https_lab_refuse driver_pin_malformed
    python3 -B - "$HTTPS_LAB_QUALIFY_DIR" "$ec" "$et" <<'PY'
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
            --expected-driver-commit) ec=${2:-}; shift 2 || https_lab_refuse https_lab_arguments_refused ;;
            --expected-driver-tree) et=${2:-}; shift 2 || https_lab_refuse https_lab_arguments_refused ;;
            *) https_lab_refuse https_lab_arguments_refused ;;
        esac
    done
    [[ $ec =~ ^[0-9a-f]{40}$ && $et =~ ^[0-9a-f]{40}$ ]] || https_lab_refuse driver_pin_malformed
    source "$HTTPS_LAB_QUALIFY_DIR/deployment_custody.sh"
    source "$HTTPS_LAB_QUALIFY_DIR/candidate.sh"
    local src=$HTTPS_LAB_FIXED_SOURCE br=$HTTPS_LAB_BUILD_ROOT bdir=$HTTPS_LAB_BUNDLE_DIR img=$HTTPS_LAB_IMAGE_REF
    check_candidate_build_storage "${BUILD:-/srv/duskweave-build}" "$br"
    [[ ! -e $br && ! -L $br ]] || https_lab_refuse build_root_collision
    [[ ! -e $bdir && ! -L $bdir ]] || https_lab_refuse https_lab_package_collision
    ! docker image inspect "$img" >/dev/null 2>&1 || https_lab_refuse build_image_collision
    python3 -B "$HTTPS_LAB_QUALIFY_DIR/https_lab_release.py" verify-source \
        --source-root "$src" --expected-commit "$ec" --expected-tree "$et" \
        --expected-driver-commit "$ec" --expected-driver-tree "$et"
    install -d -m 0700 "$HTTPS_LAB_REPORT_ROOT"
    export PATH=/srv/duskweave/toolchains/rust-1.94.1/bin:/srv/duskweave/toolchains/go1.27.3/bin:/usr/sbin:/usr/bin:/sbin:/bin
    export CARGO_HOME=/srv/duskweave/build/cargo-home CARGO_TARGET_DIR=$br/normal
    export GOCACHE=$br/go-cache GOTMPDIR=$br/tmp/go TMPDIR=$br/tmp/build GOPROXY=off GOTOOLCHAIN=local
    local dev sub output=$br/go-release context=$br/boundary-release arch digest artifact_dir=$br/tmp/build/pkg-artifacts
    dev=$(stat -c %d "${BUILD:-/srv/duskweave-build}")
    for sub in normal go-cache tmp tmp/go tmp/build go-release boundary-release; do
        install -d -m 0755 "$br/$sub"
        [[ $(stat -c %d "$br/$sub") == "$dev" ]] || https_lab_refuse build_storage_nested_mount
    done
    (cd "$src" && cargo build --locked --offline --release --features fixture-supervisor --bin duskweave --bin dw-boundary-launcher)
    (
        cd "$src/boundary"
        for arch in arm64 amd64; do
            CGO_ENABLED=0 GOOS=linux GOARCH=$arch go build -trimpath -o "$output/dw-dns-capture-$arch" ./cmd/dw-dns-capture
            CGO_ENABLED=0 GOOS=linux GOARCH=$arch go build -trimpath -o "$output/dw-dns-exporter-$arch" ./cmd/dw-dns-exporte
            CGO_ENABLED=0 GOOS=linux GOARCH=$arch go build -trimpath -o "$output/httpslab-$arch" ./qualify/httpslab
        done
    )
    install -m 0555 "$output/dw-dns-capture-arm64" "$context/dw-dns-capture"
    install -m 0555 "$output/dw-dns-exporter-arm64" "$context/dw-dns-exporter"
    install -m 0444 "$src/boundary/Dockerfile.https-lab" "$context/Dockerfile"
    install -m 0444 "$src/boundary/fixtures/https-lab-root-v1.pem" "$context/https-lab-root-v1.pem"
    docker build --network none --tag "$img" "$context"
    digest=$(docker image inspect --format '{{.Id}}' "$img")
    install -d -m 0755 "$artifact_dir"
    cp "$br/normal/release/duskweave" "$artifact_dir/duskweave"
    cp "$br/normal/release/dw-boundary-launcher" "$artifact_dir/dw-boundary-launcher"
    cp "$output/dw-dns-capture-arm64" "$artifact_dir/dw-dns-capture"
    cp "$output/dw-dns-exporter-arm64" "$artifact_dir/dw-dns-exporter"
    cp "$output/httpslab-arm64" "$artifact_dir/httpslab"
    python3 -B "$HTTPS_LAB_QUALIFY_DIR/https_lab_release.py" package \
        --package-dir "$bdir" --source-root "$src" --artifact-dir "$artifact_dir" \
        --image-ref "$img" --image-digest "$digest" \
        --build-source-commit "$ec" --build-source-tree "$et" --driver-commit "$ec" --driver-tree "$et"
}

https_lab_provision_db() {
    python3 -B "$HTTPS_LAB_QUALIFY_DIR/candidate_database.py" \
        --container "$HTTPS_LAB_CONTAINER" --database "$HTTPS_LAB_DB" --role-prefix "$HTTPS_LAB_ROLE_PREFIX" \
        --config-root "$HTTPS_LAB_CFG" --schema-revision 0038 "$@"
}

https_lab_install() {
    install -m 0444 "$HTTPS_LAB_BUNDLE_DIR/https-lab-root-v1.pem" "$HTTPS_LAB_ROOT_PEM_DST"
    [[ $(sha256sum "$HTTPS_LAB_ROOT_PEM_DST" | awk '{print $1}') == "$HTTPS_LAB_ROOT_PEM_SHA256" ]] || https_lab_refuse https_lab_root_pem_mismatch
    python3 -B "$HTTPS_LAB_QUALIFY_DIR/candidate_install.py" install "$@"
}

https_lab_main() {
    [[ $# -ge 1 ]] || https_lab_refuse https_lab_arguments_refused
    local subcmd=$1
    shift
    case "$subcmd" in
        package|verify-descriptor|verify-source)
            https_lab_require_offline_env
            python3 -B "$HTTPS_LAB_QUALIFY_DIR/https_lab_release.py" "$subcmd" "$@"
            ;;
        render-binding)
            https_lab_require_offline_env
            python3 -B "$HTTPS_LAB_QUALIFY_DIR/https_lab_binding.py" render-binding "$@"
            ;;
        build-storage)
            https_lab_require_host
            https_lab_require_clean_env
            https_lab_verify_drivers "$@"
            source "$HTTPS_LAB_QUALIFY_DIR/deployment_custody.sh"
            source "$HTTPS_LAB_QUALIFY_DIR/candidate.sh"
            check_candidate_build_storage "${BUILD:-/srv/duskweave-build}" "$HTTPS_LAB_BUILD_ROOT"
            ;;
        build)
            https_lab_require_host
            https_lab_require_clean_env
            https_lab_verify_drivers "$@"
            https_lab_build "$@"
            ;;
        provision-db)
            https_lab_require_host
            https_lab_require_clean_env
            https_lab_verify_drivers "$@"
            https_lab_provision_db "$@"
            ;;
        install)
            https_lab_require_host
            https_lab_require_clean_env
            https_lab_verify_drivers "$@"
            https_lab_install "$@"
            ;;
        bind|reconcile)
            https_lab_require_host
            https_lab_require_clean_env
            https_lab_verify_drivers "$@"
            python3 -B "$HTTPS_LAB_QUALIFY_DIR/https_lab_binding.py" "$subcmd" "$@"
            ;;
        *)
            https_lab_refuse https_lab_arguments_refused
            ;;
    esac
}

if [[ ${BASH_SOURCE[0]} == "$0" ]]; then
    https_lab_main "$@"
fi
