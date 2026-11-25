#!/usr/bin/env python3
"""Unit tests for HTTPS LAB schema-0038 release packaging and descriptor verification."""

from __future__ import annotations

import hashlib
import json
import os
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path
from unittest import mock

ROOT = Path(__file__).resolve().parents[1]
QUALIFY_DIR = ROOT / "boundary" / "qualify"
if str(QUALIFY_DIR) not in sys.path:
    sys.path.insert(0, str(QUALIFY_DIR))

import https_lab_release  # noqa: E402
import release_descriptor  # noqa: E402
from test_ci_candidate_release import _init_clean_git_repo, make_synthetic_elf_aarch64  # noqa: E402

MUTANTS_TOML_BYTES = (ROOT / ".cargo" / "mutants.toml").read_bytes()


class HttpsLabReleaseTests(unittest.TestCase):
    def _populate(
        self, base: Path, *, root_pem: bytes | None = None, extra_files: dict[str, bytes] | None = None,
    ) -> tuple[Path, Path, str, str]:
        src, art = base / "src", base / "art"
        art.mkdir(parents=True)
        for name in https_lab_release.BINARY_MEMBERS:
            (art / name).write_bytes(make_synthetic_elf_aarch64(name.encode()))
        files: dict[str, bytes] = {}
        for notice in https_lab_release.NOTICE_MEMBERS:
            files[notice] = f"notice:{notice}\n".encode()
        pem_path = ROOT / https_lab_release.ROOT_CA_SOURCE_REL
        files[https_lab_release.ROOT_CA_SOURCE_REL] = pem_path.read_bytes() if root_pem is None else root_pem
        files["Cargo.toml"] = b"[package]\nname='dw'\n"
        if extra_files:
            files.update(extra_files)
        head, tree = _init_clean_git_repo(src, files)
        return src, art, head, tree

    def test_committed_root_pem_fixture_matches_pinned_sha_and_size(self) -> None:
        pem = (ROOT / https_lab_release.ROOT_CA_SOURCE_REL).read_bytes()
        self.assertEqual(len(pem), https_lab_release.ROOT_CA_SIZE)
        self.assertEqual(hashlib.sha256(pem).hexdigest(), https_lab_release.ROOT_CA_SHA256)

    def test_package_and_verify_https_lab_round_trip_and_tamper_refusals(self) -> None:
        self.assertEqual(len(https_lab_release.FIXED_MEMBERS), 8)
        self.assertNotIn("httpslab", https_lab_release.BINARY_MEMBERS)
        with tempfile.TemporaryDirectory() as tmp:
            base = Path(tmp)
            src, art, head, tree = self._populate(base)
            pkg = base / "bundle"
            res = https_lab_release.package_https_lab_release(
                package_dir=pkg, source_root=src, artifact_dir=art,
                image_ref="dw-https-lab:r1", image_digest="sha256:" + ("a" * 64),
                build_source_commit=head, build_source_tree=tree,
                driver_commit=head, driver_tree=tree,
                synthetic_fixture=True, verify_git_baseline=True,
            )
            self.assertEqual(res["status"], "SUCCESS")
            desc = https_lab_release.verify_https_lab_descriptor(pkg, res["descriptor_sha256"], allow_synthetic=True)
            self.assertEqual(desc["schema_version"], 3)
            self.assertEqual(desc["purpose"], "HTTPS_LOOP_LAB")
            self.assertEqual(desc["schema_revision"], "0038")
            self.assertEqual(desc["release_baseline_commit"], https_lab_release.RELEASE_BASELINE_COMMIT)
            self.assertEqual(desc["release_baseline_tree"], https_lab_release.RELEASE_BASELINE_TREE)
            self.assertEqual(len(desc["members"]), 8)
            self.assertEqual(desc["https_lab_root_version"], https_lab_release.ROOT_CA_VERSION)
            self.assertEqual(desc["https_lab_root_sha256"], https_lab_release.ROOT_CA_SHA256)

            for field, val in (
                ("schema_version", 2),
                ("purpose", "LIVE"),
                ("schema_revision", "0036"),
                ("release_baseline_commit", "0" * 40),
                ("release_baseline_tree", "0" * 40),
                ("build_source_commit", https_lab_release.RELEASE_BASELINE_COMMIT),
                ("build_source_tree", https_lab_release.RELEASE_BASELINE_TREE),
                ("driver_commit", tree),
                ("driver_tree", head),
                ("https_lab_root_version", "wrong-root"),
                ("https_lab_root_sha256", "b" * 64),
            ):
                bad = {**desc, field: val}
                with self.assertRaises(https_lab_release.ReleaseError):
                    https_lab_release.parse_https_lab_descriptor_bytes(json.dumps(bad).encode())

            missing_purpose = dict(desc)
            missing_purpose.pop("purpose")
            with self.assertRaises(https_lab_release.ReleaseError):
                https_lab_release.parse_https_lab_descriptor_bytes(json.dumps(missing_purpose).encode())

            with self.assertRaises(https_lab_release.ReleaseError):
                https_lab_release.verify_https_lab_descriptor(pkg, res["descriptor_sha256"], allow_synthetic=False)

            os.chmod(pkg / https_lab_release.ROOT_CA_MEMBER, 0o644)
            (pkg / https_lab_release.ROOT_CA_MEMBER).write_bytes(b"tampered-pem")
            os.chmod(pkg / https_lab_release.ROOT_CA_MEMBER, 0o444)
            with self.assertRaises(https_lab_release.ReleaseError):
                https_lab_release.verify_https_lab_descriptor(pkg, res["descriptor_sha256"], allow_synthetic=True)

            bad_src_dir = base / "badsrc"
            b_src, b_art, b_head, b_tree = self._populate(bad_src_dir, root_pem=b"wrong-ca-bytes")
            with self.assertRaisesRegex(https_lab_release.ReleaseError, "^https_lab_root_pem_mismatch$"):
                https_lab_release.package_https_lab_release(
                    package_dir=bad_src_dir / "bundle", source_root=b_src, artifact_dir=b_art,
                    image_ref="dw-https-lab:r1", image_digest="sha256:" + ("a" * 64),
                    build_source_commit=b_head, build_source_tree=b_tree,
                    driver_commit=b_head, driver_tree=b_tree,
                )

    def test_responder_provenance_record_verify_and_tamper_rejection(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            base = Path(tmp)
            _, _, head, tree = self._populate(base)
            rsp_bin = base / "httpslab-arm64"
            rsp_prov = base / "httpslab-arm64.provenance.json"
            rsp_bin.write_bytes(make_synthetic_elf_aarch64(b"httpslab-arm64"))
            os.chmod(rsp_bin, 0o555)
            rec = https_lab_release.record_responder_provenance(
                binary_path=rsp_bin, provenance_path=rsp_prov,
                build_source_commit=head, build_source_tree=tree,
                driver_commit=head, driver_tree=tree,
            )
            self.assertEqual(rec["status"], "SUCCESS")
            ver = https_lab_release.verify_responder_provenance(
                binary_path=rsp_bin, provenance_path=rsp_prov,
                expected_build_commit=head, expected_build_tree=tree,
                expected_driver_commit=head, expected_driver_tree=tree,
            )
            self.assertEqual(ver["status"], "SUCCESS")
            self.assertEqual(ver["provenance"]["purpose"], "HTTPS_LOOP_LAB")
            with self.assertRaises(https_lab_release.ReleaseError):
                https_lab_release.verify_responder_provenance(
                    binary_path=rsp_bin, provenance_path=rsp_prov,
                    expected_build_commit="0" * 40,
                )
            os.chmod(rsp_bin, 0o755)
            rsp_bin.write_bytes(make_synthetic_elf_aarch64(b"tampered-rsp"))
            os.chmod(rsp_bin, 0o555)
            with self.assertRaises(https_lab_release.ReleaseError):
                https_lab_release.verify_responder_provenance(binary_path=rsp_bin, provenance_path=rsp_prov)

    def test_verify_descriptor_cli_refuses_tampered_hash_safely(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            proc = subprocess.run(
                [sys.executable, str(QUALIFY_DIR / "https_lab_release.py"),
                 "verify-descriptor", "--package-dir", tmp, "--expected-descriptor-sha256", "not-hex"],
                capture_output=True, text=True, timeout=10,
            )
            self.assertEqual(proc.returncode, 1)
            out = json.loads(proc.stdout)
            self.assertEqual(out["status"], "REFUSED")
            self.assertEqual(out["reason"], "expected_descriptor_sha256_invalid")

    def test_source_verifiers_accept_clean_tracked_mutants_toml_and_enforce_cargo_policy(self) -> None:
        uid = os.geteuid()
        env = {"PATH": "/usr/bin:/bin", "LC_ALL": "C"}
        with tempfile.TemporaryDirectory() as tmp:
            base = Path(tmp)
            # 1. Absent historical .cargo passes both verifiers.
            s_abs, _, h_abs, t_abs = self._populate(base / "abs")
            self.assertEqual(https_lab_release.verify_https_lab_source(s_abs, expected_commit=h_abs, expected_tree=t_abs), (h_abs, t_abs))
            with mock.patch.multiple(release_descriptor, RELEASE_BASELINE_COMMIT=h_abs, RELEASE_BASELINE_TREE=t_abs):
                self.assertEqual(release_descriptor.verify_release_source(s_abs, expected_commit=h_abs, expected_tree=t_abs), (h_abs, t_abs))

            # 2. Clean tracked exact .cargo/mutants.toml passes both verifiers and full package_https_lab_release.
            src, art, head, tree = self._populate(base / "ok", extra_files={".cargo/mutants.toml": MUTANTS_TOML_BYTES})
            self.assertEqual(
                https_lab_release.verify_https_lab_source(
                    src, expected_commit=head, expected_tree=tree,
                    expected_driver_commit=head, expected_driver_tree=tree,
                ),
                (head, tree),
            )
            pkg = base / "ok" / "bundle"
            res = https_lab_release.package_https_lab_release(
                package_dir=pkg, source_root=src, artifact_dir=art,
                image_ref="dw-https-lab:r1", image_digest="sha256:" + ("a" * 64),
                build_source_commit=head, build_source_tree=tree,
                driver_commit=head, driver_tree=tree, synthetic_fixture=True, verify_git_baseline=True,
            )
            self.assertEqual(res["status"], "SUCCESS")
            with mock.patch.multiple(release_descriptor, RELEASE_BASELINE_COMMIT=head, RELEASE_BASELINE_TREE=tree):
                self.assertEqual(
                    release_descriptor.verify_release_source(
                        src, expected_commit=head, expected_tree=tree,
                        expected_driver_commit=head, expected_driver_tree=tree,
                    ),
                    (head, tree),
                )

            # 3. Legacy drift detection and root build.rs / CA checks remain enforced alongside .cargo/mutants.toml.
            (s_abs / ".cargo").mkdir(mode=0o755)
            (s_abs / ".cargo" / "mutants.toml").write_bytes(MUTANTS_TOML_BYTES)
            os.chmod(s_abs / ".cargo" / "mutants.toml", 0o444)
            subprocess.run(["git", "-C", str(s_abs), "add", ".cargo/mutants.toml"], check=True, env=env)
            subprocess.run(["git", "-C", str(s_abs), "commit", "-qm", "add mutants"], check=True, env=env)
            h_drift = subprocess.check_output(["git", "-C", str(s_abs), "rev-parse", "HEAD"], text=True, env=env).strip()
            with mock.patch.multiple(release_descriptor, RELEASE_BASELINE_COMMIT=h_abs, RELEASE_BASELINE_TREE=t_abs):
                with self.assertRaisesRegex(release_descriptor.ReleaseError, "production_source_drift"):
                    release_descriptor.verify_release_source(s_abs, expected_commit=h_drift)

            (src / "build.rs").write_text("fn main() {}\n", encoding="utf-8")
            for fn in (https_lab_release.verify_https_lab_source, release_descriptor.verify_release_source):
                with self.assertRaisesRegex(release_descriptor.ReleaseError, "^unexpected_root_build_rs$"):
                    fn(src, expected_commit=head, expected_tree=tree)
            (src / "build.rs").unlink()

            # 4. check_clean_git=False requires valid explicit pin for real checkouts; Git-free absent .cargo passes.
            gitfree = base / "gitfree"
            (gitfree / "boundary" / "fixtures").mkdir(parents=True)
            ca_p = gitfree / https_lab_release.ROOT_CA_SOURCE_REL
            ca_p.write_bytes((ROOT / https_lab_release.ROOT_CA_SOURCE_REL).read_bytes())
            self.assertEqual(https_lab_release.verify_https_lab_source(gitfree, check_clean_git=False), ("", ""))
            (gitfree / ".cargo").mkdir()
            with self.assertRaisesRegex(release_descriptor.ReleaseError, "^candidate_cargo_config_present$"):
                https_lab_release.verify_https_lab_source(gitfree, check_clean_git=False)

            for bad_args in (
                {},
                {"expected_commit": "0" * 40, "expected_tree": tree},
                {"expected_commit": head, "expected_tree": "0" * 40},
                {"expected_commit": head, "expected_driver_commit": "0" * 40},
            ):
                with self.assertRaises(release_descriptor.ReleaseError):
                    https_lab_release.verify_https_lab_source(src, check_clean_git=False, **bad_args)
            self.assertEqual(
                https_lab_release.verify_https_lab_source(src, expected_commit=head, expected_tree=tree, check_clean_git=False),
                ("", ""),
            )
            os.unlink(src / ".cargo" / "mutants.toml")
            with self.assertRaises(release_descriptor.ReleaseError):
                https_lab_release.verify_https_lab_source(src, expected_commit=head, expected_tree=tree, check_clean_git=False)
            (src / ".cargo" / "mutants.toml").write_bytes(b"modified = true\n")
            with self.assertRaises(release_descriptor.ReleaseError):
                https_lab_release.verify_https_lab_source(src, expected_commit=head, expected_tree=tree, check_clean_git=False)

            # 5. Committed forbidden Cargo files, mode 100755, unsafe perm/owner, hardlink, symlinks, extras all refuse.
            for label, files in (
                ("cfg", {".cargo/config": b"[build]\n"}),
                ("cfgtoml", {".cargo/config.toml": b"[net]\noffline=true\n"}),
            ):
                bs, _, bh, bt = self._populate(base / f"bad_{label}", extra_files=files)
                with mock.patch.multiple(release_descriptor, RELEASE_BASELINE_COMMIT=bh, RELEASE_BASELINE_TREE=bt):
                    for fn in (https_lab_release.verify_https_lab_source, release_descriptor.verify_release_source):
                        with self.assertRaisesRegex(release_descriptor.ReleaseError, "^candidate_cargo_config_present$"):
                            fn(bs, expected_commit=bh, expected_tree=bt)

            ex_s, _, ex_h, ex_t = self._populate(base / "exec_mut", extra_files={".cargo/mutants.toml": MUTANTS_TOML_BYTES})
            os.chmod(ex_s / ".cargo" / "mutants.toml", 0o755)
            subprocess.run(["git", "-C", str(ex_s), "add", ".cargo/mutants.toml"], check=True, env=env)
            subprocess.run(["git", "-C", str(ex_s), "commit", "-qm", "exec"], check=True, env=env)
            ex_h = subprocess.check_output(["git", "-C", str(ex_s), "rev-parse", "HEAD"], text=True, env=env).strip()
            ex_t = subprocess.check_output(["git", "-C", str(ex_s), "rev-parse", "HEAD^{tree}"], text=True, env=env).strip()
            with self.assertRaises(release_descriptor.ReleaseError):
                https_lab_release.verify_https_lab_source(ex_s, expected_commit=ex_h, expected_tree=ex_t)

            # Unsafe owner/mode, empty .cargo, hardlink, symlinks, hidden/ignored/untracked extras on clean base.
            s2, _, h2, t2 = self._populate(base / "muts", extra_files={".cargo/mutants.toml": MUTANTS_TOML_BYTES, ".gitignore": b".cargo/ign.toml\n"})
            with self.assertRaises(release_descriptor.ReleaseError):
                https_lab_release.verify_https_lab_source(s2, expected_commit=h2, expected_tree=t2, expected_uids=(uid + 1,))
            mut2 = s2 / ".cargo" / "mutants.toml"
            os.chmod(mut2, 0o666)
            with self.assertRaises(release_descriptor.ReleaseError):
                https_lab_release.verify_https_lab_source(s2, expected_commit=h2, expected_tree=t2)
            os.chmod(mut2, 0o444)
            hard2 = base / "hard2"
            os.link(mut2, hard2)
            with self.assertRaises(release_descriptor.ReleaseError):
                https_lab_release.verify_https_lab_source(s2, expected_commit=h2, expected_tree=t2)
            hard2.unlink()
            for extra in (".hidden", "ign.toml", "extra.toml"):
                ep = s2 / ".cargo" / extra
                ep.write_bytes(b"x")
                with self.assertRaises(release_descriptor.ReleaseError):
                    https_lab_release.verify_https_lab_source(s2, expected_commit=h2, expected_tree=t2)
                ep.unlink()
            (s2 / ".cargo" / "sub").mkdir()
            with self.assertRaises(release_descriptor.ReleaseError):
                https_lab_release.verify_https_lab_source(s2, expected_commit=h2, expected_tree=t2)
            (s2 / ".cargo" / "sub").rmdir()
            os.unlink(mut2)
            sym_tgt = base / "sym_tgt"
            sym_tgt.write_bytes(MUTANTS_TOML_BYTES)
            mut2.symlink_to(sym_tgt)
            with self.assertRaises(release_descriptor.ReleaseError):
                https_lab_release.verify_https_lab_source(s2, expected_commit=h2, expected_tree=t2)
            mut2.unlink()
            with self.assertRaises(release_descriptor.ReleaseError):
                https_lab_release.verify_https_lab_source(s2, expected_commit=h2, expected_tree=t2)


if __name__ == "__main__":
    unittest.main()
