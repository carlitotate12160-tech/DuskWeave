#!/usr/bin/env python3
"""Unit tests for HTTPS LAB OCI image/PEM inspection, 0038 schema verification, and shell guards."""

from __future__ import annotations

import hashlib
import json
import os
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
QUALIFY_DIR = ROOT / "boundary" / "qualify"
if str(QUALIFY_DIR) not in sys.path:
    sys.path.insert(0, str(QUALIFY_DIR))

import https_lab_host  # noqa: E402
import candidate_database  # noqa: E402
import candidate_install  # noqa: E402
import https_lab_release  # noqa: E402
from candidate_host import INSTALL_BINARIES, HostObservationError  # noqa: E402
from test_ci_candidate_host import _cp, _make_save_tar  # noqa: E402
from test_ci_candidate_release import make_synthetic_elf_aarch64  # noqa: E402


class HttpsLabHostAndShellTests(unittest.TestCase):
    def _build_lab_pkg(self, base: Path, head: str, tree: str) -> tuple[Path, Path, str, str, bytes]:
        pkg, img_id = base / "bundle", "sha256:" + ("a" * 64)
        src, art = base / "src", base / "art"
        (src / "third_party" / "genai").mkdir(parents=True)
        (src / "boundary" / "fixtures").mkdir(parents=True)
        art.mkdir(parents=True)
        pem_b = (ROOT / "boundary" / "fixtures" / "https-lab-root-v1.pem").read_bytes()
        (src / "boundary" / "fixtures" / "https-lab-root-v1.pem").write_bytes(pem_b)
        for notice in https_lab_release.NOTICE_MEMBERS:
            p = src / notice
            p.parent.mkdir(parents=True, exist_ok=True)
            p.write_text(f"notice:{notice}\n", encoding="utf-8")
        new_bins = {}
        for name in https_lab_release.BINARY_MEMBERS:
            b = make_synthetic_elf_aarch64(f"new:{name}".encode())
            (art / name).write_bytes(b)
            new_bins[name] = b
        res = https_lab_release.package_https_lab_release(
            package_dir=pkg, source_root=src, artifact_dir=art,
            image_ref="dw-https-lab:r1", image_digest=img_id,
            build_source_commit=head, build_source_tree=tree,
            driver_commit=head, driver_tree=tree,
            synthetic_fixture=True, verify_git_baseline=False,
        )
        img_info = {
            "image_id": img_id, "os": "linux", "architecture": "arm64",
            "workers": {w: hashlib.sha256(new_bins[w]).hexdigest() for w in ("dw-dns-capture", "dw-dns-exporter")},
            "extras": {https_lab_host.ROOT_CA_IMAGE_PATH: https_lab_host.ROOT_CA_SHA256},
        }
        return pkg, src, res["descriptor_sha256"], img_info, pem_b

    def _seed_installed(self, root: Path, *, with_root: bool) -> tuple[Path, Path, Path, Path, dict]:
        uid, gid = os.geteuid(), os.getegid()
        for d in (root, root / "bin", root / "etc", root / "rpt"):
            d.mkdir(mode=0o755, exist_ok=True)
            os.chmod(d, 0o755)
        old_bins = {}
        for name in INSTALL_BINARIES:
            old_b = make_synthetic_elf_aarch64(f"old:{name}".encode())
            p = root / "bin" / name
            p.write_bytes(old_b)
            os.chmod(p, 0o555)
            old_bins[name] = {"bytes": old_b, "sha256": hashlib.sha256(old_b).hexdigest(),
                              "size": len(old_b), "mode": 0o555, "uid": uid, "gid": gid}
        root_pem = root / "etc" / "https-lab-root-v1.pem"
        if with_root:
            root_pem.write_bytes((ROOT / "boundary" / "fixtures" / "https-lab-root-v1.pem").read_bytes())
            os.chmod(root_pem, 0o444)
        return root / "bin", root / "rpt" / "bak", root / "etc" / "deployment.json", root_pem, old_bins
    def test_extract_https_lab_image_digests_verifies_workers_and_root_pem(self) -> None:
        img_id = "sha256:" + ("a" * 64)
        cap_b, exp_b = b"CAP_ELF", b"EXP_ELF"
        pem_b = (ROOT / "boundary" / "fixtures" / "https-lab-root-v1.pem").read_bytes()
        meta = json.dumps({
            "Id": img_id, "Os": "linux", "Architecture": "arm64",
            "RootFS": {"Type": "layers", "Layers": ["sha256:1"]},
        })

        def runner(save_bytes: bytes):
            def _run(cmd, **_kw):
                return _cp(0, stdout=meta) if cmd[2] == "inspect" else _cp(0, stdout=save_bytes)
            return _run

        good_tar = _make_save_tar([("l0.tar", [
            ("dw-dns-capture", cap_b, {"mode": 0o555}),
            ("dw-dns-exporter", exp_b, {"mode": 0o555}),
            ("etc/", "DIR:", {"mode": 0o755}),
            ("etc/duskweave/", "DIR:", {"mode": 0o755}),
            ("etc/duskweave/https-lab-root-v1.pem", pem_b, {"mode": 0o444}),
        ])])
        got = https_lab_host.extract_https_lab_image_digests("dw-https-lab:r1", docker_runner=runner(good_tar))
        self.assertEqual(got["workers"]["dw-dns-capture"], hashlib.sha256(cap_b).hexdigest())
        self.assertEqual(got["workers"]["dw-dns-exporter"], hashlib.sha256(exp_b).hexdigest())
        self.assertEqual(got["extras"][https_lab_host.ROOT_CA_IMAGE_PATH], https_lab_host.ROOT_CA_SHA256)

        bad_dir_tar = _make_save_tar([("l0.tar", [
            ("dw-dns-capture", cap_b, {"mode": 0o555}),
            ("dw-dns-exporter", exp_b, {"mode": 0o555}),
            ("etc/", "DIR:", {"mode": 0o444}),
            ("etc/duskweave/", "DIR:", {"mode": 0o444}),
            ("etc/duskweave/https-lab-root-v1.pem", pem_b, {"mode": 0o444}),
        ])])
        with self.assertRaisesRegex(https_lab_host.HostObservationError, "^image_extra_parent_unsafe:etc$"):
            https_lab_host.extract_https_lab_image_digests("dw-https-lab:r1", docker_runner=runner(bad_dir_tar))

        bad_pem_tar = _make_save_tar([("l0.tar", [
            ("dw-dns-capture", cap_b, {"mode": 0o555}),
            ("dw-dns-exporter", exp_b, {"mode": 0o555}),
            ("etc/", "DIR:", {"mode": 0o755}),
            ("etc/duskweave/", "DIR:", {"mode": 0o755}),
            ("etc/duskweave/https-lab-root-v1.pem", b"wrong-pem", {"mode": 0o444}),
        ])])
        with self.assertRaisesRegex(https_lab_host.HostObservationError, "^image_extra_member_mismatch:"):
            https_lab_host.extract_https_lab_image_digests("dw-https-lab:r1", docker_runner=runner(bad_pem_tar))

        missing_pem_tar = _make_save_tar([("l0.tar", [("dw-dns-capture", cap_b), ("dw-dns-exporter", exp_b)])])
        with self.assertRaisesRegex(https_lab_host.HostObservationError, "^image_workers_incomplete$"):
            https_lab_host.extract_https_lab_image_digests("dw-https-lab:r1", docker_runner=runner(missing_pem_tar))

    def test_observe_installed_root_pem_and_0038_spec_extractor(self) -> None:
        uid = os.geteuid()
        pem_b = (ROOT / "boundary" / "fixtures" / "https-lab-root-v1.pem").read_bytes()
        with tempfile.TemporaryDirectory() as tmp:
            base = Path(tmp) / "etc"
            base.mkdir(mode=0o755)
            os.chmod(base, 0o755)
            pem = base / "https-lab-root-v1.pem"
            pem.write_bytes(pem_b)
            os.chmod(pem, 0o444)
            digest = https_lab_host.observe_installed_root_pem(pem, expected_uids=(uid,), stop_at=base)
            self.assertEqual(digest, https_lab_host.ROOT_CA_SHA256)

            os.chmod(pem, 0o644)
            pem.write_bytes(b"tampered")
            os.chmod(pem, 0o444)
            with self.assertRaisesRegex(https_lab_host.HostObservationError, "^https_lab_root_pem_mismatch$"):
                https_lab_host.observe_installed_root_pem(pem, expected_uids=(uid,), stop_at=base)

        specs = https_lab_host.extract_0038_function_specs(ROOT / "migrations" / "0038_https_lab.sql")
        self.assertEqual(set(specs.keys()), set(https_lab_host.REQUIRED_0038_FUNCTIONS))
        self.assertIn("execution.assert_current_pack_terrain", https_lab_host.REQUIRED_0038_FUNCTIONS)
        invoker_fns = {"execution.inference_profile_matches", "execution.assert_successor_pack"}
        for name, spec in specs.items():
            self.assertEqual(spec["security_definer"], name not in invoker_fns, name)
            self.assertEqual(spec["search_path"], "search_path=pg_catalog, pg_temp", name)
        sql_0037 = (ROOT / "migrations" / "0037_https_loop.sql").read_text(encoding="utf-8").replace("\r\n", "\n")
        m_0037 = https_lab_host.LAB_FN_DEF_RE.search(sql_0037[sql_0037.index("CREATE OR REPLACE FUNCTION execution.assert_current_pack_terrain"):])
        self.assertIsNotNone(m_0037)
        self.assertNotEqual(
            specs["execution.assert_current_pack_terrain"]["prosrc_sha256"],
            hashlib.sha256(m_0037.group(6).encode("utf-8")).hexdigest(),
        )

    def test_https_lab_sh_is_sourceable_and_refuses_off_host_privileged_subcommands(self) -> None:
        clean_env = {"PATH": os.environ.get("PATH", "/usr/bin:/bin"), "HOME": "/tmp"}
        res = subprocess.run(
            ["bash", "-c", f'set -euo pipefail; source "{QUALIFY_DIR / "https_lab.sh"}"; declare -F https_lab_main >/dev/null'],
            capture_output=True, text=True, env=clean_env, timeout=15,
        )
        self.assertEqual(res.returncode, 0, res.stderr)
        self.assertEqual(res.stdout, "")

        for args in ("", "bogus", "build-storage --expected-driver-commit " + "a" * 40):
            proc = subprocess.run(
                ["bash", str(QUALIFY_DIR / "https_lab.sh"), *([a for a in args.split() if a])],
                capture_output=True, text=True, env=clean_env, timeout=15,
            )
            self.assertNotEqual(proc.returncode, 0)

        # Verify reconciled LAB scope constants and shell delegation to https_lab_host.py.
        sh_txt = (QUALIFY_DIR / "https_lab.sh").read_text(encoding="utf-8")
        self.assertIn("HTTPS_LAB_DB=dw_native_https_lab_r1_20261119", sh_txt)
        self.assertIn("HTTPS_LAB_ROLE_PREFIX=dw_hlab_r1", sh_txt)
        self.assertIn(".live-dns-owner", sh_txt)
        self.assertIn("./cmd/dw-dns-exporter", sh_txt)
        self.assertNotIn("dw-dns-exporte\n", sh_txt)
        self.assertNotIn('$artifact_dir/httpslab', sh_txt)
        self.assertIn("https_lab_release.py\" record-responder", sh_txt)
        self.assertIn("https_lab_host.py\" provision-db", sh_txt)
        self.assertIn("https_lab_host.py\" install", sh_txt)
        self.assertEqual(https_lab_host.HTTPS_LAB_SCOPE["database"], "dw_native_https_lab_r1_20261119")
        self.assertEqual(https_lab_host.HTTPS_LAB_SCOPE["role_prefix"], "dw_hlab_r1")
        self.assertIn("candidate_install_files.py", https_lab_host.HTTPS_LAB_DRIVER_FILES)

    def test_0038_provision_parameters_and_verifier_enforcement(self) -> None:
        from unittest import mock
        uid, gid = os.geteuid(), os.getegid()
        with tempfile.TemporaryDirectory() as tmp:
            base = Path(tmp)
            os.chmod(base, 0o755)
            cfg, cst = base / "cfg", base / "cfg" / "owner-custody.json"
            with self.assertRaisesRegex(candidate_database.CandidateDatabaseError, "^schema_verifier_required$"):
                candidate_database.provision_database(
                    container="dwpg-r3", database="dw_x", role_prefix="dw_x",
                    config_root=cfg, custody_file=cst, schema_revision="0038",
                )
            seen = {}

            def fake_fx(**kw):
                seen["fx"] = kw
                kw["custody_callback"]({
                    "roles": {"admin": "dw_hlab_r1_admin", "core": "dw_hlab_r1_core"},
                    "dsns": {r: f"postgresql://dw_hlab_r1_{r}:pw@127.0.0.1:5432/{kw['database']}"
                             for r in ("admin", "core", "broker", "cognition", "history", "terrain", "stop")},
                })
                return type("Fx", (), {"database": kw["database"]})()

            def fake_sql(_c, _dsn, _db, stmt):
                seen.setdefault("sql", []).append(stmt)
                return ""

            with mock.patch.object(https_lab_host, "verify_https_lab_schema_and_identities",
                                   return_value={"database": "dw_native_https_lab_r1_20261119"}) as ver:
                rcpt = https_lab_host.provision_https_lab_database(
                    config_root=cfg, custody_file=cst, expected_uids=(uid,), stop_at=base,
                    core_account={"uid": uid, "gid": gid}, fixture_factory=fake_fx,
                    migration_0038_path=ROOT / "migrations" / "0038_https_lab.sql", sql_runner=fake_sql,
                )
            self.assertEqual(rcpt["status"], "SUCCESS")
            self.assertEqual(rcpt["schema_revision"], "0038")
            self.assertEqual(seen["fx"]["schema_revision"], "0038")
            self.assertEqual(seen["fx"]["database"], "dw_native_https_lab_r1_20261119")
            self.assertEqual(seen["fx"]["role_prefix"], "dw_hlab_r1")
            ver.assert_called_once()

    def test_https_lab_install_full_new_root_replace_partial_and_unknown_receipt(self) -> None:
        uid = os.geteuid()
        head, tree = "a" * 40, "b" * 40
        # 1. Success creating new root PEM (backup retains present=False) + 2. Replacing existing root PEM.
        for with_root in (False, True):
            with tempfile.TemporaryDirectory() as tmp:
                base = Path(tmp)
                os.chmod(base, 0o755)
                pkg, _, d_sha, img, _pem = self._build_lab_pkg(base, head, tree)
                bdir, bak, mfst, root_pem, old_bins = self._seed_installed(base / "dst", with_root=with_root)
                rcpt = base / "dst" / "rpt" / "receipt.json"
                out = candidate_install.run_https_lab_install(
                    expected_driver_commit=head, expected_driver_tree=tree, expected_descriptor_sha256=d_sha,
                    expected_build_commit=head, expected_build_tree=tree,
                    package_dir=pkg, install_bin_dir=bdir, backup_dir=bak, manifest_path=mfst,
                    root_pem_path=root_pem, receipt_path=rcpt, expected_uids=(uid,), stop_at=base,
                    allow_synthetic=True, driver_verifier=lambda *_a, **_kw: None,
                    host_observer=lambda: {"binaries": old_bins},
                    image_extractor=lambda _d: img, quiescence_checker=lambda: None,
                )
                self.assertEqual(out["status"], "SUCCESS")
                self.assertEqual(out["replaced"], [*INSTALL_BINARIES, https_lab_release.ROOT_CA_MEMBER])
                b_manifest = json.loads((bak / "backup-manifest.json").read_text(encoding="utf-8"))
                self.assertEqual(b_manifest["extra_files"][https_lab_release.ROOT_CA_MEMBER]["present"], with_root)

        # 3. Source-scope/quiescence refusal leaves destinations untouched; quiescence abort cleans staged.
        with tempfile.TemporaryDirectory() as tmp:
            base = Path(tmp)
            os.chmod(base, 0o755)
            pkg, _, d_sha, img, _pem = self._build_lab_pkg(base, head, tree)
            bdir, bak, mfst, root_pem, old_bins = self._seed_installed(base / "dst", with_root=False)
            with self.assertRaisesRegex(candidate_install.InstallError, "^descriptor_build_pin_mismatch$"):
                candidate_install.run_https_lab_install(
                    expected_driver_commit=head, expected_driver_tree=tree, expected_descriptor_sha256=d_sha,
                    expected_build_commit="c" * 40, package_dir=pkg, install_bin_dir=bdir, backup_dir=bak,
                    manifest_path=mfst, root_pem_path=root_pem, expected_uids=(uid,), stop_at=base,
                    allow_synthetic=True, driver_verifier=lambda *_a, **_kw: None,
                    host_observer=lambda: {"binaries": old_bins}, image_extractor=lambda _d: img,
                    quiescence_checker=lambda: None,
                )
            self.assertFalse(bak.exists())
            calls = {"q": 0}

            def fail_second_q() -> None:
                calls["q"] += 1
                if calls["q"] == 2:
                    raise HostObservationError("REFUSED", "active_live_session")

            abort = candidate_install.run_https_lab_install(
                expected_driver_commit=head, expected_driver_tree=tree, expected_descriptor_sha256=d_sha,
                package_dir=pkg, install_bin_dir=bdir, backup_dir=bak, manifest_path=mfst,
                root_pem_path=root_pem, receipt_path=base / "dst" / "rpt" / "rcpt.json",
                expected_uids=(uid,), stop_at=base, allow_synthetic=True,
                driver_verifier=lambda *_a, **_kw: None, host_observer=lambda: {"binaries": old_bins},
                image_extractor=lambda _d: img, quiescence_checker=fail_second_q,
            )
            self.assertEqual(abort["status"], "REFUSED")
            self.assertEqual(abort["replaced"], [])
            self.assertFalse(any(p.name.startswith(".") for p in bdir.iterdir()))

        # 4. Partial replacement returns INCOMPLETE; receipt ack loss returns UNKNOWN.
        with tempfile.TemporaryDirectory() as tmp:
            base = Path(tmp)
            os.chmod(base, 0o755)
            pkg, _, d_sha, img, _pem = self._build_lab_pkg(base, head, tree)
            bdir, bak, mfst, root_pem, old_bins = self._seed_installed(base / "dst", with_root=False)
            seen = {"n": 0}

            def fail_root_replace(src, dst) -> None:
                seen["n"] += 1
                if dst == root_pem:
                    raise OSError("disk_full")
                os.replace(src, dst)

            inc = candidate_install.run_https_lab_install(
                expected_driver_commit=head, expected_driver_tree=tree, expected_descriptor_sha256=d_sha,
                package_dir=pkg, install_bin_dir=bdir, backup_dir=bak, manifest_path=mfst,
                root_pem_path=root_pem, receipt_path=base / "dst" / "rpt" / "rcpt_inc.json",
                expected_uids=(uid,), stop_at=base, allow_synthetic=True,
                driver_verifier=lambda *_a, **_kw: None, host_observer=lambda: {"binaries": old_bins},
                image_extractor=lambda _d: img, quiescence_checker=lambda: None, replacer=fail_root_replace,
            )
            self.assertEqual(inc["status"], "INCOMPLETE")
            self.assertEqual(inc["replaced"], list(INSTALL_BINARIES))

        with tempfile.TemporaryDirectory() as tmp:
            base = Path(tmp)
            os.chmod(base, 0o755)
            pkg, _, d_sha, img, _pem = self._build_lab_pkg(base, head, tree)
            bdir, bak, mfst, root_pem, old_bins = self._seed_installed(base / "dst", with_root=False)
            bad_rcpt_dir = base / "dst" / "readonly"
            bad_rcpt_dir.mkdir(mode=0o555)
            unk = candidate_install.run_https_lab_install(
                expected_driver_commit=head, expected_driver_tree=tree, expected_descriptor_sha256=d_sha,
                package_dir=pkg, install_bin_dir=bdir, backup_dir=bak,
                manifest_path=mfst, root_pem_path=root_pem, receipt_path=bad_rcpt_dir / "rcpt.json",
                expected_uids=(uid,), stop_at=base, allow_synthetic=True,
                driver_verifier=lambda *_a, **_kw: None, host_observer=lambda: {"binaries": old_bins},
                image_extractor=lambda _d: img, quiescence_checker=lambda: None,
            )
            os.chmod(bad_rcpt_dir, 0o755)
            self.assertEqual(unk["status"], "UNKNOWN")
            self.assertEqual(len(unk["replaced"]), 5)

    def test_shell_entrypoint_and_host_cli_dispatch(self) -> None:
        from unittest import mock
        with tempfile.TemporaryDirectory() as tmp:
            base = Path(tmp)
            head, tree = "c" * 40, "d" * 40
            pkg, _, d_sha, img, _ = self._build_lab_pkg(base, head, tree)
            rsp_bin = base / "httpslab-arm64"
            rsp_prov = base / "httpslab-arm64.provenance.json"
            rsp_bin.write_bytes(make_synthetic_elf_aarch64(b"rsp"))
            os.chmod(rsp_bin, 0o555)
            clean_env = {"PATH": os.environ.get("PATH", "/usr/bin:/bin"), "HOME": tmp}
            for sub_args in (
                ["record-responder", "--binary-path", str(rsp_bin), "--provenance-path", str(rsp_prov),
                 "--build-source-commit", head, "--build-source-tree", tree, "--driver-commit", head, "--driver-tree", tree],
                ["verify-responder", "--binary-path", str(rsp_bin), "--provenance-path", str(rsp_prov),
                 "--expected-build-commit", head, "--expected-build-tree", tree, "--expected-driver-commit", head, "--expected-driver-tree", tree],
                ["render-binding", "--release-sha256", "a" * 64, "--launcher-sha256", "a" * 64,
                 "--capture-sha256", "a" * 64, "--exporter-sha256", "a" * 64,
                 "--image-digest", "sha256:" + ("a" * 64), "--boot-id", "11111111-1111-4111-8111-111111111111",
                 "--core-uid", "1001", "--valid-from-ms", "1000", "--valid-until-ms", "2000", "--now-ms", "1500"],
            ):
                p = subprocess.run(["bash", str(QUALIFY_DIR / "https_lab.sh"), *sub_args],
                                   capture_output=True, text=True, env=clean_env, timeout=15)
                self.assertEqual(p.returncode, 0, p.stderr)

            # Sourced shell functions delegate provision-db and install to https_lab_host.py.
            stub_bin = base / "bin"
            stub_bin.mkdir()
            calls_log = base / "calls.log"
            stub_py = stub_bin / "python3"
            stub_py.write_text(f"#!/usr/bin/env bash\nprintf '%s\\n' \"$*\" >> '{calls_log}'\n", encoding="utf-8")
            os.chmod(stub_py, 0o755)
            sh_cmd = (f'set -euo pipefail; PATH="{stub_bin}:$PATH"; source "{QUALIFY_DIR / "https_lab.sh"}"; '
                      'https_lab_provision_db --expected-driver-commit ' + head + '; '
                      'https_lab_install --expected-driver-commit ' + head)
            p_src = subprocess.run(["bash", "-c", sh_cmd], capture_output=True, text=True, env=clean_env, timeout=15)
            self.assertEqual(p_src.returncode, 0, p_src.stderr)
            logged = calls_log.read_text(encoding="utf-8")
            self.assertIn("https_lab_host.py provision-db", logged)
            self.assertIn("https_lab_host.py install", logged)

            # Direct https_lab_host.main refuses off-host source path and succeeds under patched source scope.
            self.assertEqual(https_lab_host.main(
                ["provision-db", "--expected-driver-commit", head, "--expected-driver-tree", tree],
            ), 1)
            patched_scope = {**https_lab_host.HTTPS_LAB_SCOPE, "source_dir": str(ROOT)}
            with mock.patch.dict(https_lab_host.HTTPS_LAB_SCOPE, patched_scope), \
                 mock.patch.object(https_lab_host, "provision_https_lab_database", return_value={"status": "SUCCESS"}), \
                 mock.patch.object(https_lab_host, "install_https_lab", return_value={"status": "SUCCESS", "descriptor_sha256": d_sha}):
                self.assertEqual(https_lab_host.main(
                    ["provision-db", "--expected-driver-commit", head, "--expected-driver-tree", tree],
                    driver_verifier=lambda *_a, **_k: None,
                ), 0)
                self.assertEqual(https_lab_host.main(
                    ["install", "--expected-driver-commit", head, "--expected-driver-tree", tree, "--expected-descriptor-sha256", d_sha],
                    driver_verifier=lambda *_a, **_k: None,
                ), 0)

    def test_https_lab_sh_sourcing_inert_and_fail_closed_across_dispatches_and_build(self) -> None:
        head, tree = "a" * 40, "b" * 40
        with tempfile.TemporaryDirectory() as tmp:
            base = Path(tmp)
            marker = base / "downstream.log"
            stub_bin = base / "bin"
            stub_bin.mkdir()
            stub_py = stub_bin / "python3"
            stub_py.write_text("#!/usr/bin/env bash\necho custody_fail >&2\nexit 1\n", encoding="utf-8")
            os.chmod(stub_py, 0o755)
            env = {"PATH": f"{stub_bin}:{os.environ.get('PATH', '/usr/bin:/bin')}", "HOME": tmp}

            # 1. Sourcing preserves caller shell options (with and without -euo pipefail) and executes nothing.
            for prefix in ("set +euo pipefail", "set -euo pipefail"):
                res = subprocess.run(
                    ["bash", "-c", f'{prefix}; b_f="$-"; b_p=$(shopt -po pipefail); source "{QUALIFY_DIR / "https_lab.sh"}"; [[ $- == "$b_f" && $(shopt -po pipefail) == "$b_p" ]]'],
                    capture_output=True, text=True, env=env, timeout=15,
                )
                self.assertEqual(res.returncode, 0, res.stderr)
                self.assertEqual(res.stdout, "")

            # 2. All 6 privileged dispatches fail closed on custody failure under ordinary AND conditional calls.
            setup = (
                f'source "{QUALIFY_DIR / "https_lab.sh"}"; '
                'source() { case "$1" in */candidate.sh) return 0 ;; *) builtin source "$@" ;; esac; }; '
                'https_lab_require_host() { return 0; }; https_lab_require_clean_env() { return 0; }; '
                f'check_candidate_build_storage() {{ echo storage >> "{marker}"; }}; '
                f'https_lab_build() {{ echo build >> "{marker}"; }}; '
                f'https_lab_provision_db() {{ echo prov >> "{marker}"; }}; '
                f'https_lab_install() {{ echo inst >> "{marker}"; }}; '
                f'python3() {{ case "$*" in *https_lab_binding.py*) echo bind >> "{marker}" ;; *) command python3 "$@" ;; esac; }}; '
            )
            for cmd_name in ("build-storage", "build", "provision-db", "install", "bind", "reconcile"):
                args = f'{cmd_name} --expected-driver-commit {head} --expected-driver-tree {tree}'
                for inv in (
                    f'set +e; https_lab_main {args}',
                    f'if https_lab_main {args}; then exit 0; fi; exit 1',
                    f'rc=0; https_lab_main {args} || rc=$?; exit "$rc"',
                ):
                    r = subprocess.run(["bash", "-c", setup + inv], capture_output=True, text=True, env=env, timeout=15)
                    self.assertNotEqual(r.returncode, 0, f"{cmd_name} / {inv}")
                    self.assertFalse(marker.exists(), marker.read_text() if marker.exists() else "")

            # 3. Inner build producer failures never invoke record-responder or package.
            src = base / "src"
            (src / "boundary" / "fixtures").mkdir(parents=True)
            (src / "boundary" / "Dockerfile.https-lab").write_text("FROM scratch\n", encoding="utf-8")
            (src / "boundary" / "fixtures" / "https-lab-root-v1.pem").write_text("PEM\n", encoding="utf-8")
            for fail_at in ("verify_source", "cargo", "go", "docker_build", "docker_inspect"):
                marker.unlink(missing_ok=True)
                b_sh = (
                    f'source "{QUALIFY_DIR / "https_lab.sh"}"; '
                    f'HTTPS_LAB_FIXED_SOURCE="{src}"; HTTPS_LAB_BUILD_ROOT="{base}/br_{fail_at}"; '
                    f'HTTPS_LAB_BUNDLE_DIR="{base}/bd_{fail_at}"; HTTPS_LAB_REPORT_ROOT="{base}/rp_{fail_at}"; '
                    'source() { case "$1" in */candidate.sh) return 0 ;; *) builtin source "$@" ;; esac; }; '
                    'check_candidate_build_storage() { return 0; }; '
                    'install() { [[ $1 == -d ]] && mkdir -p "${@: -1}"; return 0; }; stat() { echo 1; }; '
                    'cp() { : > "${@: -1}"; }; chmod() { return 0; }; '
                    f'cargo() {{ [[ {fail_at} == cargo ]] && return 1; mkdir -p "$HTTPS_LAB_BUILD_ROOT/normal/release"; : > "$HTTPS_LAB_BUILD_ROOT/normal/release/duskweave"; : > "$HTTPS_LAB_BUILD_ROOT/normal/release/dw-boundary-launcher"; }}; '
                    f'go() {{ [[ {fail_at} == go ]] && return 1; while [[ $# -gt 1 ]]; do shift; done; : > "${{1:-/dev/null}}" 2>/dev/null || true; }}; '
                    f'docker() {{ if [[ $1 == image && $2 == inspect && $* != *--format* ]]; then return 1; fi; '
                    f'if [[ $1 == build ]]; then [[ {fail_at} == docker_build ]] && return 1 || return 0; fi; '
                    f'[[ {fail_at} == docker_inspect ]] && return 1 || echo "sha256:{"a"*64}"; }}; '
                    f'python3() {{ case "$*" in *verify-source*) [[ {fail_at} != verify_source ]] ;; '
                    f'*record-responder*|*package*) echo "$*" >> "{marker}" ;; esac; }}; '
                    f'if https_lab_build --expected-driver-commit {head} --expected-driver-tree {tree}; then exit 0; fi; exit 1'
                )
                r = subprocess.run(["bash", "-c", b_sh], capture_output=True, text=True, env=env, timeout=15)
                self.assertNotEqual(r.returncode, 0, fail_at)
                self.assertFalse(marker.exists(), marker.read_text() if marker.exists() else "")


if __name__ == "__main__":
    unittest.main()
