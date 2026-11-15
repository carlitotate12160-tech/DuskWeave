#!/usr/bin/env bash
# Candidate PR154-baseline qualification tooling; inert when sourced.

CANDIDATE_QUALIFY_DIR=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd -P)
CANDIDATE_BUILD_ROOT=${CANDIDATE_BUILD_ROOT:-/srv/duskweave-build/linux-prov2-qual-r1}

mount_fstype() { findmnt --noheadings --output FSTYPE --target "$1"; }
mount_dev() { stat -c %d "$1"; }
candidate_mountpoint() { mountpoint -q "$1"; }

candidate_require_clean_env() {
    local name
    while IFS='=' read -r name _; do
        case $name in
            DEEPSEEK_API_KEY|DW_*|*_FILE) refuse candidate_env_refused ;;
        esac
    done < <(env)
}

candidate_verify_sources() {
    local repo_root=${1:-$(cd -- "$CANDIDATE_QUALIFY_DIR/../.." && pwd -P)}
    local file
    for file in \
        "$CANDIDATE_QUALIFY_DIR/deployment_custody.sh" \
        "$CANDIDATE_QUALIFY_DIR/candidate.sh" \
        "$CANDIDATE_QUALIFY_DIR/qualify.sh" \
        "$CANDIDATE_QUALIFY_DIR/release_descriptor.py" \
        "$CANDIDATE_QUALIFY_DIR/candidate_database.py" \
        "$CANDIDATE_QUALIFY_DIR/candidate_binding.py" \
        "$repo_root/tests/support/m1_native_bootstrap.py"; do
        protected_ancestors "$file"
        protected_regular_file "$file"
    done
}

candidate_storage_dir() {
    local path=$1 current
    install -d -m 0755 "$path"
    current=$path
    while [[ $current == "$CANDIDATE_BUILD_ROOT"* ]]; do
        [[ -d $current && ! -L $current ]] || refuse build_storage_content_refused
        protected_dir "$current"
        [[ $(mount_dev "$current") == "$2" ]] || refuse build_storage_nested_mount
        [[ $current == "$CANDIDATE_BUILD_ROOT" ]] && break
        current=${current%/*}
    done
}

check_candidate_build_storage() {
    [[ -d $BUILD && ! -L $BUILD ]] || refuse build_storage_absent
    candidate_mountpoint "$BUILD" || refuse build_storage_unmounted
    local device coupled entry sub
    device=$(mount_dev "$BUILD")
    for coupled in / /var/lib/docker /var/lib/containerd; do
        [[ $(mount_dev "$coupled") != "$device" ]] || refuse build_storage_not_isolated
    done
    [[ $(mount_fstype "$BUILD") == ext4 ]] || refuse build_storage_filesystem_refused
    protected_ancestors "$CANDIDATE_BUILD_ROOT"
    protected_dir "$BUILD"
    if [[ -e $CANDIDATE_BUILD_ROOT || -L $CANDIDATE_BUILD_ROOT ]]; then
        [[ -d $CANDIDATE_BUILD_ROOT && ! -L $CANDIDATE_BUILD_ROOT ]] || refuse build_storage_content_refused
        protected_dir "$CANDIDATE_BUILD_ROOT"
        [[ $(mount_dev "$CANDIDATE_BUILD_ROOT") == "$device" ]] || refuse build_storage_nested_mount
        for entry in "$CANDIDATE_BUILD_ROOT"/* "$CANDIDATE_BUILD_ROOT"/.[!.]* "$CANDIDATE_BUILD_ROOT"/..?*; do
            [[ -e $entry || -L $entry ]] || continue
            [[ ! -L $entry && -d $entry ]] || refuse build_storage_foreign_entry
            protected_dir "$entry"
            [[ $(mount_dev "$entry") == "$device" ]] || refuse build_storage_nested_mount
            case ${entry##*/} in
                normal|go-cache|go-release|boundary-release) ;;
                tmp)
                    for sub in "$entry"/* "$entry"/.[!.]* "$entry"/..?*; do
                        [[ -e $sub || -L $sub ]] || continue
                        [[ ! -L $sub && -d $sub ]] || refuse build_storage_foreign_entry
                        protected_dir "$sub"
                        case ${sub##*/} in
                            go|build) ;;
                            *) refuse build_storage_foreign_entry ;;
                        esac
                    done
                    ;;
                *) refuse build_storage_foreign_entry ;;
            esac
        done
    fi
    printf 'candidate_build_storage_ready root=%s device=%s fstype=ext4\n' "$CANDIDATE_BUILD_ROOT" "$device"
}

build_release_into() {
    local image_ref=$1 source_root=$2 build_root=$3
    export PATH=$ROOT/toolchains/rust-1.94.1/bin:$ROOT/toolchains/go1.27.3/bin:$PATH
    export CARGO_HOME=$ROOT/build/cargo-home CARGO_TARGET_DIR=$build_root/normal
    export GOCACHE=$build_root/go-cache GOTMPDIR=$build_root/tmp/go TMPDIR=$build_root/tmp/build
    local component_dir
    for component_dir in "$CARGO_TARGET_DIR" "$GOCACHE" "$GOTMPDIR" "$TMPDIR"; do
        install_storage_dir "$component_dir"
    done
    cd "$source_root"
    cargo build --locked --offline --release --features fixture-supervisor --bin duskweave --bin dw-boundary-launcher
    local output=$build_root/go-release
    install_storage_dir "$output"
    cd boundary
    local arch
    for arch in arm64 amd64; do
        CGO_ENABLED=0 GOOS=linux GOARCH=$arch go build -trimpath -o "$output/dw-dns-capture-$arch" ./cmd/dw-dns-capture
        CGO_ENABLED=0 GOOS=linux GOARCH=$arch go build -trimpath -o "$output/dw-dns-exporter-$arch" ./cmd/dw-dns-exporter
    done
    CGO_ENABLED=0 go build -trimpath -o "$output/dns-lab" ./qualify/dns_lab.go
    local context=$build_root/boundary-release
    install_storage_dir "$context"
    install -m 0555 "$output/dw-dns-capture-arm64" "$context/dw-dns-capture"
    install -m 0555 "$output/dw-dns-exporter-arm64" "$context/dw-dns-exporter"
    install -m 0444 Dockerfile "$context/Dockerfile"
    docker build --network none --tag "$image_ref" "$context"
}

candidate_build() {
    [[ $# == 3 ]] || refuse candidate_arguments_refused
    local source_root=$1 image_ref=$2 package_dir=$3
    [[ $source_root == /* && -d $source_root && ! -L $source_root ]] || refuse candidate_arguments_refused
    [[ $package_dir == /* ]] || refuse candidate_arguments_refused
    [[ ! -e $package_dir && ! -L $package_dir ]] || refuse candidate_package_collision
    valid_image_ref "$image_ref" || refuse candidate_arguments_refused
    candidate_require_clean_env
    candidate_verify_sources "$source_root"
    check_candidate_build_storage
    if docker image inspect "$image_ref" >/dev/null 2>&1; then
        refuse build_image_collision
    fi
    local dev
    dev=$(mount_dev "$BUILD")
    for sub in normal go-cache tmp tmp/go tmp/build go-release boundary-release; do
        candidate_storage_dir "$CANDIDATE_BUILD_ROOT/$sub" "$dev"
    done
    GOPROXY=off build_release_into "$image_ref" "$source_root" "$CANDIDATE_BUILD_ROOT"
    local digest artifact_dir commit tree drv_commit drv_tree
    digest=$(docker image inspect --format '{{.Id}}' "$image_ref")
    commit=$(git -C "$source_root" rev-parse HEAD)
    tree=$(git -C "$source_root" rev-parse 'HEAD^{tree}')
    drv_commit=$(git -C "$CANDIDATE_QUALIFY_DIR/../.." rev-parse HEAD)
    drv_tree=$(git -C "$CANDIDATE_QUALIFY_DIR/../.." rev-parse 'HEAD^{tree}')
    artifact_dir=$CANDIDATE_BUILD_ROOT/tmp/build/pkg-artifacts
    install -d -m 0755 "$artifact_dir"
    cp "$CANDIDATE_BUILD_ROOT/normal/release/duskweave" "$artifact_dir/duskweave"
    cp "$CANDIDATE_BUILD_ROOT/normal/release/dw-boundary-launcher" "$artifact_dir/dw-boundary-launcher"
    cp "$CANDIDATE_BUILD_ROOT/go-release/dw-dns-capture-arm64" "$artifact_dir/dw-dns-capture"
    cp "$CANDIDATE_BUILD_ROOT/go-release/dw-dns-exporter-arm64" "$artifact_dir/dw-dns-exporter"
    python3 -B "$CANDIDATE_QUALIFY_DIR/release_descriptor.py" package \
        --package-dir "$package_dir" --source-root "$source_root" --artifact-dir "$artifact_dir" \
        --image-ref "$image_ref" --image-digest "$digest" \
        --build-source-commit "$commit" --build-source-tree "$tree" \
        --driver-commit "$drv_commit" --driver-tree "$drv_tree"
}

candidate_privileged_dispatch() {
    local subcmd=$1
    shift
    require_deployment_host
    candidate_require_clean_env
    candidate_verify_sources
    case "$subcmd" in
        build-storage) [[ $# == 0 ]] || refuse candidate_arguments_refused; check_candidate_build_storage ;;
        build) candidate_build "$@" ;;
        provision-db) python3 -B "$CANDIDATE_QUALIFY_DIR/candidate_database.py" "$@" ;;
        install) python3 -B "$CANDIDATE_QUALIFY_DIR/release_descriptor.py" "$subcmd" "$@" ;;
        bind|reconcile) python3 -B "$CANDIDATE_QUALIFY_DIR/candidate_binding.py" "$subcmd" "$@" ;;
        *) refuse candidate_arguments_refused ;;
    esac
}

candidate_main() {
    [[ $# -ge 1 ]] || refuse candidate_arguments_refused
    local subcmd=$1
    shift
    case "$subcmd" in
        package|verify-descriptor)
            python3 -B "$CANDIDATE_QUALIFY_DIR/release_descriptor.py" "$subcmd" "$@"
            ;;
        render-binding)
            python3 -B "$CANDIDATE_QUALIFY_DIR/candidate_binding.py" render-binding "$@"
            ;;
        build-storage|build|provision-db|install|bind|reconcile)
            candidate_privileged_dispatch "$subcmd" "$@"
            ;;
        *)
            refuse candidate_arguments_refused
            ;;
    esac
}
