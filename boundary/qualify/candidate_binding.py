#!/usr/bin/env python3
"""Candidate LIVE qualification manifest rendering, phase-ordered publication, and read-only reconcile."""

from __future__ import annotations

import argparse, hashlib, ipaddress, json, os, re, time, uuid
from pathlib import Path

from candidate_files import FileCustodyError, create_exclusive_dir, publish_atomic_verified, read_protected_bytes, reject_sensitive_env, verify_driver_custody, write_exclusive_verified
from candidate_host import HostObservationError, INSTALL_BINARIES, PRIVILEGED_SCOPE, WORKER_BINARIES, check_quiescence, extract_image_worker_digests, observe_installed_facts
from candidate_postgres import PostgresError, run_psql_stdin, verify_release_schema_and_identities
import release_descriptor

PURPOSE_LIVE, LIVE_RUN_PROFILE = "LIVE", "autonomous-dns-genai-live-v1"
PRO_V2_INFERENCE_PROFILE, MAX_MANIFEST_BYTES, INT64_MAX = "deepseek-genai-v4-pro-low-v2", 16384, (1 << 63) - 1
HEX40_RE, HEX64_RE = re.compile(r"^[0-9a-f]{40}$"), re.compile(r"^[0-9a-f]{64}$")
IMAGE_DIGEST_RE, CANONICAL_IPV4_RE = re.compile(r"^sha256:[0-9a-f]{64}$"), re.compile(r"^(?:0|[1-9][0-9]{0,2})(?:\.(?:0|[1-9][0-9]{0,2})){3}$")
SPECIAL_IPV4_NETS = tuple(ipaddress.IPv4Network(c) for c in (
    "0.0.0.0/8", "10.0.0.0/8", "100.64.0.0/10", "127.0.0.0/8", "169.254.0.0/16", "172.16.0.0/12",
    "192.0.0.0/24", "192.0.2.0/24", "192.88.99.0/24", "192.168.0.0/16", "198.18.0.0/15",
    "198.51.100.0/24", "203.0.113.0/24", "224.0.0.0/4", "240.0.0.0/4"))
IDENTITY_HASH_FIELDS = ("release_sha256", "launcher_sha256", "capture_sha256", "exporter_sha256")
BINARY_TO_FIELD = dict(zip(INSTALL_BINARIES, IDENTITY_HASH_FIELDS))
SQL_COLUMNS = ("qualification_ref", "run_profile", "purpose", *IDENTITY_HASH_FIELDS, "image_digest", "manifest_sha256", "boot_id", "resolver_ipv4", "valid_from_ms", "valid_until_ms")
MANIFEST_KEYS = (frozenset(SQL_COLUMNS) - {"manifest_sha256"}) | {"core_uid"}
DEFAULT_MIGRATION = Path(__file__).resolve().parents[2] / "migrations" / "0035_pro_budget_c_v2.sql"
class BindingError(RuntimeError): """Fail-closed pre-effect LIVE qualification refusal."""
_TRUSTED_ERRS = (BindingError, FileCustodyError, HostObservationError, PostgresError, release_descriptor.ReleaseError)
_EXPECTED_EXCS = (*_TRUSTED_ERRS, OSError, TimeoutError, ValueError, RuntimeError, json.JSONDecodeError, UnicodeDecodeError)
def _utc_now_ms() -> int: return int(time.time() * 1000)

def _safe_err(exc: Exception, default_reason: str) -> tuple[str, str]:
    status = "UNKNOWN" if isinstance(exc, TimeoutError) else getattr(exc, "status", None)
    status = status if status in ("REFUSED", "INCOMPLETE", "UNKNOWN") else "INCOMPLETE"
    raw = (getattr(exc, "reason", None) or (str(exc) if isinstance(exc, BindingError) else "")) if isinstance(exc, _TRUSTED_ERRS) else ""
    if raw and len(raw) <= 96 and "://" not in raw and not any(c.isspace() for c in raw):
        return status, raw
    return status, f"{default_reason}:{type(exc).__name__}"

def validate_global_ipv4(value: str) -> str:
    """Require canonical global IPv4; refuse RFC1918, loopback, 6to4 relay, TEST-NET, CGNAT, etc."""
    if not isinstance(value, str) or not CANONICAL_IPV4_RE.fullmatch(value):
        raise BindingError("resolver_ipv4_malformed")
    try:
        addr = ipaddress.IPv4Address(value)
    except ValueError as exc:
        raise BindingError("resolver_ipv4_invalid") from exc
    if str(addr) != value or any(addr in net for net in SPECIAL_IPV4_NETS):
        raise BindingError("resolver_ipv4_not_global")
    return value

def _validate_uuid_non_nil(raw: object, field: str) -> str:
    if not isinstance(raw, str):
        raise BindingError(f"uuid_not_string:{field}")
    try:
        parsed = uuid.UUID(raw)
    except ValueError as exc:
        raise BindingError(f"uuid_invalid:{field}") from exc
    if parsed.int == 0 or str(parsed) != raw.lower():
        raise BindingError(f"uuid_non_canonical_or_nil:{field}")
    return str(parsed)

def _validate_identities(identities: dict) -> dict:
    if not isinstance(identities, dict):
        raise BindingError("identities_malformed")
    for field in IDENTITY_HASH_FIELDS:
        if not isinstance(identities.get(field), str) or not HEX64_RE.fullmatch(identities[field]):
            raise BindingError(f"identity_hash_invalid:{field}")
    if not isinstance(identities.get("image_digest"), str) or not IMAGE_DIGEST_RE.fullmatch(identities["image_digest"]):
        raise BindingError("identity_image_digest_invalid")
    core_uid = identities.get("core_uid")
    if type(core_uid) is not int or core_uid <= 0 or core_uid > 0xFFFFFFFF:
        raise BindingError("identity_core_uid_invalid")
    return {**identities, "boot_id": _validate_uuid_non_nil(identities.get("boot_id"), "boot_id")}

def _validate_bounds(v_from: object, v_until: object) -> tuple[int, int]:
    if type(v_from) is not int or type(v_until) is not int:
        raise BindingError("validity_window_not_integer")
    if v_from <= 0 or v_until <= v_from or v_until > INT64_MAX:
        raise BindingError("validity_window_bounds_invalid")
    return v_from, v_until

def _validate_window(v_from: object, v_until: object, now_ms: object) -> tuple[int, int]:
    vf, vu = _validate_bounds(v_from, v_until)
    if type(now_ms) is not int or now_ms <= 0 or now_ms > INT64_MAX:
        raise BindingError("validity_window_not_integer")
    if not (vf <= now_ms < vu):
        raise BindingError("validity_window_expired_or_future")
    return vf, vu

def _validate_manifest_semantics(manifest: object, *, expected_qref: str | None = None, now_ms: object = None) -> dict:
    if type(manifest) is not dict:
        raise BindingError("manifest_not_object")
    keys = set(manifest.keys())
    if MANIFEST_KEYS - keys:
        raise BindingError("manifest_missing_fields")
    if keys - MANIFEST_KEYS:
        raise BindingError("manifest_unknown_fields")
    qref = _validate_uuid_non_nil(manifest["qualification_ref"], "qualification_ref")
    if expected_qref is not None and qref != expected_qref:
        raise BindingError("manifest_qref_mismatch")
    if manifest["purpose"] != PURPOSE_LIVE or manifest["run_profile"] != LIVE_RUN_PROFILE:
        raise BindingError("live_purpose_or_profile_required")
    validate_global_ipv4(manifest["resolver_ipv4"])
    _validate_identities(manifest)
    _validate_bounds(manifest["valid_from_ms"], manifest["valid_until_ms"]) if now_ms is None else _validate_window(manifest["valid_from_ms"], manifest["valid_until_ms"], now_ms)
    return manifest

def _no_dup_pairs(pairs: list[tuple[str, object]]) -> dict[str, object]:
    out: dict[str, object] = {}
    for k, v in pairs:
        if k in out:
            raise BindingError("manifest_duplicate_key")
        out[k] = v
    return out

def _parse_manifest_bytes(raw: bytes) -> dict:
    if len(raw) > MAX_MANIFEST_BYTES:
        raise BindingError("manifest_exceeds_byte_limit")
    return json.loads(raw.decode("utf-8"), object_pairs_hook=_no_dup_pairs, parse_constant=lambda _t: (_ for _ in ()).throw(BindingError("manifest_nonfinite_number")))

def build_manifest_bundle(*, identities: dict, resolver_ipv4: str, valid_from_ms: int, valid_until_ms: int,
                          now_ms: int | None = None, qualification_ref: str | None = None,
                          purpose: str = PURPOSE_LIVE, run_profile: str = LIVE_RUN_PROFILE) -> tuple[dict, bytes, str, dict]:
    """Build exact manifest JSON, serialized bytes, SHA-256, and derived SQL row."""
    if not isinstance(identities, dict):
        raise BindingError("identities_malformed")
    qref = str(uuid.uuid4()) if qualification_ref is None else qualification_ref
    cand = {"qualification_ref": qref, "purpose": purpose, "run_profile": run_profile, "resolver_ipv4": resolver_ipv4,
            "valid_from_ms": valid_from_ms, "valid_until_ms": valid_until_ms, **{k: identities.get(k) for k in (*IDENTITY_HASH_FIELDS, "image_digest", "boot_id", "core_uid")}}
    manifest = _validate_manifest_semantics(cand, now_ms=_utc_now_ms() if now_ms is None else now_ms)
    encoded = (json.dumps(manifest, sort_keys=True, separators=(",", ":")) + "\n").encode("utf-8")
    if len(encoded) > MAX_MANIFEST_BYTES:
        raise BindingError("manifest_exceeds_byte_limit")
    m_sha = hashlib.sha256(encoded).hexdigest()
    return manifest, encoded, m_sha, {**{c: manifest[c] for c in SQL_COLUMNS if c != "manifest_sha256"}, "manifest_sha256": m_sha}

def render_binding(*, identities: dict, resolver_ipv4: str, valid_from_ms: int, valid_until_ms: int,
                   now_ms: int | None = None, qualification_ref: str | None = None, purpose: str = PURPOSE_LIVE,
                   run_profile: str = LIVE_RUN_PROFILE, output_file: Path | None = None) -> dict:
    """Render UNAPPLIED LIVE manifest bytes and derived SQL columns offline."""
    manifest, encoded, m_sha, sql_row = build_manifest_bundle(
        identities=identities, resolver_ipv4=resolver_ipv4, valid_from_ms=valid_from_ms, valid_until_ms=valid_until_ms,
        now_ms=now_ms, qualification_ref=qualification_ref, purpose=purpose, run_profile=run_profile)
    if output_file is not None:
        out_path = Path(output_file)
        try:
            write_exclusive_verified(out_path, encoded, mode=0o444, expected_uids=(0, os.geteuid()), stop_at=(Path("/") if os.geteuid() == 0 else out_path.parent))
        except FileCustodyError as exc:
            raise BindingError(exc.reason) from exc
    return {"status": "UNAPPLIED", "applied": False, "qualification_ref": manifest["qualification_ref"],
            "manifest_sha256": m_sha, "inference_profile": PRO_V2_INFERENCE_PROFILE, "manifest": manifest, "sql_row": sql_row}

def _identities_match(left: object, right: object) -> bool:
    return isinstance(left, dict) and isinstance(right, dict) and all(left.get(k) == right.get(k) for k in (*IDENTITY_HASH_FIELDS, "image_digest", "boot_id", "core_uid"))

def _sql_row_matches(row: object, manifest: dict, m_sha: str) -> bool:
    return isinstance(row, dict) and all(row.get(k) == (m_sha if k == "manifest_sha256" else manifest[k]) for k in SQL_COLUMNS)

class _PhaseAbort(Exception):
    def __init__(self, receipt: dict): self.receipt = receipt

def _step(ctx: dict, phase: str, default_rsn: str, fn, *, consistent: bool = False, force_st: str | None = None):
    try:
        return fn()
    except _EXPECTED_EXCS as exc:
        st, rsn = _safe_err(exc, default_rsn)
        if phase == "final_clock_check" and rsn.startswith("validity_window_"):
            rsn = "window_expired_or_clock_rollback"
        raise _PhaseAbort(_fail_receipt(ctx, force_st or st, phase, rsn, consistent)) from exc

def _step_rec(qref: str, default_rsn: str, fn, *, m_sha: str = "", consistent: bool = False, force_st: str | None = None):
    try:
        return fn()
    except _EXPECTED_EXCS as exc:
        st, rsn = _safe_err(exc, default_rsn)
        if default_rsn == "final_clock_failed" and rsn.startswith("validity_window_"):
            rsn = "binding_window_expired_or_future"
        raise _PhaseAbort(_rec_out(force_st or st, qref, rsn, m_sha, consistent=consistent)) from exc

def _persist_receipt(ctx: dict, receipt: dict, replacer) -> dict:
    idir = ctx["intent_dir"]
    if not (idir.exists() and not idir.is_symlink()):
        return receipt
    try:
        payload = (json.dumps(receipt, sort_keys=True, indent=2) + "\n").encode("utf-8")
        publish_atomic_verified(idir / f"receipt-{ctx['qref']}.json", payload, mode=0o444, expected_uids=ctx["uids"], stop_at=ctx["stop_at"], replacer=replacer)
        return receipt
    except _EXPECTED_EXCS as exc:
        _, err_r = _safe_err(exc, "receipt_persist_failed")
        err_part = err_r if err_r.startswith("receipt_persist_failed") else f"receipt_persist_failed:{err_r}"
        reason = err_part if not receipt.get("reason") else f"{receipt['reason']}|{err_part}"
        status = "UNKNOWN" if receipt["status"] == "SUCCESS" else receipt["status"]
        return {**receipt, "status": status, "current_qualified": False, "last_phase": receipt.get("last_phase") or "receipt_persist", "reason": reason}

def _preserve_intent_and_backup(ctx: dict) -> None:
    idir, mp, qref = ctx["intent_dir"], ctx["manifest_path"], ctx["qref"]
    if not (idir.exists() or idir.is_symlink()):
        create_exclusive_dir(idir, mode=0o700, expected_uids=ctx["uids"], stop_at=ctx["stop_at"])
    if mp.exists() or mp.is_symlink():
        old_bytes = read_protected_bytes(mp, expected_uids=ctx["uids"], allowed_modes=(0o400, 0o444), max_bytes=MAX_MANIFEST_BYTES, stop_at=ctx["stop_at"], status_on_error="REFUSED")
        write_exclusive_verified(idir / f"prior-deployment-{qref}.json", old_bytes, mode=0o400, expected_uids=ctx["uids"], stop_at=ctx["stop_at"])
    intent_obj = {"qualification_ref": qref, "manifest_sha256": ctx["m_sha"], "descriptor_sha256": ctx["d_sha"], "scope": ctx["scope"], "phase": "intent_persisted"}
    write_exclusive_verified(idir / f"intent-{qref}.json", (json.dumps(intent_obj, sort_keys=True) + "\n").encode("utf-8"), mode=0o400, expected_uids=ctx["uids"], stop_at=ctx["stop_at"])

def _fail_receipt(ctx: dict, status: str, phase: str, reason: str, consistent: bool = False) -> dict:
    return {"status": status, "qualification_ref": ctx["qref"], "manifest_sha256": ctx["m_sha"], "descriptor_sha256": ctx["d_sha"], "scope": ctx["scope"], "last_phase": phase, "row_manifest_consistent": consistent, "current_qualified": False, "reason": reason}

def _step_publish_manifest(ctx: dict, encoded: bytes, sql_executor, quiescence_checker, replacer) -> None:
    if _step(ctx, "sql_precheck", "sql_precheck_failed", lambda: sql_executor("exists", ctx["qref"])):
        raise _PhaseAbort(_fail_receipt(ctx, "REFUSED", "sql_precheck", "qualification_ref_collision"))
    if quiescence_checker is not None:
        _step(ctx, "quiescence_recheck", "quiescence_recheck_failed", quiescence_checker)
    _step(ctx, "manifest_publish", "manifest_publish_failed", lambda: publish_atomic_verified(ctx["manifest_path"], encoded, mode=0o444, expected_uids=ctx["uids"], stop_at=ctx["stop_at"], replacer=replacer), force_st="INCOMPLETE")

def _check_end_clock(manifest: dict, clock_fn, now_start: int) -> None:
    now_end = clock_fn()
    if type(now_end) is not int or now_end < now_start:
        raise BindingError("window_expired_or_clock_rollback")
    _validate_window(manifest["valid_from_ms"], manifest["valid_until_ms"], now_end)

def _final_recheck(ctx: dict, manifest: dict, encoded: bytes, clock_fn, now_start: int, host_reobserver) -> dict:
    disk = _step(ctx, "final_manifest_read", "final_manifest_read_failed", lambda: read_protected_bytes(ctx["manifest_path"], expected_uids=ctx["uids"], allowed_modes=(0o400, 0o444), max_bytes=MAX_MANIFEST_BYTES, stop_at=ctx["stop_at"], status_on_error="INCOMPLETE"), force_st="INCOMPLETE")
    if disk != encoded:
        raise _PhaseAbort(_fail_receipt(ctx, "INCOMPLETE", "final_manifest_read", "manifest_changed_after_commit"))
    _step(ctx, "final_clock_check", "final_clock_failed", lambda: _check_end_clock(manifest, clock_fn, now_start), consistent=True)
    if host_reobserver is not None:
        obs = _step(ctx, "final_host_reobserve", "final_host_reobserve_failed", host_reobserver, consistent=True)
        if not _identities_match(manifest, obs):
            raise _PhaseAbort(_fail_receipt(ctx, "INCOMPLETE", "final_host_reobserve", "installed_identity_changed", True))
    return {"status": "SUCCESS", "qualification_ref": ctx["qref"], "manifest_sha256": ctx["m_sha"], "descriptor_sha256": ctx["d_sha"], "scope": ctx["scope"], "inference_profile": PRO_V2_INFERENCE_PROFILE, "row_manifest_consistent": True, "current_qualified": True}

def _step_insert_and_readback(ctx: dict, sql_executor, sql_row: dict, manifest: dict, encoded: bytes, clock_fn, now_start: int, host_reobserver) -> dict:
    for phase, op, arg in (("sql_insert", "insert", sql_row), ("sql_readback", "read", ctx["qref"])):
        result = _step(ctx, phase, f"{phase}_uncertain", lambda o=op, a=arg: sql_executor(o, a), force_st="UNKNOWN")
    if not _sql_row_matches(result, manifest, ctx["m_sha"]):
        raise _PhaseAbort(_fail_receipt(ctx, "INCOMPLETE", "sql_readback", "sql_readback_mismatch"))
    return _final_recheck(ctx, manifest, encoded, clock_fn, now_start, host_reobserver)

def publish_binding(*, identities: dict, installed_identities: dict, resolver_ipv4: str, valid_from_ms: int, valid_until_ms: int, manifest_path: Path, intent_dir: Path, sql_executor, now_ms: int | None = None, qualification_ref: str | None = None, descriptor_sha256: str = "", scope: dict | None = None, env: dict[str, str] | None = None, expected_uids: tuple[int, ...] = (0,), stop_at: Path = Path("/"), clock_fn=_utc_now_ms, quiescence_checker=None, host_reobserver=None, replacer=os.replace) -> dict:
    """Publish LIVE manifest then insert ONE SQL row and persist bounded receipt across all failures."""
    reject_sensitive_env(env, error_cls=lambda _s, r: BindingError(r))
    admission_now = clock_fn() if now_ms is None else now_ms
    manifest, encoded, m_sha, sql_row = build_manifest_bundle(identities=identities, resolver_ipv4=resolver_ipv4, valid_from_ms=valid_from_ms, valid_until_ms=valid_until_ms, now_ms=admission_now, qualification_ref=qualification_ref)
    if not _identities_match(manifest, installed_identities):
        raise BindingError("installed_identity_mismatch")
    ctx = {"qref": manifest["qualification_ref"], "m_sha": m_sha, "d_sha": descriptor_sha256, "scope": scope or {"manifest_path": str(manifest_path), "intent_dir": str(intent_dir)}, "manifest_path": manifest_path, "intent_dir": intent_dir, "uids": expected_uids, "stop_at": stop_at}
    try:
        _step(ctx, "intent_and_backup", "intent_and_backup_failed", lambda: _preserve_intent_and_backup(ctx), force_st="INCOMPLETE")
        _step_publish_manifest(ctx, encoded, sql_executor, quiescence_checker, replacer)
        res = _step_insert_and_readback(ctx, sql_executor, sql_row, manifest, encoded, clock_fn, admission_now, host_reobserver)
    except _PhaseAbort as exc:
        res = exc.receipt
    return _persist_receipt(ctx, res, replacer)

def _rec_out(status: str, qref: str, reason: str, m_sha: str = "", consistent: bool = False) -> dict:
    return {"status": status, "qualification_ref": qref, "manifest_sha256": m_sha, "row_manifest_consistent": consistent, "current_qualified": False, "reason": reason}

def _evaluate_reconcile(qref: str, m_sha: str, manifest: dict, row: object, installed: object, clock_fn) -> dict:
    if row is None or not _sql_row_matches(row, manifest, m_sha):
        return _rec_out("INCOMPLETE", qref, "sql_binding_unissued" if row is None else "reconcile_mismatch", m_sha)
    if not _identities_match(manifest, installed):
        return _rec_out("INCOMPLETE", qref, "installed_identity_replaced", m_sha, consistent=True)
    _step_rec(qref, "final_clock_failed", lambda: _validate_window(manifest["valid_from_ms"], manifest["valid_until_ms"], clock_fn()), m_sha=m_sha, consistent=True)
    return {"status": "SUCCESS", "qualification_ref": qref, "manifest_sha256": m_sha, "inference_profile": PRO_V2_INFERENCE_PROFILE, "row_manifest_consistent": True, "current_qualified": True}

def reconcile_binding(*, qualification_ref: str, manifest_path: Path, installed_identities: dict, sql_reader, expected_uids: tuple[int, ...] = (0,), stop_at: Path = Path("/"), clock_fn=_utc_now_ms) -> dict:
    """Strictly read-only reconciliation separating historical row/file consistency from current readiness."""
    qref = _validate_uuid_non_nil(qualification_ref, "qualification_ref")
    if not (manifest_path.exists() or manifest_path.is_symlink()):
        return _rec_out("INCOMPLETE", qref, "manifest_missing")
    try:
        raw = _step_rec(qref, "manifest_read_failed", lambda: read_protected_bytes(manifest_path, expected_uids=expected_uids, allowed_modes=(0o400, 0o444), max_bytes=MAX_MANIFEST_BYTES, stop_at=stop_at, status_on_error="INCOMPLETE"))
        m_sha = hashlib.sha256(raw).hexdigest()
        manifest = _step_rec(qref, "manifest_invalid", lambda: _validate_manifest_semantics(_parse_manifest_bytes(raw), expected_qref=qref), m_sha=m_sha, force_st="INCOMPLETE")
        row = _step_rec(qref, "sql_read_failed", lambda: sql_reader(qref), m_sha=m_sha, force_st="UNKNOWN")
        return _evaluate_reconcile(qref, m_sha, manifest, row, installed_identities, clock_fn)
    except _PhaseAbort as exc:
        return exc.receipt

def _ids_from_facts(facts: dict, image_digest: str) -> dict:
    return {**{BINARY_TO_FIELD[b]: facts["installed_hashes"][b] for b in INSTALL_BINARIES}, "image_digest": image_digest, "boot_id": facts["boot_id"], "core_uid": facts["core_uid"]}

def _live_scope(kw: dict) -> dict:
    return {"uids": kw.get("expected_uids", (0,)), "stop": kw.get("stop_at", Path("/")), "cnt": kw.get("container") or PRIVILEGED_SCOPE["pg_container"], "dbn": kw.get("database") or PRIVILEGED_SCOPE["database"], "rpx": kw.get("role_prefix") or PRIVILEGED_SCOPE["role_prefix"], "mfst": kw.get("manifest_path") or Path(PRIVILEGED_SCOPE["manifest_path"]), "idir": kw.get("intent_dir") or Path(PRIVILEGED_SCOPE["binding_intent_dir"])}

def _check_pkg_match(desc: dict, img: dict, facts: dict, pkg_shas: dict[str, str]) -> str | None:
    if img.get("image_id") != desc["image_digest"] or any(img["workers"].get(w) != pkg_shas[w] for w in WORKER_BINARIES):
        return "image_worker_bytes_mismatch"
    for bname in INSTALL_BINARIES:
        if facts["installed_hashes"].get(bname) != pkg_shas[bname]:
            return f"installed_binary_package_mismatch:{bname}"
    return None

def _observe_live_context(kw: dict, *, require_match: bool) -> tuple[dict, str, dict, callable, dict]:
    sc = _live_scope(kw)
    if not (HEX40_RE.fullmatch(kw["expected_driver_commit"]) and HEX40_RE.fullmatch(kw["expected_driver_tree"])):
        raise BindingError("expected_driver_pin_invalid")
    h_obs = kw.get("host_observer") or (lambda: observe_installed_facts(expected_uids=sc["uids"], stop_at=sc["stop"], require_binaries=True))
    d_ver = kw.get("db_verifier") or (lambda c, d, r, u, m, p: verify_release_schema_and_identities(c, d, r, u, migration_path=m, role_prefix=p))
    q_chk = kw.get("quiescence_checker") or (lambda adsn: check_quiescence(pg_container=sc["cnt"], admin_dsn=adsn, candidate_db=sc["dbn"]))
    try:
        (kw.get("driver_verifier") or verify_driver_custody)(kw.get("driver_dir") or Path(__file__).resolve().parent, kw["expected_driver_commit"], kw["expected_driver_tree"], expected_uids=sc["uids"], stop_at=sc["stop"])
        desc = release_descriptor.verify_descriptor(kw.get("package_dir") or Path(PRIVILEGED_SCOPE["bundle_dir"]), kw["expected_descriptor_sha256"], allow_synthetic=kw.get("allow_synthetic", False), expected_uids=sc["uids"], stop_at=sc["stop"])
        custody = json.loads(read_protected_bytes(kw.get("custody_file") or Path(PRIVILEGED_SCOPE["owner_custody_path"]), expected_uids=sc["uids"], allowed_modes=(0o400,), stop_at=sc["stop"], status_on_error="REFUSED").decode("utf-8"))
        if desc["driver_commit"] != kw["expected_driver_commit"] or desc["driver_tree"] != kw["expected_driver_tree"]:
            raise BindingError("descriptor_driver_pin_mismatch")
        pkg_shas = {m["path"]: m["sha256"] for m in desc["members"]}
        img, facts = (kw.get("image_extractor") or extract_image_worker_digests)(desc["image_digest"]), h_obs()
        d_ver(sc["cnt"], sc["dbn"], custody["roles"], custody["dsns"], kw.get("migration_path", DEFAULT_MIGRATION), sc["rpx"])
        q_chk(custody["dsns"]["admin"])
    except (FileCustodyError, HostObservationError, PostgresError, release_descriptor.ReleaseError, UnicodeDecodeError, json.JSONDecodeError, KeyError) as exc:
        raise BindingError(getattr(exc, "reason", str(exc))) from exc
    err = _check_pkg_match(desc, img, facts, pkg_shas)
    if require_match and err:
        raise BindingError(err)
    observed_img = desc["image_digest"] if not err else f"sha256:{'0' * 64}"
    return _ids_from_facts(facts, observed_img), custody["dsns"]["admin"], desc, h_obs, sc

def pg_sql_executor(container: str, database: str, admin_dsn: str):
    """Build PostgreSQL executor over key-free run_psql_stdin transport."""
    cols = ", ".join(f"'{c}', {'host(resolver_ipv4)' if c == 'resolver_ipv4' else (f'{c}::text' if c in ('qualification_ref', 'boot_id') else c)}" for c in SQL_COLUMNS)
    def _exec(op: str, arg: dict | str | None = None):
        if op == "exists":
            return run_psql_stdin(container, admin_dsn, database, f"SELECT 1 FROM execution.live_qualifications WHERE qualification_ref='{_validate_uuid_non_nil(str(arg), 'qref')}'") == "1"
        if op == "insert":
            vals = (f"'{_validate_uuid_non_nil(arg['qualification_ref'], 'qref')}'::uuid", f"'{LIVE_RUN_PROFILE}'", f"'{PURPOSE_LIVE}'", *(f"'{arg[f]}'" for f in (*IDENTITY_HASH_FIELDS, "image_digest", "manifest_sha256")), f"'{_validate_uuid_non_nil(arg['boot_id'], 'boot_id')}'::uuid", f"'{validate_global_ipv4(arg['resolver_ipv4'])}'::inet", str(int(arg["valid_from_ms"])), str(int(arg["valid_until_ms"])))
            run_psql_stdin(container, admin_dsn, database, f"BEGIN; INSERT INTO execution.live_qualifications ({', '.join(SQL_COLUMNS)}) VALUES ({', '.join(vals)}); COMMIT;")
            return None
        if op == "read":
            out = run_psql_stdin(container, admin_dsn, database, f"SELECT json_build_object({cols})::text FROM execution.live_qualifications WHERE qualification_ref='{_validate_uuid_non_nil(str(arg), 'qref')}'")
            return json.loads(out) if out else None
        raise BindingError(f"unknown_sql_op:{op}")
    return _exec

def bind_live(*, resolver_ipv4: str, valid_from_ms: int, valid_until_ms: int, purpose: str, run_profile: str,
              env: dict[str, str] | None = None, clock_fn=_utc_now_ms, sql_executor=None, replacer=os.replace, **kw) -> dict:
    """Orchestrate privileged LIVE binding from verified host/package/0035 observations."""
    reject_sensitive_env(env, error_cls=lambda _s, r: BindingError(r))
    if purpose != PURPOSE_LIVE or run_profile != LIVE_RUN_PROFILE:
        raise BindingError("live_purpose_or_profile_required")
    ids, admin_dsn, desc, h_obs, sc = _observe_live_context(kw, require_match=True)
    q_chk = kw.get("quiescence_checker") or (lambda adsn: check_quiescence(pg_container=sc["cnt"], admin_dsn=adsn, candidate_db=sc["dbn"]))
    scope = {"container": sc["cnt"], "database": sc["dbn"], "manifest_path": str(sc["mfst"]), "intent_dir": str(sc["idir"])}
    return publish_binding(identities=ids, installed_identities=ids, resolver_ipv4=resolver_ipv4, valid_from_ms=valid_from_ms, valid_until_ms=valid_until_ms, manifest_path=sc["mfst"], intent_dir=sc["idir"], sql_executor=sql_executor or pg_sql_executor(sc["cnt"], sc["dbn"], admin_dsn), descriptor_sha256=kw["expected_descriptor_sha256"], scope=scope, env=env, expected_uids=sc["uids"], stop_at=sc["stop"], clock_fn=clock_fn, quiescence_checker=lambda: q_chk(admin_dsn), host_reobserver=lambda: _ids_from_facts(h_obs(), desc["image_digest"]), replacer=replacer)

def reconcile_live(*, qualification_ref: str, env: dict[str, str] | None = None, clock_fn=_utc_now_ms, sql_reader=None, **kw) -> dict:
    """Orchestrate read-only LIVE reconciliation against independent host, descriptor, and 0035 DB."""
    reject_sensitive_env(env, error_cls=lambda _s, r: BindingError(r))
    ids, admin_dsn, _, _, sc = _observe_live_context(kw, require_match=False)
    reader = sql_reader or (lambda r: pg_sql_executor(sc["cnt"], sc["dbn"], admin_dsn)("read", r))
    return reconcile_binding(qualification_ref=qualification_ref, manifest_path=sc["mfst"], installed_identities=ids, sql_reader=reader, expected_uids=sc["uids"], stop_at=sc["stop"], clock_fn=clock_fn)

_PIN_FLAGS = ("--expected-driver-commit", "--expected-driver-tree", "--expected-descriptor-sha256")
_CLI_SPEC = (
    ("render-binding", (("--release-sha256", {}), ("--launcher-sha256", {}), ("--capture-sha256", {}), ("--exporter-sha256", {}), ("--image-digest", {}), ("--boot-id", {}), ("--resolver-ipv4", {}), ("--core-uid", {"type": int}), ("--valid-from-ms", {"type": int}), ("--valid-until-ms", {"type": int}), ("--now-ms", {"type": int, "required": False}), ("--purpose", {"required": False, "default": PURPOSE_LIVE}), ("--run-profile", {"required": False, "default": LIVE_RUN_PROFILE}), ("--output-file", {"type": Path, "required": False}))),
    ("bind", tuple((f, {}) for f in (*_PIN_FLAGS, "--resolver-ipv4", "--purpose", "--run-profile")) + (("--valid-from-ms", {"type": int}), ("--valid-until-ms", {"type": int}))),
    ("reconcile", tuple((f, {}) for f in (*_PIN_FLAGS, "--qualification-ref"))),
)

def _parse_args(argv: list[str] | None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Candidate LIVE binding renderer, publisher, and reconcile")
    sub = parser.add_subparsers(dest="cmd", required=True)
    for name, flags in _CLI_SPEC:
        sp = sub.add_parser(name)
        for flag, opts in flags:
            sp.add_argument(flag, **{"required": True, **opts})
    return parser.parse_args(argv)

def main(argv: list[str] | None = None) -> int:
    args = _parse_args(argv)
    try:
        if args.cmd == "render-binding":
            ids = {k: getattr(args, k) for k in (*IDENTITY_HASH_FIELDS, "image_digest", "boot_id", "core_uid")}
            out = render_binding(identities=ids, resolver_ipv4=args.resolver_ipv4, valid_from_ms=args.valid_from_ms, valid_until_ms=args.valid_until_ms, now_ms=args.now_ms, purpose=args.purpose, run_profile=args.run_profile, output_file=args.output_file)
            print(json.dumps(out, sort_keys=True))
            return 0
        if Path(__file__).resolve().parent != Path(PRIVILEGED_SCOPE["source_dir"]) / "boundary" / "qualify":
            raise BindingError("privileged_driver_outside_fixed_source")
        out = (bind_live if args.cmd == "bind" else reconcile_live)(**{k: getattr(args, k) for k in vars(args) if k != "cmd"})
    except BindingError as exc:
        print(json.dumps({"status": "REFUSED", "reason": str(exc)}, sort_keys=True))
        return 1
    print(json.dumps(out, sort_keys=True))
    return 0 if out["status"] == "SUCCESS" else 2

if __name__ == "__main__":
    raise SystemExit(main())
