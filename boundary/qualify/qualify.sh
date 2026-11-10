#!/usr/bin/env bash
# Bounded preparation and qualification for this deployment, never external target scope.
set -euo pipefail
umask 077
ROOT=/srv/duskweave
BUILD=/srv/duskweave-build
SOURCE=$ROOT/src/dns-exporter-attach-20261103
CONTAINER=dw-live-dns-20261030-pg
VOLUME=dw-live-dns-20261030-pg17
IMAGE=postgres@sha256:ac359420747e1f5f8ac4d23f40d7ce022c0a39090ed6cdd8dde5f596affbe41d
SECRETS=/etc/duskweave/m1/service
RELEASE_IMAGE=dw-live-dns-20261030:candidate
# Frozen final-release bundle identity; it is consumed, never rebuilt or relabeled.
BUNDLE_SUMS='e4aad5ca68c08bde2dd283b204014d55a5fd735fd7058ab55adbb8e054e91012  duskweave
159b40330a72d0d4f8ec77e0c057e82dbd6155f5ace7df8023dcb7f791ba959b  dw-boundary-launcher
f15c975c544265f356bc3fb052225003f9f6ba18ad9965c948ed15b6477ed06a  dw-dns-capture
61d4a07998164143f17e2b446e2da4bf8bb17d0093dfe528afac3b6d28ae0fde  dw-dns-exporter'
refuse() { printf '%s\n' "$1" >&2; exit 1; }
valid_image_ref() {
    # One explicit unqualified name:tag; never a registry/path or digest form.
    [[ $1 =~ ^[a-z0-9][a-z0-9_.-]*:[A-Za-z0-9_][A-Za-z0-9_.-]{0,127}$ ]]
}
valid_lab_destination() {
    # An absent absolute destination below a real root-owned directory.
    [[ $1 == /* && ! -e $1 && ! -L $1 ]] || return 1
    local parent=${1%/*}
    [[ -d $parent && ! -L $parent && $(stat -c %u "$parent") == 0 ]]
}
require_deployment_host() {
    [[ $EUID == 0 && $(hostname) == pica-arm-01 && $(uname -m) == aarch64 ]] || refuse host_identity_refused
    [[ -f $ROOT/.live-dns-20261030-owner ]] || refuse deployment_owner_refused
    [[ $(cat "$ROOT/.live-dns-20261030-owner") == DW-IMPLEMENT-M1-LIVE-DNS-GENAI ]] || refuse deployment_owner_refused
}

# Fixed output/cache/temp components consumed by compiler, Go and Docker steps.
STORAGE_COMPONENTS="normal normal/release normal/release/build normal/release/deps normal/release/examples normal/release/incremental normal/release/.fingerprint go-cache tmp tmp/go tmp/build go-release boundary-release coverage"

require_storage_component() {
    # An absent fixed component is allowed; an existing one must be a real
    # root-owned directory on the build filesystem, never a symlink, file,
    # foreign device or nested mount point.
    local path=$1
    [[ -e $path || -L $path ]] || return 0
    [[ -d $path && ! -L $path ]] || refuse build_storage_content_refused
    [[ $(stat -c %u "$path") == 0 && $(( (8#$(stat -c %a "$path") & 022) == 0 )) == 1 ]] || refuse build_storage_content_refused
    [[ $(stat -c %d "$path") == $(stat -c %d "$BUILD") ]] || refuse build_storage_coupled_filesystem
    ! mountpoint -q "$path" || refuse build_storage_coupled_filesystem
}

check_storage_components() {
    local component
    for component in $STORAGE_COMPONENTS; do
        require_storage_component "$BUILD/$component"
    done
}

install_storage_dir() {
    # Newly created fixed directories receive the same validation as existing
    # ones before any compiler/Docker consumer touches them.
    local path=$1 current
    install -d -m 0755 "$path"
    current=$path
    while [[ $current == "$BUILD"/* ]]; do
        require_storage_component "$current"
        current=${current%/*}
    done
}

check_build_storage() {
    [[ -d $BUILD && ! -L $BUILD ]] || refuse build_storage_absent
    mountpoint -q "$BUILD" || refuse build_storage_unmounted
    local device coupled
    device=$(stat -c %d "$BUILD")
    for coupled in / /var/lib/docker /var/lib/containerd; do
        [[ $(stat -c %d "$coupled") != "$device" ]] || refuse build_storage_coupled_filesystem
    done
    [[ $(findmnt --noheadings --output FSTYPE --target "$BUILD") == ext4 ]] || refuse build_storage_filesystem_refused
    [[ $(stat -c %u "$BUILD") == 0 && $(( (8#$(stat -c %a "$BUILD") & 022) == 0 )) == 1 ]] || refuse build_storage_identity_refused
    local entry
    for entry in "$BUILD"/* "$BUILD"/.[!.]* "$BUILD"/..?*; do
        [[ -e $entry || -L $entry ]] || continue
        case ${entry##*/} in
            lost+found|normal|go-cache|tmp|go-release|boundary-release|coverage) ;;
            *) refuse build_storage_content_refused ;;
        esac
    done
    [[ -z $(find "$BUILD" -mindepth 1 -maxdepth 1 \( ! -type d -o ! -uid 0 -o -perm /022 \) -print -quit) ]] || refuse build_storage_content_refused
    check_storage_components
    printf 'build_storage_ready source=%s device=%s fstype=ext4\n' "$(findmnt --noheadings --output SOURCE --target "$BUILD")" "$(findmnt --noheadings --output MAJ:MIN --target "$BUILD")"
}

prepare_service() {
    if ! docker image inspect "$IMAGE" >/dev/null 2>&1; then docker pull "$IMAGE"; fi
    [[ $(docker image inspect --format '{{.Architecture}}' "$IMAGE") == arm64 ]] || refuse postgres_platform_refused
    if docker inspect "$CONTAINER" >/dev/null 2>&1; then
        [[ $(docker inspect --format '{{.Config.Image}}' "$CONTAINER") == "$IMAGE" ]] || refuse postgres_service_collision
        [[ $(docker inspect --format '{{index .Config.Labels "org.duskweave.owner"}}' "$CONTAINER") == live-dns-20261030 ]] || refuse postgres_service_collision
        if [[ $(docker inspect --format '{{.State.Running}}' "$CONTAINER") != true ]]; then docker start "$CONTAINER" >/dev/null; fi
    else
        [[ -z $(ss -H -ltn 'sport = :5432') ]] || refuse postgres_port_collision
        if docker volume inspect "$VOLUME" >/dev/null 2>&1; then
            [[ $(docker volume inspect --format '{{index .Labels "org.duskweave.owner"}}' "$VOLUME") == live-dns-20261030 ]] || refuse postgres_volume_collision
        else
            docker volume create --label org.duskweave.owner=live-dns-20261030 "$VOLUME" >/dev/null
        fi
        if [[ -e $SECRETS ]]; then
            [[ -d $SECRETS && ! -L $SECRETS && $(stat -c %u "$SECRETS") == 0 && $(stat -c %a "$SECRETS") == 700 ]] || refuse postgres_config_collision
        else
            install -d -m 0700 "$SECRETS"
        fi
        python3 - "$SECRETS/admin-password" <<'PY'
import os,secrets,sys
path=sys.argv[1]
if not os.path.exists(path):
    fd=os.open(path,os.O_WRONLY|os.O_CREAT|os.O_EXCL,0o400)
    with os.fdopen(fd,'w') as stream: stream.write(secrets.token_urlsafe(48)+'\n')
metadata=os.lstat(path)
if metadata.st_uid!=0 or metadata.st_mode&0o7777!=0o400 or metadata.st_nlink!=1:
    raise SystemExit('postgres_secret_file_refused')
PY
        docker run -d --name "$CONTAINER" --label org.duskweave.owner=live-dns-20261030 \
            --publish 127.0.0.1:5432:5432 --log-driver none --memory 1g --memory-swap 1g \
            --mount "type=volume,src=$VOLUME,dst=/var/lib/postgresql/data" \
            --mount "type=bind,src=$SECRETS/admin-password,dst=/run/secrets/admin-password,readonly" \
            --env POSTGRES_PASSWORD_FILE=/run/secrets/admin-password "$IMAGE" \
            postgres -c log_statement=none -c log_parameter_max_length=0 \
            -c log_parameter_max_length_on_error=0 -c log_min_error_statement=panic \
            -c log_min_messages=panic -c logging_collector=off >/dev/null
    fi
    local bound=$((SECONDS+30))
    until docker exec "$CONTAINER" pg_isready -U postgres >/dev/null 2>&1; do
        (( SECONDS < bound )) || refuse postgres_readiness_failed
        sleep 1
    done
    local version
    version=$(docker exec "$CONTAINER" psql -X -U postgres -Atc "SELECT current_setting('server_version_num')")
    [[ $version == 170011 ]] || refuse postgres_version_refused
    printf '%s\n' 'postgres_service_prepared version=17.11 physical_live_qualified=false'
}

build_release() {
    local image_ref=$1
    check_build_storage
    export PATH=$ROOT/toolchains/rust-1.94.1/bin:$ROOT/toolchains/go1.27.3/bin:$PATH
    export CARGO_HOME=$ROOT/build/cargo-home CARGO_TARGET_DIR=$BUILD/normal
    export GOCACHE=$BUILD/go-cache GOTMPDIR=$BUILD/tmp/go TMPDIR=$BUILD/tmp/build
    for component_dir in "$CARGO_TARGET_DIR" "$GOCACHE" "$GOTMPDIR" "$TMPDIR"; do
        install_storage_dir "$component_dir"
    done
    cd "$SOURCE"
    cargo build --locked --offline --release --features fixture-supervisor --bin duskweave --bin dw-boundary-launcher
    local output=$BUILD/go-release
    install_storage_dir "$output"
    cd boundary
    for arch in arm64 amd64; do
        CGO_ENABLED=0 GOOS=linux GOARCH=$arch go build -trimpath -o "$output/dw-dns-capture-$arch" ./cmd/dw-dns-capture
        CGO_ENABLED=0 GOOS=linux GOARCH=$arch go build -trimpath -o "$output/dw-dns-exporter-$arch" ./cmd/dw-dns-exporter
    done
    CGO_ENABLED=0 go build -trimpath -o "$output/dns-lab" ./qualify/dns_lab.go
    local context=$BUILD/boundary-release
    install_storage_dir "$context"
    install -m 0555 "$output/dw-dns-capture-arm64" "$context/dw-dns-capture"
    install -m 0555 "$output/dw-dns-exporter-arm64" "$context/dw-dns-exporter"
    install -m 0444 Dockerfile "$context/Dockerfile"
    docker build --network none --tag "$image_ref" "$context"
}

install_release() {
    check_build_storage
    local bundle=${1:-} bin=/usr/local/libexec/duskweave marker=/etc/duskweave/m1/.live-dns-owner
    [[ ! -L $marker && $(cat "$marker") == DW-IMPLEMENT-M1-LIVE-DNS-GENAI ]] || refuse deployment_owner_refused
    [[ $(id -u duskweave-core) == $(cat "$ROOT/.core-account-uid") ]] || refuse core_identity_refused
    if id -nG duskweave-core | tr ' ' '\n' | grep -Eq '^(root|sudo|docker|adm|lxd)$'; then refuse core_privilege_refused; fi
    local names=(duskweave dw-boundary-launcher dw-dns-capture dw-dns-exporter) sources index
    if [[ -n $bundle ]]; then
        sources=("$bundle/duskweave" "$bundle/dw-boundary-launcher" "$bundle/dw-dns-capture" "$bundle/dw-dns-exporter")
    else
        sources=("$BUILD/normal/release/duskweave" "$BUILD/normal/release/dw-boundary-launcher" "$BUILD/go-release/dw-dns-capture-arm64" "$BUILD/go-release/dw-dns-exporter-arm64")
    fi
    local policy temporary
    policy='duskweave-core ALL=(root) NOPASSWD: /usr/local/libexec/duskweave/dw-boundary-launcher ""'
    temporary=$(mktemp)
    printf '%s\n' "$policy" > "$temporary"
    /usr/sbin/visudo -cf "$temporary" >/dev/null
    if [[ -e /etc/sudoers.d/duskweave-m1-launcher ]] && ! cmp -s "$temporary" /etc/sudoers.d/duskweave-m1-launcher; then
        rm -- "$temporary"; refuse launcher_policy_collision
    fi
    for index in "${!names[@]}"; do
        [[ ! -L $bin/${names[$index]} ]] || refuse release_path_collision
        install -m 0555 "${sources[$index]}" "$bin/${names[$index]}"
    done
    install -m 0440 "$temporary" /etc/sudoers.d/duskweave-m1-launcher
    rm -- "$temporary"
    /usr/sbin/visudo -c >/dev/null
    printf '%s\n' 'release_installed qualification_pending=true'
}

install_bundle() {
    # Frozen-bundle identity and membership checks complete before any install.
    local bundle=$1 name path entry
    [[ $(stat -c %u:%g "$bundle") == 0:0 ]] || refuse bundle_refused
    for entry in "$bundle"/* "$bundle"/.[!.]* "$bundle"/..?*; do
        [[ -e $entry || -L $entry ]] || continue
        case ${entry##*/} in duskweave|dw-boundary-launcher|dw-dns-capture|dw-dns-exporter) ;; *) refuse bundle_membership_refused ;; esac
    done
    for name in duskweave dw-boundary-launcher dw-dns-capture dw-dns-exporter; do
        path=$bundle/$name
        [[ -f $path && ! -L $path && $(stat -c %u "$path") == 0 && $(stat -c %h "$path") == 1 && $(( (8#$(stat -c %a "$path") & 022) == 0 )) == 1 ]] || refuse bundle_member_refused
    done
    (cd -- "$bundle" && sha256sum --check --strict - >/dev/null 2>&1 <<<"$BUNDLE_SUMS") || refuse bundle_digest_mismatch
    install_release "$bundle"
    printf '%s\n' 'bundle_installed qualification_pending=true'
}

prepare_lab_database() {
    python3 - "$@" <<'PY'
import importlib.util,json,os,pathlib,pwd,subprocess,sys
def secret(path,data,owner):
    fd=os.open(path,os.O_WRONLY|os.O_CREAT|os.O_EXCL,0o400)
    with os.fdopen(fd,'w') as stream: stream.write(data)
    if owner is not None: os.chown(path,owner,owner)
if len(sys.argv)==1:
    source,database,role_prefix=(pathlib.Path('/srv/duskweave/src/dns-exporter-attach-20261103'),
                                 'dw_native_dns_attach_20261103a','dw_da1103a')
    config=pathlib.Path('/etc/duskweave/m1/lab-dns-attach-20261103a');fresh=False
elif len(sys.argv)==5:
    source,database,role_prefix,config=(pathlib.Path(sys.argv[1]),sys.argv[2],sys.argv[3],pathlib.Path(sys.argv[4]))
    fresh=True
    try: safe=config.parent.is_dir() and config.parent.lstat().st_uid==0
    except OSError: safe=False
    if not safe or not config.is_absolute() or config.exists() or config.is_symlink():
        raise SystemExit('lab_config_collision')
else: raise SystemExit('qualification_arguments_refused')
spec=importlib.util.spec_from_file_location('native_bootstrap',source/'tests/support/m1_native_bootstrap.py')
module=importlib.util.module_from_spec(spec);spec.loader.exec_module(module)
inherited=dict(os.environ)
fixture=module.Fixture('dw-live-dns-20261030-pg','fixture',database=database,
                       schema_revision='0030',role_prefix=role_prefix)
core=pwd.getpwnam('duskweave-core')
config.mkdir(mode=0o700)
if fresh:
    os.chown(config,0,core.pw_gid);os.chmod(config,0o750)
    meta=os.stat(config)
    if meta.st_mode&0o777!=0o750 or meta.st_uid!=0 or meta.st_gid!=core.pw_gid:
        raise SystemExit('lab_config_mode_refused')
else: os.chown(config,core.pw_uid,core.pw_uid)
owners=[('core','DW_DATABASE_URL'),('broker','DW_INFERENCE_BROKER_DATABASE_URL'),
        ('cognition','DW_COGNITION_DATABASE_URL'),('history','DW_TRAJECTORY_DATABASE_URL'),
        ('terrain','DW_TERRAIN_DATABASE_URL'),('stop','DW_STOP_WRITER_DATABASE_URL')]
variables={}
for owner,var in owners:
    path=config/(owner+'.dsn');secret(path,fixture.env['DW_TEST_DATABASE_URL' if owner=='core' else var],core.pw_uid)
    variables[var+'_FILE']=str(path)
variables.update(DW_DATABASE_CONFIG_MODE='file',DW_M1_RUN_CONTROL_ROOT='/run/duskweave/m1')
created={k:v for k,v in fixture.env.items() if k.startswith('DW_') and
         (k.endswith('_DATABASE_URL') or k in ('DW_DATABASE_URL','DW_DATABASE_CONFIG_MODE','DW_NATIVE_FIXTURE_DATABASE'))}
if fresh: created={k:v for k,v in created.items() if inherited.get(k)!=v}
test=config/'test-environment.json';secret(test,json.dumps(created),None)
runtime=config/'environment.json';secret(runtime,json.dumps(variables),core.pw_uid)
if fresh:
    def core_can(op,p): return subprocess.run(['/usr/sbin/runuser','-u','duskweave-core','--','/usr/bin/test',op,str(p)],capture_output=True).returncode==0
    if any(not core_can('-r',p) for p in [config/(o+'.dsn') for o,_ in owners]+[runtime]) \
            or core_can('-r',test) or core_can('-w',config):
        raise SystemExit('lab_runtime_access_refused')
print('lab_database_provisioned credentials_printed=false')
PY
}

bind_lab_release() {
    BUNDLE_SUMS=$BUNDLE_SUMS python3 - "$@" <<'PY'
import hashlib,json,os,pathlib,pwd,shutil,subprocess,sys,time,uuid
if len(sys.argv)==1:
    image_ref,database='dw-live-dns-20261030:candidate','dw_native_dns_attach_20261103a'
    report=pathlib.Path('/srv/duskweave/reports/live-dns-20261030/completion-20261031a/dns-attach-20261103a')
    frozen=None
elif len(sys.argv)==4:
    image_ref,database,report=sys.argv[1],sys.argv[2],pathlib.Path(sys.argv[3])
    dig={n:h for h,n in (l.split() for l in os.environ['BUNDLE_SUMS'].splitlines())}
    frozen={'release_sha256':dig['duskweave'],'launcher_sha256':dig['dw-boundary-launcher'],
            'capture_sha256':dig['dw-dns-capture'],'exporter_sha256':dig['dw-dns-exporter']}
    if not report.is_absolute() or report.is_symlink(): raise SystemExit('lab_report_collision')
    if report.exists():
        m=report.lstat()
        if m.st_uid!=0 or m.st_gid!=0 or m.st_mode&0o777!=0o700 or not report.is_dir():
            raise SystemExit('lab_report_collision')
    elif not report.parent.is_dir() or report.parent.lstat().st_uid!=0:
        raise SystemExit('lab_report_collision')
else: raise SystemExit('qualification_arguments_refused')
root=pathlib.Path('/usr/local/libexec/duskweave')
base=pathlib.Path('/etc/duskweave/m1')
assert (base/'.live-dns-owner').read_text().strip()=='DW-IMPLEMENT-M1-LIVE-DNS-GENAI'
manifest={name+'_sha256':hashlib.sha256((root/binary).read_bytes()).hexdigest()
          for name,binary in [('release','duskweave'),('launcher','dw-boundary-launcher'),
                              ('capture','dw-dns-capture'),('exporter','dw-dns-exporter')]}
if frozen is not None and any(manifest[k]!=v for k,v in frozen.items()):
    raise SystemExit('lab_installed_release_mismatch')
image=subprocess.check_output(['docker','image','inspect','--format','{{.Id}}',image_ref],text=True).strip()
assert image.startswith('sha256:') and len(image)==71
# Bind the actual worker bytes inside this exact image: a never-started,
# network-none, owned inspection container yields only the two fixed paths.
inspect_dir=pathlib.Path('/tmp/dw-bind-inspect-'+uuid.uuid4().hex)
inspect_dir.mkdir(mode=0o700)
inspect_name='dw-bind-inspect-'+uuid.uuid4().hex[:12]
def confirm_removed():
    subprocess.run(['docker','rm','-f','-v',inspect_name],capture_output=True,timeout=60)
    if subprocess.run(['docker','inspect',inspect_name],capture_output=True,timeout=60).returncode==0:
        raise SystemExit('lab_bind_cleanup_unconfirmed')
try:
    subprocess.run(['docker','create','--name',inspect_name,'--network','none',image,'/bin/true'],
                   check=True,capture_output=True,timeout=60)
    for binary,key in [('dw-dns-capture','capture_sha256'),('dw-dns-exporter','exporter_sha256')]:
        target=inspect_dir/binary
        subprocess.run(['docker','cp',inspect_name+':/'+binary,str(target)],
                       check=True,capture_output=True,timeout=60)
        if target.stat().st_size>64*1024*1024:
            raise SystemExit('lab_image_worker_size_refused:'+binary)
        if hashlib.sha256(target.read_bytes()).hexdigest()!=manifest[key]:
            raise SystemExit('lab_image_worker_mismatch:'+binary)
    confirm_removed()
except SystemExit:
    confirm_removed()
    raise
except Exception:
    confirm_removed()
    raise SystemExit('lab_image_inspection_failed')
finally:
    shutil.rmtree(inspect_dir,ignore_errors=True)
now=time.time_ns()//1000000
manifest.update(qualification_ref=str(uuid.uuid4()),purpose='LAB',run_profile='autonomous-dns-genai-lab-v1',
                image_digest=image,boot_id=pathlib.Path('/proc/sys/kernel/random/boot_id').read_text().strip(),
                resolver_ipv4='192.0.2.53',valid_from_ms=now,valid_until_ms=now+86400000,
                core_uid=pwd.getpwnam('duskweave-core').pw_uid)
encoded=(json.dumps(manifest,sort_keys=True,separators=(',',':'))+chr(10)).encode()
columns=['qualification_ref','run_profile','purpose','release_sha256','launcher_sha256','capture_sha256',
         'exporter_sha256','image_digest','manifest_sha256','boot_id','resolver_ipv4','valid_from_ms','valid_until_ms']
record=dict(manifest,manifest_sha256=hashlib.sha256(encoded).hexdigest())
values=[str(record[k]) for k in columns]
assert all("'" not in v for v in values)
sql="INSERT INTO execution.live_qualifications ("+','.join(columns)+") VALUES ("+','.join("'"+v+"'" for v in values)+");"
r=subprocess.run(['docker','exec','-i','dw-live-dns-20261030-pg','psql','-X','-U','postgres',
                  '-d',database,'-v','ON_ERROR_STOP=1','-At'],
                 input=sql,capture_output=True,text=True)
if r.returncode: raise SystemExit('lab_qualification_write_failed')
path=base/'deployment.json'
if path.exists():
 m=path.lstat()
 assert m.st_uid==0 and m.st_nlink==1 and m.st_mode&0o22==0 and not path.is_symlink()
temporary=base/'deployment.next'
fd=os.open(temporary,os.O_WRONLY|os.O_CREAT|os.O_EXCL,0o444)
with os.fdopen(fd,'wb') as stream:stream.write(encoded)
os.replace(temporary,path)
report.mkdir(mode=0o700,parents=True,exist_ok=True)
m=report.lstat()
assert m.st_uid==0 and m.st_mode&0o777==0o700 and not report.is_symlink()
(report/('deployment-'+manifest['qualification_ref']+'.json')).write_bytes(encoded)
print('lab_release_bound purpose=LAB qualification='+manifest['qualification_ref'])
PY
}

build_only() {
    local image_ref=$1 source_root
    valid_image_ref "$image_ref" || refuse qualification_arguments_refused
    source_root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd -P)
    check_build_storage
    if docker image inspect "$image_ref" >/dev/null 2>&1; then
        refuse build_image_collision
    fi
    SOURCE=$source_root GOPROXY=off build_release "$image_ref"
}

main() {
    require_deployment_host
    case "${1:-}" in
        service) [[ $# == 1 ]] || refuse qualification_arguments_refused; prepare_service ;;
        release) [[ $# == 1 ]] || refuse qualification_arguments_refused; build_release "$RELEASE_IMAGE"; install_release ;;
        lab-database)
            if [[ $# == 1 ]]; then prepare_lab_database
            elif [[ $# == 5 && $2 == /* && -d $2 && ! -L $2 && -f $2/tests/support/m1_native_bootstrap.py \
                    && $3 =~ ^dw_native_[a-z0-9_]{1,53}$ && $4 =~ ^dw_[a-z0-9_]{0,33}$ ]] \
                    && valid_lab_destination "$5"; then
                prepare_lab_database "$2" "$3" "$4" "$5"
            else refuse qualification_arguments_refused; fi ;;
        lab-bind)
            if [[ $# == 1 ]]; then bind_lab_release
            elif [[ $# == 4 && $3 =~ ^dw_native_[a-z0-9_]{1,53}$ ]] && valid_image_ref "$2" \
                    && { valid_lab_destination "$4" \
                        || [[ $4 == /* && -d $4 && ! -L $4 && $(stat -c %u "$4") == 0 && $(stat -c %g "$4") == 0 && $(stat -c %a "$4") == 700 ]]; }; then
                bind_lab_release "$2" "$3" "$4"
            else refuse qualification_arguments_refused; fi ;;
        install-bundle) [[ $# == 2 && $2 == /* && -d $2 && ! -L $2 ]] || refuse qualification_arguments_refused; install_bundle "$2" ;;
        build-storage) [[ $# == 1 ]] || refuse qualification_arguments_refused; check_build_storage ;;
        build-only) [[ $# == 2 ]] || refuse qualification_arguments_refused; build_only "$2" ;;
        *) refuse qualification_arguments_refused ;;
    esac
}
# Sourcing exposes definitions to focused tests without executing the CLI;
# direct invocation still enforces every real host/ownership/storage guard.
if [[ ${BASH_SOURCE[0]} == "$0" ]]; then
    main "$@"
fi
