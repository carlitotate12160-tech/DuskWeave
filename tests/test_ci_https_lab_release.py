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

ROOT = Path(__file__).resolve().parents[1]
QUALIFY_DIR = ROOT / "boundary" / "qualify"
if str(QUALIFY_DIR) not in sys.path:
    sys.path.insert(0, str(QUALIFY_DIR))

import https_lab_release  # noqa: E402
from test_ci_candidate_release import _init_clean_git_repo, make_synthetic_elf_aarch64  # noqa: E402


class HttpsLabReleaseTests(unittest.TestCase):
    def _populate(self, base: Path, *, root_pem: bytes | None = None) -> tuple[Path, Path, str, str]:
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
        head, tree = _init_clean_git_repo(src, files)
        return src, art, head, tree

    def test_committed_root_pem_fixture_matches_pinned_sha_and_size(self) -> None:
        pem = (ROOT / https_lab_release.ROOT_CA_SOURCE_REL).read_bytes()
        self.assertEqual(len(pem), https_lab_release.ROOT_CA_SIZE)
        self.assertEqual(hashlib.sha256(pem).hexdigest(), https_lab_release.ROOT_CA_SHA256)

    def test_package_and_verify_https_lab_round_trip_and_tamper_refusals(self) -> None:
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
            self.assertEqual(desc["schema_revision"], "0038")
            self.assertEqual(len(desc["members"]), 9)
            self.assertEqual(desc["https_lab_root_version"], https_lab_release.ROOT_CA_VERSION)
            self.assertEqual(desc["https_lab_root_sha256"], https_lab_release.ROOT_CA_SHA256)

            for field, val in (
                ("schema_revision", "0036"),
                ("https_lab_root_version", "wrong-root"),
                ("https_lab_root_sha256", "b" * 64),
            ):
                bad = {**desc, field: val}
                with self.assertRaises(https_lab_release.ReleaseError):
                    https_lab_release.parse_https_lab_descriptor_bytes(json.dumps(bad).encode())

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


if __name__ == "__main__":
    unittest.main()
