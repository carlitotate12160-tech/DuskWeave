#!/usr/bin/env bash
# Dedicated packet-owned host preparation. This never authorizes campaign egress.
set -euo pipefail
umask 077

ROOT=/srv/duskweave
REPORT=$ROOT/reports/live-dns-20261030
TOOLS=$ROOT/toolchains
OWNER=$ROOT/.live-dns-20261030-owner
CORE=duskweave-core

refuse() { printf '%s\n' "$1" >&2; exit 1; }
[[ $EUID == 0 ]] || refuse root_required
[[ $(hostname) == pica-arm-01 && $(uname -m) == aarch64 ]] || refuse host_identity_refused
[[ -f /etc/os-release ]] || refuse host_os_unavailable
grep -qx 'VERSION_ID="22.04"' /etc/os-release || refuse host_os_refused
[[ $(df --output=avail -B1 /srv | tail -n1) -ge 12884901888 ]] || refuse host_disk_insufficient
if [[ -L $ROOT || ( -e $ROOT && ! -f $OWNER ) ]]; then refuse deployment_root_collision; fi
if [[ ! -e $ROOT ]]; then
    install -d -m 0755 "$ROOT"
    printf '%s\n' DW-IMPLEMENT-M1-LIVE-DNS-GENAI > "$OWNER"
    chmod 0400 "$OWNER"
fi
[[ $(cat "$OWNER") == DW-IMPLEMENT-M1-LIVE-DNS-GENAI ]] || refuse deployment_owner_refused
[[ ! -L $OWNER && $(stat -c %u "$ROOT") == 0 && $(stat -c %a "$ROOT") == 755 ]] || refuse deployment_owner_refused
install -d -m 0755 "$TOOLS" "$ROOT/src" "$ROOT/build" "$ROOT/build/normal"
install -d -m 0700 "$REPORT" "$TOOLS/downloads"

download() {
    local url=$1 output=$2 expected=$3 staged=$2.next
    if [[ -e $staged ]]; then rm -f -- "$staged"; fi
    if [[ ! -e $output ]]; then
        curl --fail --silent --show-error --location --max-time 300 "$url" -o "$staged"
        printf '%s  %s\n' "$expected" "$staged" | sha256sum --check --status || refuse toolchain_hash_refused
        mv -- "$staged" "$output"
    fi
    printf '%s  %s\n' "$expected" "$output" | sha256sum --check --status || refuse toolchain_hash_refused
}

prepare_go() {
    local archive=$TOOLS/downloads/go1.27.3.linux-arm64.tar.gz
    download https://go.dev/dl/go1.27.3.linux-arm64.tar.gz "$archive" a06e99309dbcbdca1a8ccd2282c3ee146e55930886e26b4f1193269587165f49
    if [[ ! -e $TOOLS/go1.27.3 ]]; then
        install -d -m 0755 "$TOOLS/go1.27.3"
        tar -xzf "$archive" -C "$TOOLS/go1.27.3" --strip-components=1
    fi
    [[ $("$TOOLS/go1.27.3/bin/go" version) == 'go version go1.27.3 linux/arm64' ]] || refuse go_version_refused
}

prepare_rust() {
    local stem=rust-1.94.1-aarch64-unknown-linux-gnu
    local archive=$TOOLS/downloads/$stem.tar.xz
    local checksum=$archive.sha256
    if [[ ! -e $checksum ]]; then
        local staged_checksum=$checksum.next
        rm -f -- "$staged_checksum"
        curl --fail --silent --show-error --max-time 60 "https://static.rust-lang.org/dist/$stem.tar.xz.sha256" -o "$staged_checksum"
        grep -q '^[0-9a-f]\{64\} ' "$staged_checksum" || { rm -f -- "$staged_checksum"; refuse rust_checksum_refused; }
        mv -- "$staged_checksum" "$checksum"
    fi
    local expected
    expected=$(cut -d ' ' -f1 "$checksum")
    [[ $expected =~ ^[0-9a-f]{64}$ ]] || refuse rust_checksum_refused
    download "https://static.rust-lang.org/dist/$stem.tar.xz" "$archive" "$expected"
    if [[ ! -e $TOOLS/rust-1.94.1 ]]; then
        tar -xJf "$archive" -C "$TOOLS/downloads"
        "$TOOLS/downloads/$stem/install.sh" --prefix="$TOOLS/rust-1.94.1" --without=rust-docs --disable-ldconfig
    fi
    [[ $("$TOOLS/rust-1.94.1/bin/rustc" --version) == 'rustc 1.94.1 '* ]] || refuse rust_version_refused
    [[ $("$TOOLS/rust-1.94.1/bin/cargo" --version) == 'cargo 1.94.1 '* ]] || refuse cargo_version_refused
}

ensure_directory() {
    local path=$1 mode=$2 owner=$3
    if [[ -e $path || -L $path ]]; then
        [[ -d $path && ! -L $path && $(stat -c %u "$path") == $(id -u "$owner") && $(stat -c %a "$path") == "$mode" ]] || refuse host_directory_collision
    else
        install -d -m "$mode" -o "$owner" -g "$owner" "$path"
    fi
}

prepare_account() {
    local uid_record=$ROOT/.core-account-uid
    if id "$CORE" >/dev/null 2>&1; then
        [[ -f $uid_record && ! -L $uid_record ]] || refuse core_account_collision
        [[ $(cat "$uid_record") == $(id -u "$CORE") ]] || refuse core_identity_refused
    else
        [[ ! -e $uid_record ]] || refuse core_identity_refused
        useradd --system --home-dir /nonexistent --no-create-home --shell /usr/sbin/nologin "$CORE"
        id -u "$CORE" > "$uid_record"
        chmod 0400 "$uid_record"
    fi
    [[ $(id -u "$CORE") -ne 0 ]] || refuse core_identity_refused
    if id -nG "$CORE" | tr ' ' '\n' | grep -Eq '^(root|sudo|docker|adm|lxd)$'; then refuse core_privilege_refused; fi
    for parent in /etc/duskweave /usr/local/libexec/duskweave /run/duskweave; do ensure_directory "$parent" 755 root; done
    local config_owner=/etc/duskweave/m1/.live-dns-owner
    if [[ -e /etc/duskweave/m1 && ! -f $config_owner ]]; then refuse host_config_collision; fi
    ensure_directory /etc/duskweave/m1 755 root
    if [[ ! -e $config_owner ]]; then printf '%s\n' DW-IMPLEMENT-M1-LIVE-DNS-GENAI > "$config_owner"; chmod 0400 "$config_owner"; fi
    [[ ! -L $config_owner && $(cat "$config_owner") == DW-IMPLEMENT-M1-LIVE-DNS-GENAI ]] || refuse host_config_collision
    ensure_directory /run/duskweave/m1 700 "$CORE"
    ensure_directory /run/duskweave/launcher 700 root
    local temporary
    temporary=$(mktemp)
    printf 'd /run/duskweave/m1 0700 %s %s -\nd /run/duskweave/launcher 0700 root root -\n' "$CORE" "$CORE" > "$temporary"
    if [[ -e /etc/tmpfiles.d/duskweave-m1.conf ]] && ! cmp -s "$temporary" /etc/tmpfiles.d/duskweave-m1.conf; then
        rm -- "$temporary"; refuse control_config_collision
    fi
    install -m 0644 "$temporary" /etc/tmpfiles.d/duskweave-m1.conf
    rm -- "$temporary"
}

prepare_raw_sinks() {
    if [[ ! -e $REPORT/raw-sink-settings.before ]]; then
        { sysctl kernel.core_pattern fs.suid_dumpable; swapon --show --noheadings --bytes; } > "$REPORT/raw-sink-settings.before"
        systemctl is-enabled apport.service swapfile.swap > "$REPORT/service-activation.before" 2>&1 || true
        awk '$1=="/swapfile" && $3=="swap" {print}' /etc/fstab > "$REPORT/swap-activation.before"
        if [[ -f /etc/default/apport ]]; then grep '^enabled=' /etc/default/apport > "$REPORT/apport-enabled.before" || true; fi
    fi
    if awk 'NR>1 {print $1}' /proc/swaps | grep -qvx /swapfile; then refuse unexpected_swap_refused; fi
    if awk 'NR>1 {print $1}' /proc/swaps | grep -qx /swapfile; then swapoff /swapfile; fi
    # Preserve every unrelated mount line and retain only the non-sensitive swap line above.
    # Replace fstab through an owned same-directory file, then atomic rename so a
    # killed run never leaves a truncated file behind.
    local temporary=/etc/fstab.next
    awk '!($1=="/swapfile" && $3=="swap")' /etc/fstab > "$temporary"
    chown --reference=/etc/fstab "$temporary"
    chmod --reference=/etc/fstab "$temporary"
    mv -- "$temporary" /etc/fstab
    systemctl mask --now swapfile.swap
    if [[ -f /etc/default/apport ]]; then sed -i 's/^enabled=1$/enabled=0/' /etc/default/apport; fi
    systemctl mask --now apport.service
    local settings=/etc/sysctl.d/70-duskweave-m1-raw-boundary.conf
    temporary=$(mktemp)
    printf 'fs.suid_dumpable=0\nkernel.core_pattern=|/bin/false\n' > "$temporary"
    if [[ -e $settings ]] && ! cmp -s "$temporary" "$settings"; then
        rm -- "$temporary"; refuse raw_sink_config_collision
    fi
    install -m 0644 "$temporary" "$settings"
    rm -- "$temporary"
    sysctl -p "$settings"
    [[ -z $(awk 'NR>1 {print $1}' /proc/swaps) ]] || refuse active_swap_refused
    [[ $(sysctl -n fs.suid_dumpable) == 0 ]] || refuse dumpability_policy_refused
    [[ $(sysctl -n kernel.core_pattern) == '|/bin/false' ]] || refuse core_sink_refused
}

for binary in curl tar xz sha256sum gcc pkg-config docker nft runc; do command -v "$binary" >/dev/null || refuse build_dependency_missing; done
pkg-config --exists openssl || refuse openssl_headers_missing
if ip -4 route show table all | grep -Eq '(^|[[:space:]])10\.203\.'; then refuse boundary_route_collision; fi
prepare_go
prepare_rust
prepare_account
prepare_raw_sinks
printf '%s\n' 'host_preparation_complete live_qualified=false provider_contact=false'
