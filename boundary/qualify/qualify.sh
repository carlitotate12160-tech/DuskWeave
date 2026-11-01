#!/usr/bin/env bash
# Bounded preparation and qualification for this deployment, never external target scope.
set -euo pipefail
umask 077
ROOT=/srv/duskweave
BUILD=/srv/duskweave-build
SOURCE=$ROOT/src/live-dns-20261030
REPORT=$ROOT/reports/live-dns-20261030
CONTAINER=dw-live-dns-20261030-pg
VOLUME=dw-live-dns-20261030-pg17
IMAGE=postgres@sha256:ac359420747e1f5f8ac4d23f40d7ce022c0a39090ed6cdd8dde5f596affbe41d
SECRETS=/etc/duskweave/m1/service
refuse() { printf '%s\n' "$1" >&2; exit 1; }
[[ $EUID == 0 && $(hostname) == pica-arm-01 && $(uname -m) == aarch64 ]] || refuse host_identity_refused
[[ -f $ROOT/.live-dns-20261030-owner ]] || refuse deployment_owner_refused
[[ $(cat "$ROOT/.live-dns-20261030-owner") == DW-IMPLEMENT-M1-LIVE-DNS-GENAI ]] || refuse deployment_owner_refused

# Fixed output/cache/temp components consumed by compiler, Go and Docker steps.
STORAGE_COMPONENTS="normal go-cache tmp tmp/go tmp/build go-release boundary-release coverage"

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
    docker build --network none --tag dw-live-dns-20261030:candidate "$context"
}

install_release() {
    check_build_storage
    local bin=/usr/local/libexec/duskweave marker=/etc/duskweave/m1/.live-dns-owner
    [[ ! -L $marker && $(cat "$marker") == DW-IMPLEMENT-M1-LIVE-DNS-GENAI ]] || refuse deployment_owner_refused
    [[ $(id -u duskweave-core) == $(cat "$ROOT/.core-account-uid") ]] || refuse core_identity_refused
    if id -nG duskweave-core | tr ' ' '\n' | grep -Eq '^(root|sudo|docker|adm|lxd)$'; then refuse core_privilege_refused; fi
    local policy temporary
    policy='duskweave-core ALL=(root) NOPASSWD: /usr/local/libexec/duskweave/dw-boundary-launcher ""'
    temporary=$(mktemp)
    printf '%s\n' "$policy" > "$temporary"
    /usr/sbin/visudo -cf "$temporary" >/dev/null
    if [[ -e /etc/sudoers.d/duskweave-m1-launcher ]] && ! cmp -s "$temporary" /etc/sudoers.d/duskweave-m1-launcher; then
        rm -- "$temporary"; refuse launcher_policy_collision
    fi
    for name in duskweave dw-boundary-launcher; do
        [[ ! -L $bin/$name ]] || refuse release_path_collision
        install -m 0555 "$BUILD/normal/release/$name" "$bin/$name"
    done
    for name in dw-dns-capture dw-dns-exporter; do
        [[ ! -L $bin/$name ]] || refuse release_path_collision
        install -m 0555 "$BUILD/go-release/$name-arm64" "$bin/$name"
    done
    install -m 0440 "$temporary" /etc/sudoers.d/duskweave-m1-launcher
    rm -- "$temporary"
    /usr/sbin/visudo -c >/dev/null
    printf '%s\n' 'release_installed qualification_pending=true'
}

prepare_lab_database() {
    cd "$SOURCE"
    python3 - <<'PY'
import importlib.util,json,os,pathlib,pwd
source=pathlib.Path('/srv/duskweave/src/live-dns-20261030')
spec=importlib.util.spec_from_file_location('native_bootstrap',source/'tests/support/m1_native_bootstrap.py')
module=importlib.util.module_from_spec(spec);spec.loader.exec_module(module)
fixture=module.Fixture('dw-live-dns-20261030-pg','fixture',database='dw_native_live_dns_20261031b',
                       schema_revision='0030',role_prefix='dw_ldns31')
uid=pwd.getpwnam('duskweave-core').pw_uid
config=pathlib.Path('/etc/duskweave/m1/lab-20261031b')
config.mkdir(mode=0o700)
os.chown(config,uid,uid)
variables={}
for owner,var in [('core','DW_DATABASE_URL'),('broker','DW_INFERENCE_BROKER_DATABASE_URL'),
                  ('cognition','DW_COGNITION_DATABASE_URL'),('history','DW_TRAJECTORY_DATABASE_URL'),
                  ('terrain','DW_TERRAIN_DATABASE_URL'),('stop','DW_STOP_WRITER_DATABASE_URL')]:
 value=fixture.env['DW_TEST_DATABASE_URL' if owner=='core' else var]
 path=config/(owner+'.dsn')
 fd=os.open(path,os.O_WRONLY|os.O_CREAT|os.O_EXCL,0o400)
 with os.fdopen(fd,'w') as stream: stream.write(value)
 os.chown(path,uid,uid)
 variables[var+'_FILE']=str(path)
variables.update(DW_DATABASE_CONFIG_MODE='file',DW_M1_RUN_CONTROL_ROOT='/run/duskweave/m1')
test=config/'test-environment.json'
fd=os.open(test,os.O_WRONLY|os.O_CREAT|os.O_EXCL,0o400)
test_env={k:v for k,v in fixture.env.items() if k.startswith('DW_') and
          (k.endswith('_DATABASE_URL') or k in ('DW_DATABASE_URL','DW_DATABASE_CONFIG_MODE','DW_NATIVE_FIXTURE_DATABASE'))}
with os.fdopen(fd,'w') as stream: json.dump(test_env,stream)
runtime=config/'environment.json'
fd=os.open(runtime,os.O_WRONLY|os.O_CREAT|os.O_EXCL,0o400)
with os.fdopen(fd,'w') as stream: json.dump(variables,stream)
os.chown(runtime,uid,uid)
print('lab_database_provisioned credentials_printed=false')
PY
}

bind_lab_release() {
    python3 - <<'PY'
import hashlib,json,os,pathlib,pwd,subprocess,time,uuid
root=pathlib.Path('/usr/local/libexec/duskweave')
base=pathlib.Path('/etc/duskweave/m1')
assert (base/'.live-dns-owner').read_text().strip()=='DW-IMPLEMENT-M1-LIVE-DNS-GENAI'
manifest={name+'_sha256':hashlib.sha256((root/binary).read_bytes()).hexdigest()
          for name,binary in [('release','duskweave'),('launcher','dw-boundary-launcher'),
                              ('capture','dw-dns-capture'),('exporter','dw-dns-exporter')]}
image=subprocess.check_output(['docker','image','inspect','--format','{{.Id}}',
                               'dw-live-dns-20261030:candidate'],text=True).strip()
assert image.startswith('sha256:') and len(image)==71
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
                  '-d','dw_native_live_dns_20261031b','-v','ON_ERROR_STOP=1','-At'],
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
report=pathlib.Path('/srv/duskweave/reports/live-dns-20261030/completion-20261031a')
report.mkdir(mode=0o700,exist_ok=True)
m=report.lstat()
assert m.st_uid==0 and m.st_mode&0o777==0o700 and not report.is_symlink()
(report/('deployment-'+manifest['qualification_ref']+'.json')).write_bytes(encoded)
print('lab_release_bound purpose=LAB qualification='+manifest['qualification_ref'])
PY
}

case "$1" in
    service) [[ $# == 1 ]] || refuse qualification_arguments_refused; prepare_service ;;
    release) [[ $# == 1 ]] || refuse qualification_arguments_refused; build_release; install_release ;;
    lab-database) [[ $# == 1 ]] || refuse qualification_arguments_refused; prepare_lab_database ;;
    lab-bind) [[ $# == 1 ]] || refuse qualification_arguments_refused; bind_lab_release ;;
    build-storage) [[ $# == 1 ]] || refuse qualification_arguments_refused; check_build_storage ;;
    *) refuse qualification_arguments_refused ;;
esac
