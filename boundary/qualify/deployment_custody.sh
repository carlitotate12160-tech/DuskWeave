# Protected deployment-path custody and the frozen-bundle file/digest contract.
# Sourced by boundary/qualify/qualify.sh and https_lab.sh; refusal names live in callers.

# Canonical frozen-bundle map: sha256 + bundle-relative path. Membership and
# digest validation both consume this one owner; bind selects its four binaries.
DEPLOYMENT_BUNDLE_SUMS='e4aad5ca68c08bde2dd283b204014d55a5fd735fd7058ab55adbb8e054e91012  duskweave
159b40330a72d0d4f8ec77e0c057e82dbd6155f5ace7df8023dcb7f791ba959b  dw-boundary-launcher
f15c975c544265f356bc3fb052225003f9f6ba18ad9965c948ed15b6477ed06a  dw-dns-capture
61d4a07998164143f17e2b446e2da4bf8bb17d0093dfe528afac3b6d28ae0fde  dw-dns-exporter
b30eaf580f67b95e5cec404a129a50a345536a07ae22bd11fef31e62c4694212  third_party/genai/LICENSE-APACHE
8d8c7da8bba808062a123cdfe7ed16825c4f075f70473cbdfc8bd44f21f1ce1f  third_party/genai/LICENSE-MIT
58aa7c18c1d49f3376f3ab5e8055573e90b76578f94179f4814f65c99a56d66b  third_party/genai/source-manifest.json'
DEPLOYMENT_BUNDLE_DIRS=$'third_party\nthird_party/genai'

protected_dir() {
    # Real non-symlink root-owned directory without group/world write.
    [[ -d $1 && ! -L $1 ]] || return 1
    [[ $(stat -c %u "$1") == 0 ]] || return 1
    (((8#$(stat -c %a "$1") & 022) == 0))
}

protected_ancestors() {
    # Every ancestor directory of $1 through / is a protected directory.
    local dir=$1
    while [[ $dir == /*/* ]]; do
        dir=${dir%/*}
        protected_dir "$dir" || return 1
    done
    protected_dir /
}

protected_regular_file() {
    # Non-symlink root-owned regular file: nlink 1, no group/world write.
    [[ -f $1 && ! -L $1 ]] || return 1
    [[ $(stat -c %u "$1") == 0 && $(stat -c %h "$1") == 1 ]] || return 1
    (((8#$(stat -c %a "$1") & 022) == 0))
}

protected_fresh_destination() {
    # An absent absolute destination below protected ancestors.
    [[ $1 == /* && ! -e $1 && ! -L $1 ]] || return 1
    protected_ancestors "$1"
}

protected_report_destination() {
    # Fresh destination, or an existing root:root/0700 evidence directory,
    # under protected ancestors either way.
    if protected_fresh_destination "$1"; then return 0; fi
    [[ $1 == /* && -d $1 && ! -L $1 ]] || return 1
    [[ $(stat -c %u "$1") == 0 && $(stat -c %g "$1") == 0 && $(stat -c %a "$1") == 700 ]] || return 1
    protected_ancestors "$1"
}

protected_source_tree() {
    # Release-export custody for an explicit LAB SOURCE: protected ancestry,
    # the executed bootstrap helper and every migration input as regular files.
    local entry sql=0
    protected_ancestors "$1/tests/support/m1_native_bootstrap.py" || return 1
    protected_regular_file "$1/tests/support/m1_native_bootstrap.py" || return 1
    protected_dir "$1/migrations" || return 1
    while IFS= read -r entry; do
        [[ $entry == *.sql ]] && sql=1
        protected_regular_file "$1/migrations/$entry" || return 1
    done < <(find "$1/migrations" -mindepth 1 -maxdepth 1 -printf '%f\n' 2>/dev/null | LC_ALL=C sort)
    ((sql))
}

validate_bundle_custody() {
    # Bundle root:root identity, protected ancestors and protected member dirs.
    local dir
    protected_dir "$1" || return 1
    [[ $(stat -c %g "$1") == 0 ]] || return 1
    protected_ancestors "$1" || return 1
    while IFS= read -r dir; do
        protected_dir "$1/$dir" || return 1
        [[ $(stat -c %g "$1/$dir") == 0 ]] || return 1
    done <<<"$DEPLOYMENT_BUNDLE_DIRS"
}

validate_bundle_membership() {
    # Observed entries must be exactly the pinned members or their parent dirs;
    # extras, hidden files, symlinked dirs and nested entries refuse.
    local entry
    while IFS= read -r entry; do
        entry=${entry#./}
        case $'\n'"$DEPLOYMENT_BUNDLE_DIRS"$'\n' in
            *$'\n'"$entry"$'\n'*)
                [[ -d $1/$entry && ! -L $1/$entry ]] || return 1 ;;
            *)
                case $'\n'"$DEPLOYMENT_BUNDLE_SUMS"$'\n' in
                    *[[:space:]]"$entry"$'\n'*) ;;
                    *) return 1 ;;
                esac ;;
        esac
    done < <(cd -- "$1" && find . -mindepth 1 | LC_ALL=C sort)
}

validate_bundle_members() {
    # Every pinned member is a protected regular file.
    local _sum member
    while read -r _sum member; do
        protected_regular_file "$1/$member" || return 1
    done <<<"$DEPLOYMENT_BUNDLE_SUMS"
}

validate_bundle_digests() {
    (cd -- "$1" && sha256sum --check --strict - >/dev/null 2>&1 <<<"$DEPLOYMENT_BUNDLE_SUMS")
}

qualify_check_driver_file() {
    local p=$1 enforce_root=$2
    if [[ $enforce_root == 1 ]]; then
        protected_ancestors "$p" && protected_regular_file "$p" || { refuse driver_custody_refused; return 1; }
    elif [[ $enforce_root == 0 ]]; then
        [[ -f $p && ! -L $p && $(stat -c %h "$p") == 1 ]] || { refuse driver_custody_refused; return 1; }
    else
        refuse driver_custody_refused; return 1
    fi
}

qualify_verify_driver_before_load() {
    local qdir=$1 commit=$2 tree=$3 enforce_root=${4:-1} repo f st hd tr
    [[ $commit =~ ^[0-9a-f]{40}$ && $tree =~ ^[0-9a-f]{40}$ ]] || { refuse driver_pin_malformed; return 1; }
    repo=$(cd -- "$qdir/../.." && pwd -P) || { refuse driver_custody_refused; return 1; }
    for f in deployment_custody.sh qualify.sh candidate.sh release_descriptor.py candidate_files.py \
             candidate_postgres.py candidate_host.py candidate_install.py candidate_install_files.py \
             candidate_database.py candidate_binding.py; do
        qualify_check_driver_file "$qdir/$f" "$enforce_root" || return 1
    done
    qualify_check_driver_file "$repo/tests/support/m1_native_bootstrap.py" "$enforce_root" || return 1
    st=$(env -i PATH=/usr/bin:/bin LC_ALL=C git -C "$repo" status --porcelain=v1 --untracked-files=all) \
        || { refuse driver_git_failed; return 1; }
    [[ -z $st ]] || { refuse driver_worktree_unclean; return 1; }
    hd=$(env -i PATH=/usr/bin:/bin LC_ALL=C git -C "$repo" rev-parse HEAD) || { refuse driver_git_failed; return 1; }
    tr=$(env -i PATH=/usr/bin:/bin LC_ALL=C git -C "$repo" rev-parse 'HEAD^{tree}') || { refuse driver_git_failed; return 1; }
    [[ $hd == "$commit" && $tr == "$tree" ]] || { refuse driver_git_pin_mismatch; return 1; }
    if ! python3 -I -B - "$qdir" "$repo" "$commit" "$enforce_root" <<'PY'
import os, pathlib, sys
sys.path.insert(0, sys.argv[1])
from candidate_files import FileCustodyError, verify_cargo_dir_custody
repo = pathlib.Path(sys.argv[2])
enforce = sys.argv[4] == "1"
uids = (0,) if enforce else (os.geteuid(),)
stop_at = pathlib.Path("/") if enforce else repo
try:
    verify_cargo_dir_custody(
        repo, commit=sys.argv[3], expected_uids=uids, stop_at=stop_at,
        require_git=True, reason="driver_cargo_config_present",
    )
except FileCustodyError:
    raise SystemExit(1)
PY
    then
        refuse driver_cargo_config_present
        return 1
    fi
}
