#!/usr/bin/env python3
"""HTTPS LAB deployment manifest rendering, atomic publication, and read-only reconcile."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import time
import uuid

from candidate_binding import (
    BINARY_TO_FIELD,
    BindingError,
    HEX40_RE,
    HEX64_RE,
    IDENTITY_HASH_FIELDS,
    IMAGE_DIGEST_RE,
    MAX_MANIFEST_BYTES,
    PRO_V2_INFERENCE_PROFILE,
    SQL_COLUMNS,
    _PhaseAbort,
    _evaluate_reconcile,
    _identities_match,
    _no_dup_pairs,
    _persist_receipt,
    _preserve_intent_and_backup,
    _rec_out,
    _step,
    _step_insert_and_readback,
    _step_publish_manifest,
    _step_rec,
    _utc_now_ms,
    _validate_uuid_non_nil,
    _validate_window,
)
from candidate_files import (
    FileCustodyError,
    read_protected_bytes,
    reject_sensitive_env,
    write_exclusive_verified,
)
from candidate_postgres import PostgresError, run_psql_stdin
from https_lab_host import (
    HTTPS_LAB_SCOPE,
    HostObservationError,
    check_https_lab_quiescence,
    extract_https_lab_image_digests,
    observe_https_lab_facts,
    verify_https_lab_driver_custody,
    verify_https_lab_schema_and_identities,
)
import https_lab_release

UUID_NIL = "00000000-0000-0000-0000-000000000000"
PURPOSE_HTTPS_LAB = "HTTPS_LOOP_LAB"
RUN_PROFILE_HTTPS_LAB = "autonomous-https-genai-lab-v1"
RESOLVER_IPV4_HTTPS_LAB = "192.0.2.53"
HTTPS_LAB_ROOT_VERSION = https_lab_release.ROOT_CA_VERSION
HTTPS_LAB_ROOT_SHA256 = https_lab_release.ROOT_CA_SHA256
MANIFEST_FIELDS = frozenset({
    "qualification_ref", "purpose", "run_profile", *IDENTITY_HASH_FIELDS,
    "image_digest", "boot_id", "resolver_ipv4",
    "https_lab_root_version", "https_lab_root_sha256",
    "valid_from_ms", "valid_until_ms", "core_uid",
})
INSTALL_BINARIES = ("duskweave", "dw-boundary-launcher", "dw-dns-capture", "dw-dns-exporter")
WORKER_BINARIES = ("dw-dns-capture", "dw-dns-exporter")


def _validate_lab_identities(identities: object) -> dict:
    if not isinstance(identities, dict):
        raise BindingError("identities_not_object")
    for field in IDENTITY_HASH_FIELDS:
        val = identities.get(field)
        if not isinstance(val, str) or not HEX64_RE.fullmatch(val):
            raise BindingError(f"invalid_hash:{field}")
    img = identities.get("image_digest")
    if not isinstance(img, str) or not IMAGE_DIGEST_RE.fullmatch(img):
        raise BindingError("invalid_image_digest")
    boot_id = _validate_uuid_non_nil(identities.get("boot_id"), "boot_id")
    uid = identities.get("core_uid")
    if type(uid) is not int or uid <= 0 or uid > 2_147_483_647:
        raise BindingError("invalid_core_uid")
    return {**{k: identities[k] for k in IDENTITY_HASH_FIELDS}, "image_digest": img, "boot_id": boot_id, "core_uid": uid}


def _validate_manifest_semantics(manifest: dict, *, expected_qref: str | None = None) -> dict:
    if set(manifest.keys()) != MANIFEST_FIELDS:
        raise BindingError("manifest_unexpected_or_missing_fields")
    qref = _validate_uuid_non_nil(manifest.get("qualification_ref"), "qualification_ref")
    if expected_qref is not None and qref != expected_qref:
        raise BindingError("qualification_ref_mismatch")
    if manifest.get("purpose") != PURPOSE_HTTPS_LAB or manifest.get("run_profile") != RUN_PROFILE_HTTPS_LAB:
        raise BindingError("manifest_purpose_or_profile_invalid")
    if manifest.get("resolver_ipv4") != RESOLVER_IPV4_HTTPS_LAB:
        raise BindingError("https_lab_resolver_mismatch")
    if (
        manifest.get("https_lab_root_version") != HTTPS_LAB_ROOT_VERSION
        or manifest.get("https_lab_root_sha256") != HTTPS_LAB_ROOT_SHA256
    ):
        raise BindingError("https_lab_root_ca_mismatch")
    ids = _validate_lab_identities(manifest)
    wf, wu = manifest.get("valid_from_ms"), manifest.get("valid_until_ms")
    if type(wf) is not int or type(wu) is not int or wf <= 0 or wu <= wf:
        raise BindingError("binding_window_bounds_invalid")
    return {"qualification_ref": qref, "purpose": PURPOSE_HTTPS_LAB, "run_profile": RUN_PROFILE_HTTPS_LAB,
            **ids, "resolver_ipv4": RESOLVER_IPV4_HTTPS_LAB,
            "https_lab_root_version": HTTPS_LAB_ROOT_VERSION, "https_lab_root_sha256": HTTPS_LAB_ROOT_SHA256,
            "valid_from_ms": wf, "valid_until_ms": wu}


def build_lab_manifest_bundle(
    *,
    identities: dict,
    resolver_ipv4: str = RESOLVER_IPV4_HTTPS_LAB,
    valid_from_ms: int,
    valid_until_ms: int,
    now_ms: int,
    purpose: str = PURPOSE_HTTPS_LAB,
    run_profile: str = RUN_PROFILE_HTTPS_LAB,
    root_version: str = HTTPS_LAB_ROOT_VERSION,
    root_sha256: str = HTTPS_LAB_ROOT_SHA256,
    qualification_ref: str | None = None,
) -> tuple[dict, bytes, str, dict]:
    """Validate HTTPS LAB manifest + DB row inputs and build canonical deployment.json bytes."""
    qref = str(uuid.uuid4()) if qualification_ref is None else _validate_uuid_non_nil(qualification_ref, "qualification_ref")
    if qref == UUID_NIL:
        raise BindingError("invalid_qualification_ref")
    _validate_window(valid_from_ms, valid_until_ms, now_ms)
    raw_manifest = {
        "qualification_ref": qref, "purpose": purpose, "run_profile": run_profile,
        **_validate_lab_identities(identities), "resolver_ipv4": resolver_ipv4,
        "https_lab_root_version": root_version, "https_lab_root_sha256": root_sha256,
        "valid_from_ms": valid_from_ms, "valid_until_ms": valid_until_ms,
    }
    manifest = _validate_manifest_semantics(raw_manifest, expected_qref=qref)
    encoded = (json.dumps(manifest, sort_keys=True, separators=(",", ":")) + "\n").encode("utf-8")
    if len(encoded) > MAX_MANIFEST_BYTES:
        raise BindingError("manifest_exceeds_limit")
    m_sha = hashlib.sha256(encoded).hexdigest()
    sql_row = {col: (m_sha if col == "manifest_sha256" else manifest[col]) for col in SQL_COLUMNS}
    return manifest, encoded, m_sha, sql_row


def render_lab_binding(
    *,
    identities: dict,
    valid_from_ms: int,
    valid_until_ms: int,
    now_ms: int | None = None,
    resolver_ipv4: str = RESOLVER_IPV4_HTTPS_LAB,
    purpose: str = PURPOSE_HTTPS_LAB,
    run_profile: str = RUN_PROFILE_HTTPS_LAB,
    output_file: Path | None = None,
    clock_fn=_utc_now_ms,
) -> dict:
    """Offline render of unapplied HTTPS LAB deployment manifest and SQL row preview."""
    now = clock_fn() if now_ms is None else now_ms
    manifest, encoded, m_sha, sql_row = build_lab_manifest_bundle(
        identities=identities, resolver_ipv4=resolver_ipv4,
        valid_from_ms=valid_from_ms, valid_until_ms=valid_until_ms,
        now_ms=now, purpose=purpose, run_profile=run_profile,
    )
    if output_file is not None:
        if output_file.is_symlink() or output_file.exists():
            raise BindingError("render_output_collision")
        write_exclusive_verified(
            output_file, encoded, mode=0o444, expected_uids=(0, os.geteuid()),
            stop_at=(Path("/") if os.geteuid() == 0 else output_file.parent),
        )
    return {
        "status": "UNAPPLIED", "applied": False, "qualification_ref": manifest["qualification_ref"],
        "manifest": manifest, "manifest_sha256": m_sha, "sql_row": sql_row,
        "inference_profile": PRO_V2_INFERENCE_PROFILE,
    }


def publish_lab_binding(
    *,
    identities: dict,
    installed_identities: dict,
    valid_from_ms: int,
    valid_until_ms: int,
    manifest_path: Path,
    intent_dir: Path,
    sql_executor,
    now_ms: int | None = None,
    qualification_ref: str | None = None,
    descriptor_sha256: str = "",
    scope: dict | None = None,
    env: dict[str, str] | None = None,
    expected_uids: tuple[int, ...] = (0,),
    stop_at: Path = Path("/"),
    clock_fn=_utc_now_ms,
    quiescence_checker=None,
    host_reobserver=None,
    replacer=os.replace,
) -> dict:
    """Publish HTTPS LAB deployment.json then insert SQL qualification row with bounded receipt."""
    reject_sensitive_env(env, error_cls=lambda _s, r: BindingError(r))
    now_start = clock_fn() if now_ms is None else now_ms
    manifest, encoded, m_sha, sql_row = build_lab_manifest_bundle(
        identities=identities, valid_from_ms=valid_from_ms, valid_until_ms=valid_until_ms,
        now_ms=now_start, qualification_ref=qualification_ref,
    )
    if not _identities_match(manifest, installed_identities):
        raise BindingError("installed_identity_mismatch")
    ctx = {
        "qref": manifest["qualification_ref"], "m_sha": m_sha, "d_sha": descriptor_sha256,
        "scope": scope or {"manifest_path": str(manifest_path), "intent_dir": str(intent_dir)},
        "manifest_path": manifest_path, "intent_dir": intent_dir, "uids": expected_uids, "stop_at": stop_at,
    }
    try:
        _step(ctx, "intent_and_backup", "intent_and_backup_failed", lambda: _preserve_intent_and_backup(ctx), force_st="INCOMPLETE")
        _step_publish_manifest(ctx, encoded, sql_executor, quiescence_checker, replacer)
        res = _step_insert_and_readback(ctx, sql_executor, sql_row, manifest, encoded, clock_fn, now_start, host_reobserver)
    except _PhaseAbort as exc:
        res = exc.receipt
    return _persist_receipt(ctx, res, replacer)


def reconcile_lab_binding(
    *,
    qualification_ref: str,
    manifest_path: Path,
    installed_identities: dict,
    sql_reader,
    expected_uids: tuple[int, ...] = (0,),
    stop_at: Path = Path("/"),
    clock_fn=_utc_now_ms,
) -> dict:
    """Strictly read-only HTTPS LAB reconciliation separating consistency from current readiness."""
    qref = _validate_uuid_non_nil(qualification_ref, "qualification_ref")
    if not (manifest_path.exists() or manifest_path.is_symlink()):
        return _rec_out("INCOMPLETE", qref, "manifest_missing")
    try:
        raw = _step_rec(qref, "manifest_read_failed", lambda: read_protected_bytes(
            manifest_path, expected_uids=expected_uids, allowed_modes=(0o400, 0o444),
            max_bytes=MAX_MANIFEST_BYTES, stop_at=stop_at, status_on_error="INCOMPLETE",
        ))
        m_sha = hashlib.sha256(raw).hexdigest()
        manifest = _step_rec(qref, "manifest_invalid", lambda: _validate_manifest_semantics(
            json.loads(raw.decode("utf-8"), object_pairs_hook=_no_dup_pairs,
                       parse_constant=lambda _t: (_ for _ in ()).throw(BindingError("manifest_nonfinite_number"))),
            expected_qref=qref,
        ), m_sha=m_sha, force_st="INCOMPLETE")
        row = _step_rec(qref, "sql_read_failed", lambda: sql_reader(qref), m_sha=m_sha, force_st="UNKNOWN")
        return _evaluate_reconcile(qref, m_sha, manifest, row, installed_identities, clock_fn)
    except _PhaseAbort as exc:
        return exc.receipt


def pg_lab_sql_executor(container: str, database: str, admin_dsn: str):
    """Build PostgreSQL executor over key-free run_psql_stdin for HTTPS_LOOP_LAB."""
    cols = ", ".join(f"'{c}', {'host(resolver_ipv4)' if c == 'resolver_ipv4' else (f'{c}::text' if c in ('qualification_ref', 'boot_id') else c)}" for c in SQL_COLUMNS)
    def _exec(op: str, arg: dict | str | None = None):
        if op == "exists":
            return run_psql_stdin(container, admin_dsn, database, f"SELECT 1 FROM execution.live_qualifications WHERE qualification_ref='{_validate_uuid_non_nil(str(arg), 'qref')}'") == "1"
        if op == "insert":
            vals = (f"'{_validate_uuid_non_nil(arg['qualification_ref'], 'qref')}'::uuid", f"'{RUN_PROFILE_HTTPS_LAB}'", f"'{PURPOSE_HTTPS_LAB}'",
                    *(f"'{arg[f]}'" for f in (*IDENTITY_HASH_FIELDS, "image_digest", "manifest_sha256")),
                    f"'{_validate_uuid_non_nil(arg['boot_id'], 'boot_id')}'::uuid", f"'{RESOLVER_IPV4_HTTPS_LAB}'::inet",
                    str(int(arg["valid_from_ms"])), str(int(arg["valid_until_ms"])))
            run_psql_stdin(container, admin_dsn, database, f"BEGIN; INSERT INTO execution.live_qualifications ({', '.join(SQL_COLUMNS)}) VALUES ({', '.join(vals)}); COMMIT;")
            return None
        if op == "read":
            out = run_psql_stdin(container, admin_dsn, database, f"SELECT json_build_object({cols})::text FROM execution.live_qualifications WHERE qualification_ref='{_validate_uuid_non_nil(str(arg), 'qref')}'")
            return json.loads(out) if out else None
        raise BindingError(f"unknown_sql_op:{op}")
    return _exec


def _observe_lab_context(kw: dict, *, require_match: bool) -> tuple[dict, str, dict, callable, dict]:
    uids, stop = kw.get("expected_uids", (0,)), kw.get("stop_at", Path("/"))
    cnt, dbn = kw.get("container") or HTTPS_LAB_SCOPE["pg_container"], kw.get("database") or HTTPS_LAB_SCOPE["database"]
    rpx, mfst = kw.get("role_prefix") or HTTPS_LAB_SCOPE["role_prefix"], kw.get("manifest_path") or Path(HTTPS_LAB_SCOPE["manifest_path"])
    idir = kw.get("intent_dir") or Path(HTTPS_LAB_SCOPE["binding_intent_dir"])
    if not (HEX40_RE.fullmatch(kw["expected_driver_commit"]) and HEX40_RE.fullmatch(kw["expected_driver_tree"])):
        raise BindingError("expected_driver_pin_invalid")
    h_obs = kw.get("host_observer") or (lambda: observe_https_lab_facts(expected_uids=uids, stop_at=stop, require_binaries=True))
    d_ver = kw.get("db_verifier") or (lambda c, d, r, u, m, p: verify_https_lab_schema_and_identities(c, d, r, u, migration_0038_path=m, role_prefix=p))
    q_chk = kw.get("quiescence_checker") or (lambda adsn: check_https_lab_quiescence(pg_container=cnt, admin_dsn=adsn, candidate_db=dbn))
    try:
        (kw.get("driver_verifier") or verify_https_lab_driver_custody)(kw.get("driver_dir") or Path(__file__).resolve().parent, kw["expected_driver_commit"], kw["expected_driver_tree"], expected_uids=uids, stop_at=stop)
        desc = https_lab_release.verify_https_lab_descriptor(kw.get("package_dir") or Path(HTTPS_LAB_SCOPE["bundle_dir"]), kw["expected_descriptor_sha256"], allow_synthetic=kw.get("allow_synthetic", False), expected_uids=uids, stop_at=stop)
        custody = json.loads(read_protected_bytes(kw.get("custody_file") or Path(HTTPS_LAB_SCOPE["owner_custody_path"]), expected_uids=uids, allowed_modes=(0o400,), stop_at=stop, status_on_error="REFUSED").decode("utf-8"))
        if desc["driver_commit"] != kw["expected_driver_commit"] or desc["driver_tree"] != kw["expected_driver_tree"]:
            raise BindingError("descriptor_driver_pin_mismatch")
        if desc.get("schema_version") != https_lab_release.SCHEMA_VERSION or desc.get("purpose") != PURPOSE_HTTPS_LAB:
            raise BindingError("descriptor_schema_or_purpose_mismatch")
        img, facts = (kw.get("image_extractor") or extract_https_lab_image_digests)(desc["image_digest"]), h_obs()
        d_ver(cnt, dbn, custody["roles"], custody["dsns"], kw.get("migration_path") or Path(HTTPS_LAB_SCOPE["source_dir"]) / "migrations" / "0038_https_lab.sql", rpx)
        q_chk(custody["dsns"]["admin"])
    except (FileCustodyError, HostObservationError, PostgresError, https_lab_release.ReleaseError, UnicodeDecodeError, json.JSONDecodeError, KeyError) as exc:
        raise BindingError(getattr(exc, "reason", str(exc))) from exc
    pkg_shas = {m["path"]: m["sha256"] for m in desc["members"]}
    err = ("image_worker_bytes_mismatch" if img.get("image_id") != desc["image_digest"] or any(img["workers"].get(w) != pkg_shas[w] for w in WORKER_BINARIES)
           else "image_root_ca_mismatch" if img.get("extras", {}).get("etc/duskweave/https-lab-root-v1.pem") != HTTPS_LAB_ROOT_SHA256
           else next((f"installed_binary_package_mismatch:{b}" for b in INSTALL_BINARIES if facts["installed_hashes"].get(b) != pkg_shas[b]), None))
    if require_match and err:
        raise BindingError(err)
    obs_img = desc["image_digest"] if not err else f"sha256:{'0' * 64}"
    ids = {**{BINARY_TO_FIELD[b]: facts["installed_hashes"][b] for b in INSTALL_BINARIES}, "image_digest": obs_img, "boot_id": facts["boot_id"], "core_uid": facts["core_uid"]}
    return ids, custody["dsns"]["admin"], desc, h_obs, {"uids": uids, "stop": stop, "cnt": cnt, "dbn": dbn, "mfst": mfst, "idir": idir, "q_chk": q_chk}


def bind_https_lab(*, resolver_ipv4: str = RESOLVER_IPV4_HTTPS_LAB, valid_from_ms: int, valid_until_ms: int,
                   purpose: str = PURPOSE_HTTPS_LAB, run_profile: str = RUN_PROFILE_HTTPS_LAB,
                   env: dict[str, str] | None = None, clock_fn=_utc_now_ms, sql_executor=None, replacer=os.replace, **kw) -> dict:
    """Orchestrate privileged HTTPS_LOOP_LAB binding from verified host, package, and 0038 observations."""
    reject_sensitive_env(env, error_cls=lambda _s, r: BindingError(r))
    if purpose != PURPOSE_HTTPS_LAB or run_profile != RUN_PROFILE_HTTPS_LAB or resolver_ipv4 != RESOLVER_IPV4_HTTPS_LAB:
        raise BindingError("https_lab_binding_policy_mismatch")
    ids, admin_dsn, desc, h_obs, sc = _observe_lab_context(kw, require_match=True)
    scope = {"container": sc["cnt"], "database": sc["dbn"], "manifest_path": str(sc["mfst"]), "intent_dir": str(sc["idir"])}
    return publish_lab_binding(
        identities=ids, installed_identities=ids, valid_from_ms=valid_from_ms, valid_until_ms=valid_until_ms,
        manifest_path=sc["mfst"], intent_dir=sc["idir"],
        sql_executor=sql_executor or pg_lab_sql_executor(sc["cnt"], sc["dbn"], admin_dsn),
        descriptor_sha256=kw["expected_descriptor_sha256"], scope=scope, env=env,
        expected_uids=sc["uids"], stop_at=sc["stop"], clock_fn=clock_fn,
        quiescence_checker=lambda: sc["q_chk"](admin_dsn),
        host_reobserver=lambda: {**{BINARY_TO_FIELD[b]: h_obs()["installed_hashes"][b] for b in INSTALL_BINARIES},
                                  "image_digest": desc["image_digest"], "boot_id": ids["boot_id"], "core_uid": ids["core_uid"]},
        replacer=replacer,
    )


def reconcile_https_lab(*, qualification_ref: str, env: dict[str, str] | None = None, clock_fn=_utc_now_ms, sql_reader=None, **kw) -> dict:
    """Orchestrate read-only HTTPS_LOOP_LAB reconciliation against independent host, descriptor, and 0038 DB."""
    reject_sensitive_env(env, error_cls=lambda _s, r: BindingError(r))
    ids, admin_dsn, _, _, sc = _observe_lab_context(kw, require_match=False)
    reader = sql_reader or (lambda r: pg_lab_sql_executor(sc["cnt"], sc["dbn"], admin_dsn)("read", r))
    return reconcile_lab_binding(qualification_ref=qualification_ref, manifest_path=sc["mfst"], installed_identities=ids,
                                 sql_reader=reader, expected_uids=sc["uids"], stop_at=sc["stop"], clock_fn=clock_fn)


_PIN_FLAGS = ("--expected-driver-commit", "--expected-driver-tree", "--expected-descriptor-sha256")
_CLI_SPEC = (
    ("render-binding", (("--release-sha256", {}), ("--launcher-sha256", {}), ("--capture-sha256", {}), ("--exporter-sha256", {}), ("--image-digest", {}), ("--boot-id", {}), ("--resolver-ipv4", {"required": False, "default": RESOLVER_IPV4_HTTPS_LAB}), ("--core-uid", {"type": int}), ("--valid-from-ms", {"type": int}), ("--valid-until-ms", {"type": int}), ("--now-ms", {"type": int, "required": False}), ("--purpose", {"required": False, "default": PURPOSE_HTTPS_LAB}), ("--run-profile", {"required": False, "default": RUN_PROFILE_HTTPS_LAB}), ("--output-file", {"type": Path, "required": False}))),
    ("bind", tuple((f, {}) for f in _PIN_FLAGS) + (("--resolver-ipv4", {"required": False, "default": RESOLVER_IPV4_HTTPS_LAB}), ("--purpose", {"required": False, "default": PURPOSE_HTTPS_LAB}), ("--run-profile", {"required": False, "default": RUN_PROFILE_HTTPS_LAB}), ("--valid-from-ms", {"type": int}), ("--valid-until-ms", {"type": int}))),
    ("reconcile", tuple((f, {}) for f in (*_PIN_FLAGS, "--qualification-ref"))),
)


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description="HTTPS LAB binding renderer, publisher, and reconcile")
    sub = parser.add_subparsers(dest="cmd", required=True)
    for name, flags in _CLI_SPEC:
        sp = sub.add_parser(name)
        for flag, opts in flags:
            sp.add_argument(flag, **{"required": True, **opts})
    args = parser.parse_args(argv)
    try:
        if args.cmd == "render-binding":
            ids = {k: getattr(args, k) for k in (*IDENTITY_HASH_FIELDS, "image_digest", "boot_id", "core_uid")}
            out = render_lab_binding(identities=ids, resolver_ipv4=args.resolver_ipv4, valid_from_ms=args.valid_from_ms, valid_until_ms=args.valid_until_ms, now_ms=args.now_ms, purpose=args.purpose, run_profile=args.run_profile, output_file=args.output_file)
            print(json.dumps(out, sort_keys=True))
            return 0
        if Path(__file__).resolve().parent != Path(HTTPS_LAB_SCOPE["source_dir"]) / "boundary" / "qualify":
            raise BindingError("privileged_driver_outside_fixed_source")
        out = (bind_https_lab if args.cmd == "bind" else reconcile_https_lab)(**{k: getattr(args, k) for k in vars(args) if k != "cmd"})
    except BindingError as exc:
        print(json.dumps({"status": "REFUSED", "reason": str(exc)}, sort_keys=True))
        return 1
    print(json.dumps(out, sort_keys=True))
    return 0 if out["status"] == "SUCCESS" else 2


if __name__ == "__main__":
    raise SystemExit(main())
