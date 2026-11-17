"""Fixed-scope host, release binary, OCI image, sudoers, and quiescence observations."""

from __future__ import annotations

import grp, hashlib, io, json, os, pwd, re, stat, subprocess, tarfile, uuid
from pathlib import Path, PurePosixPath
from candidate_files import FileCustodyError, read_protected_bytes, require_safe_ancestors
from candidate_postgres import PostgresError, run_psql_stdin

_RPT = "/srv/duskweave/reports/live-context-purpose-r1"
_CFG = "/etc/duskweave/m1/linux-prov2-qual-r1"
PRIVILEGED_SCOPE = {
    "source_dir": "/srv/duskweave/src/linux-prov2-qual-r1", "build_root": "/srv/duskweave-build/linux-prov2-qual-r1",
    "report_root": _RPT, "bundle_dir": f"{_RPT}/bundle", "backup_dir": f"{_RPT}/install-backup",
    "binding_intent_dir": f"{_RPT}/binding", "config_dir": _CFG, "owner_custody_path": f"{_CFG}/owner-custody.json",
    "core_dsn_path": f"{_CFG}/core.dsn", "env_json_path": f"{_CFG}/environment.json",
    "manifest_path": "/etc/duskweave/m1/deployment.json", "install_bin_dir": "/usr/local/libexec/duskweave",
    "database": "dw_native_linux_prov2_qual_20261115r1", "pg_container": "dw-live-dns-20261030-pg",
    "role_prefix": "dw_lpq15r1", "core_uid_path": "/srv/duskweave/.core-account-uid",
    "sudoers_path": "/etc/sudoers.d/duskweave-m1-launcher", "control_root": "/run/duskweave/m1",
    "launcher_root": "/run/duskweave/launcher", "netns_dir": "/run/netns",
    "live_dns_owner_path": "/etc/duskweave/m1/.live-dns-owner",
}
RETAINED_QUIESCENCE_DBS = (
    "dw_native_final_release_qual_20261110b", "dw_native_diag_release_20261111r1",
    "dw_native_provider_confirm_20261111r1", "dw_native_prop_diag_green2suite_20261111r1",
)
INSTALL_BINARIES = ("duskweave", "dw-boundary-launcher", "dw-dns-capture", "dw-dns-exporter")
WORKER_BINARIES = ("dw-dns-capture", "dw-dns-exporter")
FORBIDDEN_CORE_GROUPS = frozenset({"root", "sudo", "docker", "adm", "lxd"})
EXPECTED_SUDOERS = 'duskweave-core ALL=(root) NOPASSWD: /usr/local/libexec/duskweave/dw-boundary-launcher ""\n'
EXPECTED_LIVE_DNS_OWNER = "DW-IMPLEMENT-M1-LIVE-DNS-GENAI"
MAX_WORKER_BYTES = 64 * 1024 * 1024
MAX_IMAGE_STREAM_BYTES = 96 * 1024 * 1024
HEX32_RE = re.compile(r"^[0-9a-f]{32}$")
QUIESCENCE_SQL = (
    "SELECT CASE WHEN to_regclass('execution.autonomous_control') IS NULL "
    "OR to_regclass('execution.live_effect_control') IS NULL THEN -1 ELSE "
    "(SELECT count(*) FROM execution.autonomous_control WHERE status = 'running') + "
    "(SELECT count(*) FROM execution.live_effect_control WHERE phase IN ('reserved', 'may_start', 'result_recorded')) END;"
)


class HostObservationError(RuntimeError):
    """Fail-closed error carrying bounded non-secret status and reason."""

    def __init__(self, status: str, reason: str) -> None:
        super().__init__(reason)
        self.status, self.reason = status, reason


def _read_root_text(path: Path, allowed_modes: tuple[int, ...] | None, max_bytes: int,
                    stop_at: Path, expected_uids: tuple[int, ...] = (0,)) -> str:
    try:
        return read_protected_bytes(path, expected_uids=expected_uids, allowed_modes=allowed_modes,
                                    max_bytes=max_bytes, stop_at=stop_at, status_on_error="REFUSED").decode("utf-8")
    except FileCustodyError as exc:
        raise HostObservationError(exc.status, exc.reason) from exc
    except UnicodeDecodeError as exc:
        raise HostObservationError("REFUSED", f"non_utf8_host_file:{path.name}") from exc


def resolve_core_account(*, core_uid_path: Path | None = None, stop_at: Path = Path("/"),
                         expected_uids: tuple[int, ...] = (0,), getpwuid_fn=pwd.getpwuid,
                         getpwnam_fn=pwd.getpwnam, getgrgid_fn=grp.getgrgid,
                         getgrall_fn=grp.getgrall) -> dict[str, int | str]:
    """Read protected .core-account-uid and verify matching unprivileged account."""
    uid_file = core_uid_path or Path(PRIVILEGED_SCOPE["core_uid_path"])
    text = _read_root_text(uid_file, (0o400,), 64, stop_at, expected_uids).strip()
    if not text.isdigit() or int(text) <= 0:
        raise HostObservationError("REFUSED", "core_account_uid_invalid")
    uid = int(text)
    try:
        pw_uid, pw_name = getpwuid_fn(uid), getpwnam_fn("duskweave-core")
        primary_group = getgrgid_fn(pw_uid.pw_gid).gr_name
    except KeyError as exc:
        raise HostObservationError("REFUSED", "core_account_missing") from exc
    if pw_uid.pw_name != "duskweave-core" or pw_name.pw_uid != uid or pw_uid.pw_gid <= 0:
        raise HostObservationError("REFUSED", "core_account_mismatch")
    groups = {g.gr_name for g in getgrall_fn() if "duskweave-core" in g.gr_mem} | {primary_group}
    if groups & FORBIDDEN_CORE_GROUPS:
        raise HostObservationError("REFUSED", "core_account_forbidden_group")
    return {"name": "duskweave-core", "uid": uid, "gid": pw_uid.pw_gid}


def verify_sudoers(*, sudoers_path: Path | None = None, stop_at: Path = Path("/"),
                   expected_uids: tuple[int, ...] = (0,), cmd_runner=subprocess.run) -> None:
    """Require exact established sudoers policy bytes plus visudo syntax validation."""
    target = sudoers_path or Path(PRIVILEGED_SCOPE["sudoers_path"])
    if _read_root_text(target, (0o400, 0o440), 4096, stop_at, expected_uids) != EXPECTED_SUDOERS:
        raise HostObservationError("REFUSED", "sudoers_policy_mismatch")
    try:
        proc = cmd_runner(["visudo", "-cf", str(target)], capture_output=True, text=True, timeout=15)
    except (OSError, subprocess.TimeoutExpired) as exc:
        raise HostObservationError("UNKNOWN", "visudo_execution_failed") from exc
    if proc.returncode != 0:
        raise HostObservationError("REFUSED", "sudoers_syntax_invalid")


def _check_dir_owner_mode(path: Path, expected_uid: int, expected_mode: int, stop_at: Path,
                          ancestor_uids: tuple[int, ...]) -> None:
    try:
        require_safe_ancestors(path.parent, ancestor_uids, stop_at=stop_at)
        st = os.lstat(path)
    except (OSError, FileCustodyError) as exc:
        raise HostObservationError("REFUSED", f"host_dir_unqualified:{path.name}") from exc
    if stat.S_ISLNK(st.st_mode) or not stat.S_ISDIR(st.st_mode):
        raise HostObservationError("REFUSED", f"host_dir_type_unsafe:{path.name}")
    if st.st_uid != expected_uid or stat.S_IMODE(st.st_mode) != expected_mode:
        raise HostObservationError("REFUSED", f"host_dir_owner_or_mode:{path.name}")


def observe_host_sinks(core_uid: int, *, proc_root: Path = Path("/proc"),
                       live_dns_owner_path: Path | None = None,
                       control_root: Path | None = None, launcher_root: Path | None = None,
                       stop_at: Path = Path("/"), expected_uids: tuple[int, ...] = (0,)) -> str:
    """Observe boot_id, swap absence, core/dumpability sinks, live-dns-owner, and runtime dirs."""
    boot_raw = _read_root_text(proc_root / "sys/kernel/random/boot_id", None, 64, stop_at, expected_uids).strip()
    try:
        boot_id = str(uuid.UUID(boot_raw))
    except ValueError as exc:
        raise HostObservationError("REFUSED", "invalid_host_boot_id") from exc
    swaps = _read_root_text(proc_root / "swaps", None, 65536, stop_at, expected_uids).splitlines()
    if len([ln for ln in swaps if ln.strip()]) > 1:
        raise HostObservationError("REFUSED", "active_swap_detected")
    core_pat = _read_root_text(proc_root / "sys/kernel/core_pattern", None, 256, stop_at, expected_uids).rstrip("\n")
    suid_dump = _read_root_text(proc_root / "sys/fs/suid_dumpable", None, 32, stop_at, expected_uids).strip()
    if core_pat != "|/bin/false" or suid_dump != "0":
        raise HostObservationError("REFUSED", "unsafe_host_core_sink")
    owner_file = live_dns_owner_path or Path(PRIVILEGED_SCOPE["live_dns_owner_path"])
    if _read_root_text(owner_file, (0o400,), 128, stop_at, expected_uids).rstrip("\n") != EXPECTED_LIVE_DNS_OWNER:
        raise HostObservationError("REFUSED", "live_dns_owner_mismatch")
    _check_dir_owner_mode(control_root or Path(PRIVILEGED_SCOPE["control_root"]), core_uid, 0o700, stop_at, expected_uids)
    lnch = launcher_root or Path(PRIVILEGED_SCOPE["launcher_root"])
    if lnch.exists() or lnch.is_symlink():
        _check_dir_owner_mode(lnch, expected_uids[0], 0o700, stop_at, expected_uids)
    return boot_id


def _read_installed_member(root: Path, name: str, stop_at: Path, expected_uids: tuple[int, ...],
                           require_all: bool) -> dict | None:
    path = root / name
    if not (path.exists() or path.is_symlink()):
        if require_all:
            raise HostObservationError("REFUSED", f"installed_binary_missing:{name}")
        return None
    try:
        raw = read_protected_bytes(path, expected_uids=expected_uids, allowed_modes=(0o555, 0o755),
                                   stop_at=stop_at, status_on_error="REFUSED")
    except FileCustodyError as exc:
        raise HostObservationError(exc.status, exc.reason) from exc
    st = os.lstat(path)
    return {"sha256": hashlib.sha256(raw).hexdigest(), "size": len(raw),
            "mode": stat.S_IMODE(st.st_mode), "uid": st.st_uid, "gid": st.st_gid, "bytes": raw}


def observe_installed_binaries(install_bin_dir: Path | None = None, *, stop_at: Path = Path("/"),
                             expected_uids: tuple[int, ...] = (0,), require_all: bool = True) -> dict[str, dict]:
    """Read and hash installed binaries with custody checks; optionally tolerate all absent."""
    root = install_bin_dir or Path(PRIVILEGED_SCOPE["install_bin_dir"])
    try:
        require_safe_ancestors(root, expected_uids, stop_at=stop_at)
    except FileCustodyError as exc:
        raise HostObservationError(exc.status, exc.reason) from exc
    observed: dict[str, dict] = {}
    for name in INSTALL_BINARIES:
        info = _read_installed_member(root, name, stop_at, expected_uids, require_all)
        if info is not None:
            observed[name] = info
    return observed


def observe_installed_facts(*, install_bin_dir: Path | None = None, core_uid_path: Path | None = None,
                            proc_root: Path = Path("/proc"), live_dns_owner_path: Path | None = None,
                            control_root: Path | None = None, launcher_root: Path | None = None,
                            sudoers_path: Path | None = None, stop_at: Path = Path("/"),
                            expected_uids: tuple[int, ...] = (0,), require_binaries: bool = True,
                            cmd_runner=subprocess.run, pwd_resolver=pwd.getpwuid,
                            pwnam_resolver=pwd.getpwnam, grgid_resolver=grp.getgrgid,
                            grall_resolver=grp.getgrall) -> dict:
    """Independently observe host account, sinks, sudoers, and installed binaries."""
    core = resolve_core_account(core_uid_path=core_uid_path, stop_at=stop_at, expected_uids=expected_uids,
                                getpwuid_fn=pwd_resolver, getpwnam_fn=pwnam_resolver,
                                getgrgid_fn=grgid_resolver, getgrall_fn=grall_resolver)
    boot_id = observe_host_sinks(int(core["uid"]), proc_root=proc_root, live_dns_owner_path=live_dns_owner_path,
                                 control_root=control_root, launcher_root=launcher_root,
                                 stop_at=stop_at, expected_uids=expected_uids)
    verify_sudoers(sudoers_path=sudoers_path, stop_at=stop_at, expected_uids=expected_uids, cmd_runner=cmd_runner)
    binaries = observe_installed_binaries(install_bin_dir=install_bin_dir, stop_at=stop_at,
                                          expected_uids=expected_uids, require_all=require_binaries)
    return {"core_uid": int(core["uid"]), "core_gid": int(core["gid"]), "boot_id": boot_id,
            "installed_hashes": {k: v["sha256"] for k, v in binaries.items()}, "binaries": binaries}


def _inspect_image_meta(image_ref: str, docker_runner) -> tuple[str, int]:
    try:
        proc = docker_runner(
            ["docker", "image", "inspect", "--format", "{{json .}}", image_ref],
            capture_output=True, text=True, timeout=30,
        )
    except (OSError, subprocess.TimeoutExpired) as exc:
        raise HostObservationError("UNKNOWN", "docker_image_inspect_failed") from exc
    if proc.returncode != 0:
        raise HostObservationError("REFUSED", "image_inspect_nonzero")
    return _parse_image_meta(proc.stdout)


def _parse_image_meta(raw: str) -> tuple[str, int]:
    try:
        doc = json.loads(raw)
    except json.JSONDecodeError as exc:
        raise HostObservationError("UNKNOWN", "image_inspect_malformed") from exc
    if not isinstance(doc, dict) or doc.get("Os") != "linux" or doc.get("Architecture") != "arm64":
        raise HostObservationError("REFUSED", "image_platform_mismatch")
    image_id, rootfs = doc.get("Id"), doc.get("RootFS") or {}
    layers = rootfs.get("Layers") if isinstance(rootfs, dict) else None
    if not isinstance(image_id, str) or rootfs.get("Type") != "layers" or not layers:
        raise HostObservationError("REFUSED", "image_rootfs_invalid")
    return image_id, len(layers)


def _normalize_tar_member(name: str) -> str:
    clean = name.removeprefix("./")
    parts = clean.split("/")
    if not clean or "\\" in clean or any(p in ("", ".", "..") or p.startswith(".wh.") for p in parts):
        raise HostObservationError("REFUSED", "image_layer_unsafe_member")
    return clean


def _extract_layer_regular_member(ltf: tarfile.TarFile, member: tarfile.TarInfo,
                                  found: dict[str, str], total_ref: list[int]) -> None:
    norm = _normalize_tar_member(member.name)
    if norm not in WORKER_BINARIES or norm in found:
        raise HostObservationError("REFUSED", f"image_unexpected_worker_member:{norm}")
    if member.size <= 0 or member.size > MAX_WORKER_BYTES:
        raise HostObservationError("REFUSED", f"image_worker_size_invalid:{norm}")
    total_ref[0] += member.size
    if total_ref[0] > MAX_IMAGE_STREAM_BYTES:
        raise HostObservationError("REFUSED", "image_extracted_size_exceeded")
    extracted = ltf.extractfile(member)
    raw = extracted.read() if extracted is not None else b""
    if len(raw) != member.size:
        raise HostObservationError("REFUSED", f"image_worker_truncated:{norm}")
    found[norm] = hashlib.sha256(raw).hexdigest()


def _validate_layer_member(ltf: tarfile.TarFile, member: tarfile.TarInfo,
                           found: dict[str, str], total_ref: list[int]) -> None:
    if member.issym() or member.islnk() or member.isdev() or member.isfifo():
        raise HostObservationError("REFUSED", "image_layer_link_or_device_refused")
    if member.isdir():
        if any(p.startswith(".wh.") or p == ".." for p in PurePosixPath(member.name).parts):
            raise HostObservationError("REFUSED", "image_layer_unsafe_member")
        return
    if not member.isreg():
        raise HostObservationError("REFUSED", "image_layer_nonregular_member")
    _extract_layer_regular_member(ltf, member, found, total_ref)


def _scan_layer_tar(layer_bytes: bytes, found: dict[str, str], total_ref: list[int]) -> None:
    try:
        with tarfile.open(fileobj=io.BytesIO(layer_bytes), mode="r:*") as ltf:
            for member in ltf.getmembers():
                _validate_layer_member(ltf, member, found, total_ref)
    except tarfile.TarError as exc:
        raise HostObservationError("UNKNOWN", "image_layer_tar_corrupt") from exc


def _extract_workers_from_save(raw_tar: bytes, expected_layers: int) -> dict[str, str]:
    found: dict[str, str] = {}
    total_ref = [0]
    try:
        with tarfile.open(fileobj=io.BytesIO(raw_tar), mode="r:*") as tf:
            mfile = tf.extractfile("manifest.json")
            manifest = json.loads(mfile.read().decode("utf-8")) if mfile is not None else None
            if not isinstance(manifest, list) or len(manifest) != 1:
                raise HostObservationError("REFUSED", "image_manifest_invalid")
            layers = manifest[0].get("Layers")
            if not isinstance(layers, list) or len(layers) != expected_layers:
                raise HostObservationError("REFUSED", "image_layer_count_mismatch")
            for layer_path in layers:
                lfile = tf.extractfile(layer_path)
                if lfile is None:
                    raise HostObservationError("REFUSED", "image_layer_missing")
                _scan_layer_tar(lfile.read(), found, total_ref)
    except (tarfile.TarError, UnicodeDecodeError, json.JSONDecodeError, KeyError) as exc:
        raise HostObservationError("UNKNOWN", "image_archive_parse_failed") from exc
    return found


def extract_image_worker_digests(image_ref: str, *, docker_runner=subprocess.run) -> dict:
    """Inspect OCI image metadata and extract exact layer worker SHA-256 digests safely."""
    image_id, expected_layers = _inspect_image_meta(image_ref, docker_runner)
    try:
        save_proc = docker_runner(["docker", "image", "save", image_ref], capture_output=True, text=False, timeout=60)
    except (OSError, subprocess.TimeoutExpired) as exc:
        raise HostObservationError("UNKNOWN", "docker_image_save_failed") from exc
    if save_proc.returncode != 0 or len(save_proc.stdout) > MAX_IMAGE_STREAM_BYTES:
        raise HostObservationError("REFUSED", "docker_image_save_rejected")
    found = _extract_workers_from_save(save_proc.stdout, expected_layers)
    if set(found) != set(WORKER_BINARIES):
        raise HostObservationError("REFUSED", "image_workers_incomplete")
    return {"image_id": image_id, "os": "linux", "architecture": "arm64", "workers": found}


def _check_procs(cmd_runner) -> None:
    cmd = ["pgrep", "-af", "/usr/local/libexec/duskweave/(duskweave|dw-boundary-launcher)"]
    try:
        pg = cmd_runner(cmd, capture_output=True, text=True, timeout=15)
    except (OSError, subprocess.TimeoutExpired) as exc:
        raise HostObservationError("UNKNOWN", "pgrep_observation_failed") from exc
    if pg.returncode == 0:
        raise HostObservationError("REFUSED", "active_runtime_processes_present")
    if pg.returncode != 1:
        raise HostObservationError("UNKNOWN", "pgrep_observation_error")


def _check_containers(cmd_runner) -> None:
    try:
        dk = cmd_runner(["docker", "ps", "-a", "--format", "{{.Names}}"], capture_output=True, text=True, timeout=15)
    except (OSError, subprocess.TimeoutExpired) as exc:
        raise HostObservationError("UNKNOWN", "docker_ps_observation_failed") from exc
    if dk.returncode != 0:
        raise HostObservationError("UNKNOWN", "docker_ps_error")
    for line in dk.stdout.splitlines():
        if line.strip().startswith(("dwcap", "dwexp")):
            raise HostObservationError("REFUSED", f"active_worker_container:{line.strip()}")


def _check_netns(netns_dir: Path) -> None:
    if not (netns_dir.exists() or netns_dir.is_symlink()):
        return
    if netns_dir.is_symlink() or not netns_dir.is_dir():
        raise HostObservationError("UNKNOWN", "netns_dir_unsafe")
    for entry in os.listdir(netns_dir):
        if entry.startswith("dw"):
            raise HostObservationError("REFUSED", f"active_worker_netns:{entry}")


def _scan_launcher_entries(launcher_root: Path) -> None:
    if not (launcher_root.exists() or launcher_root.is_symlink()):
        return
    if launcher_root.is_symlink() or not launcher_root.is_dir():
        raise HostObservationError("UNKNOWN", "launcher_dir_unsafe")
    names = set(os.listdir(launcher_root))
    for name in sorted(names):
        if name.endswith(".pending") and f"{name[:-8]}.complete" not in names:
            raise HostObservationError("REFUSED", f"pending_launcher_marker:{name}")
        if HEX32_RE.fullmatch(name) and (launcher_root / name / "session.json").exists():
            raise HostObservationError("REFUSED", f"active_launcher_session:{name}")


def _exec_container_psql(container: str, db: str, query: str, cmd_runner) -> str:
    cmd = ["docker", "exec", "-i", container, "psql", "-X", "-U", "postgres", "-d", db, "-v", "ON_ERROR_STOP=1", "-Atc", query]
    try:
        proc = cmd_runner(cmd, capture_output=True, text=True, timeout=30)
    except (OSError, subprocess.TimeoutExpired) as exc:
        raise HostObservationError("UNKNOWN", f"quiescence_db_query_failed:{db}") from exc
    if proc.returncode != 0:
        raise HostObservationError("UNKNOWN", f"quiescence_db_query_failed:{db}")
    return proc.stdout.strip()


def _query_quiescence_db(container: str, db: str, admin_dsn: str | None, candidate_db: str | None,
                         cmd_runner, sql_runner) -> str:
    try:
        if sql_runner is not None:
            return sql_runner(db, QUIESCENCE_SQL).strip()
        if db == candidate_db and admin_dsn:
            return run_psql_stdin(container, admin_dsn, db, QUIESCENCE_SQL).strip()
        return _exec_container_psql(container, db, QUIESCENCE_SQL, cmd_runner)
    except PostgresError as exc:
        raise HostObservationError("UNKNOWN", f"quiescence_db_query_failed:{db}") from exc


def _check_quiescence_dbs(container: str, admin_dsn: str | None, candidate_db: str | None,
                          cmd_runner, sql_runner) -> list[str]:
    dbs = list(RETAINED_QUIESCENCE_DBS) + ([candidate_db] if candidate_db else [])
    for db in dbs:
        out = _query_quiescence_db(container, db, admin_dsn, candidate_db, cmd_runner, sql_runner)
        if out == "-1" or not out.isdigit():
            raise HostObservationError("UNKNOWN", f"quiescence_db_catalog_missing:{db}")
        if int(out) > 0:
            raise HostObservationError("REFUSED", f"active_db_runs_present:{db}")
    return dbs


def check_quiescence(*, pg_container: str | None = None, admin_dsn: str | None = None, candidate_db: str | None = None,
                     netns_dir: Path | None = None, launcher_root: Path | None = None,
                     cmd_runner=subprocess.run, sql_runner=None) -> dict:
    """Reconcile process, container, netns, launcher journal, and retained/candidate DB activity."""
    _check_procs(cmd_runner)
    _check_containers(cmd_runner)
    try:
        _check_netns(netns_dir or Path(PRIVILEGED_SCOPE["netns_dir"]))
        _scan_launcher_entries(launcher_root or Path(PRIVILEGED_SCOPE["launcher_root"]))
    except OSError as exc:
        raise HostObservationError("UNKNOWN", "runtime_dir_observation_failed") from exc
    dbs = _check_quiescence_dbs(pg_container or PRIVILEGED_SCOPE["pg_container"], admin_dsn, candidate_db, cmd_runner, sql_runner)
    return {"quiescent": True, "checked_dbs": dbs}
