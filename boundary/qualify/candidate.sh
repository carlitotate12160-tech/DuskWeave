#!/usr/bin/env bash
# Candidate pinned-release qualification tooling; inert when sourced.

CANDIDATE_QUALIFY_DIR=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd -P)
CANDIDATE_FIXED_SOURCE=/srv/duskweave/src/linux-prov2-qual-r1
CANDIDATE_BUILD_ROOT=/srv/duskweave-build/linux-prov2-qual-r1
CANDIDATE_REPORT_ROOT=/srv/duskweave/reports/live-context-purpose-r1
CANDIDATE_BUNDLE_DIR=$CANDIDATE_REPORT_ROOT/bundle
CANDIDATE_IMAGE_REF=dw-live-dns-20261030:candidate-r1
CANDIDATE_STORAGE_SUBDIRS="normal normal/release normal/release/build normal/release/deps normal/release/examples normal/release/incremental normal/release/.fingerprint go-cache tmp tmp/go tmp/build go-release boundary-release"

mount_fstype() { findmnt --noheadings --output FSTYPE --target "$1"; }
mount_dev() { stat -c %d "$1"; }
candidate_mountpoint() { mountpoint -q "$1"; }
candidate_refuse() { if declare -F refuse >/dev/null 2>&1; then refuse "$1"; else printf '%s\n' "$1" >&2; exit 1; fi; }

candidate_require_clean_env() {
    local name
    while IFS='=' read -r name _; do
        case $name in
            DEEPSEEK_API_KEY|DW_*|*_FILE|CANDIDATE_*|SOURCE|SOURCE_ROOT|BUILD|PACKAGE_DIR|BUNDLE_DIR|\
            CARGO_*|RUSTC|RUSTC_*|RUSTDOC|RUSTDOC_*|RUSTFLAGS|CARGO_TARGET_DIR|\
            GOFLAGS|GOCACHE|GOPATH|GOMODCACHE|GOTMPDIR|GOPROXY|GOOS|GOARCH|CGO_*|\
            CC|CC_*|CXX|CXX_*|LD|AR|DOCKER_HOST|HTTP_PROXY|http_proxy|HTTPS_PROXY|https_proxy|ALL_PROXY|all_proxy|LD_*)
                candidate_refuse "candidate_env_refused:$name"
                ;;
        esac
    done < <(env)
}

candidate_verify_sources() {
    local qdir=${1:-$CANDIDATE_QUALIFY_DIR} commit=${2:-} tree=${3:-} repo_root file
    repo_root=$(cd -- "$qdir/../.." && pwd -P)
    for file in \
        "$qdir/deployment_custody.sh" "$qdir/qualify.sh" "$qdir/candidate.sh" \
        "$qdir/release_descriptor.py" "$qdir/candidate_files.py" "$qdir/candidate_postgres.py" \
        "$qdir/candidate_host.py" "$qdir/candidate_install.py" \
        "$qdir/candidate_database.py" "$qdir/candidate_binding.py" \
        "$repo_root/tests/support/m1_native_bootstrap.py"; do
        protected_ancestors "$file" || candidate_refuse driver_custody_refused
        protected_regular_file "$file" || candidate_refuse driver_custody_refused
    done
    if [[ -n $commit || -n $tree ]]; then
        qualify_verify_driver_before_load "$qdir" "$commit" "$tree" 1
    fi
}

candidate_check_mountinfo() {
    local root_dir=$1 line mnt
    [[ -r /proc/self/mountinfo ]] || candidate_refuse build_storage_mountinfo_unreadable
    while IFS= read -r line; do
        mnt=$(awk '{print $5}' <<<"$line")
        case $mnt in
            "$root_dir"|"$root_dir"/*) candidate_refuse build_storage_nested_mount ;;
        esac
    done < /proc/self/mountinfo
}

candidate_storage_dir() {
    local path=$1 dev=$2 current
    install -d -m 0755 "$path"
    current=$path
    while [[ $current == "$CANDIDATE_BUILD_ROOT"* ]]; do
        [[ -d $current && ! -L $current ]] || candidate_refuse build_storage_content_refused
        protected_dir "$current" || candidate_refuse build_storage_content_refused
        [[ $(mount_dev "$current") == "$dev" ]] || candidate_refuse build_storage_nested_mount
        ! candidate_mountpoint "$current" || candidate_refuse build_storage_nested_mount
        [[ $current == "$CANDIDATE_BUILD_ROOT" ]] && break
        current=${current%/*}
    done
}

check_candidate_build_storage() {
    local build_parent=${1:-${BUILD:-/srv/duskweave-build}} build_root=${2:-$CANDIDATE_BUILD_ROOT}
    local mountinfo_fn=${3:-candidate_check_mountinfo}
    [[ -d $build_parent && ! -L $build_parent ]] || candidate_refuse build_storage_absent
    candidate_mountpoint "$build_parent" || candidate_refuse build_storage_unmounted
    local device coupled sub comp
    device=$(mount_dev "$build_parent")
    for coupled in / /var/lib/docker /var/lib/containerd; do
        [[ $(mount_dev "$coupled") != "$device" ]] || candidate_refuse build_storage_not_isolated
    done
    [[ $(mount_fstype "$build_parent") == ext4 ]] || candidate_refuse build_storage_filesystem_refused
    protected_ancestors "$build_root" || candidate_refuse build_storage_identity_refused
    protected_dir "$build_parent" || candidate_refuse build_storage_identity_refused
    "$mountinfo_fn" "$build_root"
    for comp in "" $CANDIDATE_STORAGE_SUBDIRS; do
        sub=${comp:+$build_root/$comp}
        sub=${sub:-$build_root}
        [[ -e $sub || -L $sub ]] || continue
        [[ -d $sub && ! -L $sub ]] || candidate_refuse build_storage_content_refused
        protected_dir "$sub" || candidate_refuse build_storage_content_refused
        [[ $(mount_dev "$sub") == "$device" ]] || candidate_refuse build_storage_nested_mount
        ! candidate_mountpoint "$sub" || candidate_refuse build_storage_nested_mount
    done
    if [[ -d $build_root && ! -L $build_root ]]; then
        local entry child
        for entry in "$build_root"/* "$build_root"/.[!.]* "$build_root"/..?*; do
            [[ -e $entry || -L $entry ]] || continue
            [[ -d $entry && ! -L $entry ]] || candidate_refuse build_storage_foreign_entry
            case ${entry##*/} in
                normal|go-cache|go-release|boundary-release) ;;
                tmp)
                    for child in "$entry"/* "$entry"/.[!.]* "$entry"/..?*; do
                        [[ -e $child || -L $child ]] || continue
                        [[ -d $child && ! -L $child ]] || candidate_refuse build_storage_foreign_entry
                        protected_dir "$child" || candidate_refuse build_storage_foreign_entry
                        [[ ${child##*/} == go || ${child##*/} == build ]] || candidate_refuse build_storage_foreign_entry
                    done
                    ;;
                *) candidate_refuse build_storage_foreign_entry ;;
            esac
        done
    fi
    printf 'candidate_build_storage_ready root=%s device=%s fstype=ext4\n' "$build_root" "$device"
}

build_release_into() {
    local image_ref=$1 source_root=$2 build_root=$3
    export PATH=/srv/duskweave/toolchains/rust-1.94.1/bin:/srv/duskweave/toolchains/go1.27.3/bin:/usr/sbin:/usr/bin:/sbin:/bin
    export CARGO_HOME=/srv/duskweave/build/cargo-home CARGO_TARGET_DIR=$build_root/normal
    export GOCACHE=$build_root/go-cache GOTMPDIR=$build_root/tmp/go TMPDIR=$build_root/tmp/build
    export GOPROXY=off
    local component_dir
    for component_dir in $CANDIDATE_STORAGE_SUBDIRS; do
        candidate_storage_dir "$build_root/$component_dir" "$(mount_dev "$BUILD")"
    done
    cd "$source_root"
    cargo build --locked --offline --release --features fixture-supervisor --bin duskweave --bin dw-boundary-launcher
    local output=$build_root/go-release context=$build_root/boundary-release arch
    cd boundary
    for arch in arm64 amd64; do
        CGO_ENABLED=0 GOOS=linux GOARCH=$arch go build -trimpath -o "$output/dw-dns-capture-$arch" ./cmd/dw-dns-capture
        CGO_ENABLED=0 GOOS=linux GOARCH=$arch go build -trimpath -o "$output/dw-dns-exporter-$arch" ./cmd/dw-dns-exporter
    done
    CGO_ENABLED=0 go build -trimpath -o "$output/dns-lab" ./qualify/dns_lab.go
    install -m 0555 "$output/dw-dns-capture-arm64" "$context/dw-dns-capture"
    install -m 0555 "$output/dw-dns-exporter-arm64" "$context/dw-dns-exporter"
    install -m 0444 Dockerfile "$context/Dockerfile"
    docker build --network none --tag "$image_ref" "$context"
}

candidate_parse_driver_pins() {
    local c="" t=""
    while [[ $# -gt 0 ]]; do
        case $1 in
            --expected-driver-commit) c=${2:-}; shift 2 || candidate_refuse candidate_arguments_refused ;;
            --expected-driver-tree) t=${2:-}; shift 2 || candidate_refuse candidate_arguments_refused ;;
            *) candidate_refuse candidate_arguments_refused ;;
        esac
    done
    [[ $c =~ ^[0-9a-f]{40}$ && $t =~ ^[0-9a-f]{40}$ ]] || candidate_refuse driver_pin_malformed
    CANDIDATE_EXP_COMMIT=$c CANDIDATE_EXP_TREE=$t
}

candidate_build() {
    candidate_parse_driver_pins "$@"
    local src=$CANDIDATE_FIXED_SOURCE br=$CANDIDATE_BUILD_ROOT bdir=$CANDIDATE_BUNDLE_DIR img=$CANDIDATE_IMAGE_REF
    check_candidate_build_storage "$BUILD" "$br"
    [[ ! -e $br && ! -L $br ]] || candidate_refuse build_root_collision
    [[ ! -e $bdir && ! -L $bdir ]] || candidate_refuse candidate_package_collision
    ! docker image inspect "$img" >/dev/null 2>&1 || candidate_refuse build_image_collision
    python3 -B "$CANDIDATE_QUALIFY_DIR/release_descriptor.py" verify-source \
        --source-root "$src" --expected-commit "$CANDIDATE_EXP_COMMIT" --expected-tree "$CANDIDATE_EXP_TREE" \
        --expected-driver-commit "$CANDIDATE_EXP_COMMIT" --expected-driver-tree "$CANDIDATE_EXP_TREE"
    [[ -d $CANDIDATE_REPORT_ROOT || ! -e $CANDIDATE_REPORT_ROOT ]] || candidate_refuse report_root_collision
    if [[ ! -e $CANDIDATE_REPORT_ROOT ]]; then
        protected_ancestors "$CANDIDATE_REPORT_ROOT" || candidate_refuse report_root_collision
        install -d -m 0700 "$CANDIDATE_REPORT_ROOT"
    fi
    protected_dir "$CANDIDATE_REPORT_ROOT" || candidate_refuse report_root_collision
    local dev sub digest artifact_dir
    dev=$(mount_dev "$BUILD")
    for sub in $CANDIDATE_STORAGE_SUBDIRS; do
        candidate_storage_dir "$br/$sub" "$dev"
    done
    build_release_into "$img" "$src" "$br"
    digest=$(docker image inspect --format '{{.Id}}' "$img")
    artifact_dir=$br/tmp/build/pkg-artifacts
    install -d -m 0755 "$artifact_dir"
    cp "$br/normal/release/duskweave" "$artifact_dir/duskweave"
    cp "$br/normal/release/dw-boundary-launcher" "$artifact_dir/dw-boundary-launcher"
    cp "$br/go-release/dw-dns-capture-arm64" "$artifact_dir/dw-dns-capture"
    cp "$br/go-release/dw-dns-exporter-arm64" "$artifact_dir/dw-dns-exporter"
    python3 -B "$CANDIDATE_QUALIFY_DIR/release_descriptor.py" package \
        --package-dir "$bdir" --source-root "$src" --artifact-dir "$artifact_dir" \
        --image-ref "$img" --image-digest "$digest" \
        --build-source-commit "$CANDIDATE_EXP_COMMIT" --build-source-tree "$CANDIDATE_EXP_TREE" \
        --driver-commit "$CANDIDATE_EXP_COMMIT" --driver-tree "$CANDIDATE_EXP_TREE"
}

candidate_privileged_dispatch() {
    local subcmd=$1
    shift
    declare -F protected_regular_file >/dev/null && declare -F require_candidate_host >/dev/null || candidate_refuse driver_custody_refused
    require_candidate_host
    [[ $CANDIDATE_QUALIFY_DIR == "$CANDIDATE_FIXED_SOURCE/boundary/qualify" ]] || candidate_refuse privileged_driver_outside_fixed_source
    candidate_require_clean_env
    local ec="" et="" arg prev=""
    for arg in "$@"; do
        [[ $prev == --expected-driver-commit ]] && ec=$arg
        [[ $prev == --expected-driver-tree ]] && et=$arg
        prev=$arg
    done
    [[ $ec =~ ^[0-9a-f]{40}$ && $et =~ ^[0-9a-f]{40}$ ]] || candidate_refuse driver_pin_malformed
    candidate_verify_sources "$CANDIDATE_QUALIFY_DIR" "$ec" "$et"
    case "$subcmd" in
        build-storage) candidate_parse_driver_pins "$@"; check_candidate_build_storage "$BUILD" "$CANDIDATE_BUILD_ROOT" ;;
        build) candidate_build "$@" ;;
        provision-db) python3 -B "$CANDIDATE_QUALIFY_DIR/candidate_database.py" "$@" ;;
        install) python3 -B "$CANDIDATE_QUALIFY_DIR/candidate_install.py" "$@" ;;
        bind|reconcile) python3 -B "$CANDIDATE_QUALIFY_DIR/candidate_binding.py" "$subcmd" "$@" ;;
        *) candidate_refuse candidate_arguments_refused ;;
    esac
}

candidate_require_offline_env() {
    local name
    while IFS='=' read -r name _; do
        case $name in
            DW_TEST_FIXTURE_SUPERVISOR) ;;
            DEEPSEEK_API_KEY|DW_*) candidate_refuse "candidate_env_refused:$name" ;;
        esac
    done < <(env)
}

candidate_main() {
    [[ $# -ge 1 ]] || candidate_refuse candidate_arguments_refused
    local subcmd=$1
    shift
    case "$subcmd" in
        package|verify-descriptor)
            candidate_require_offline_env
            python3 -B "$CANDIDATE_QUALIFY_DIR/release_descriptor.py" "$subcmd" "$@"
            ;;
        render-binding)
            candidate_require_offline_env
            python3 -B "$CANDIDATE_QUALIFY_DIR/candidate_binding.py" render-binding "$@"
            ;;
        build-storage|build|provision-db|install|bind|reconcile)
            candidate_privileged_dispatch "$subcmd" "$@"
            ;;
        *) candidate_refuse candidate_arguments_refused ;;
    esac
}
