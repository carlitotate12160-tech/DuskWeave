"""Contract tests for candidate build storage, release descriptor packaging, and future install."""

import hashlib
import json
import os
import stat
import struct
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path
from unittest import mock

ROOT = Path(__file__).resolve().parents[1]
QUALIFY_DIR = ROOT / "boundary" / "qualify"
sys.path.insert(0, str(QUALIFY_DIR))

import release_descriptor  # noqa: E402


def make_synthetic_elf_aarch64(payload: bytes = b"candidate-bin") -> bytes:
    """Build a minimal valid 64-bit little-endian ARM64 ELF header + payload (SYNTHETIC)."""
    e_ident = b"\x7fELF\x02\x01\x01" + (b"\x00" * 9)
    fields = struct.pack("<HHIQQQIHHHHHH", 2, 183, 1, 0x400000, 64, 0, 0, 64, 56, 0, 64, 0, 0)
    return e_ident + fields + payload


def sha256_bytes(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


class CandidateShellContractTests(unittest.TestCase):
    def _run_bash(self, script: str, env: dict[str, str] | None = None) -> subprocess.CompletedProcess:
        clean_env = {"PATH": os.environ.get("PATH", "/usr/bin:/bin"), "HOME": "/tmp"}
        if env:
            clean_env.update(env)
        return subprocess.run(
            ["bash", "-c", script],
            capture_output=True,
            text=True,
            env=clean_env,
            timeout=15,
        )

    def test_candidate_and_qualify_are_sourceable_without_side_effects(self):
        script = f"""
set -euo pipefail
source "{QUALIFY_DIR / 'qualify.sh'}"
declare -F candidate_main >/dev/null
declare -F check_candidate_build_storage >/dev/null
declare -F build_release_into >/dev/null
declare -F build_release >/dev/null
"""
        res = self._run_bash(script)
        self.assertEqual(res.returncode, 0, res.stderr)
        self.assertEqual(res.stdout, "")

    def test_unknown_and_malformed_candidate_commands_refuse_off_host(self):
        for args in ("candidate", "candidate bogus", "candidate build-storage extra", "candidate build"):
            res = self._run_bash(f'bash "{QUALIFY_DIR / "qualify.sh"}" {args}')
            self.assertNotEqual(res.returncode, 0, f"expected refusal for {args}")

    def test_candidate_storage_allows_retained_siblings_but_refuses_symlinks_and_coupled_dev(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            build_mount = root / "duskweave-build"
            build_mount.mkdir(mode=0o755)
            for retained in ("diag-inference-pre-send-20261111r1", "proposal-diagnostics-r1", "normal-sc30cand"):
                (build_mount / retained).mkdir(mode=0o755)
            cand = build_mount / "linux-prov2-qual-r1"
            cand.mkdir(mode=0o755)
            for sub in ("normal", "go-cache", "tmp", "tmp/go", "tmp/build", "go-release", "boundary-release"):
                (cand / sub).mkdir(parents=True, exist_ok=True, mode=0o755)

            script_ok = f"""
set -euo pipefail
source "{QUALIFY_DIR / 'qualify.sh'}"
BUILD="{build_mount}"
CANDIDATE_BUILD_ROOT="{cand}"
mount_fstype() {{ printf 'ext4\\n'; }}
candidate_mountpoint() {{ return 0; }}
mount_dev() {{
    case "$1" in
        "{build_mount}") printf '100\\n' ;;
        "{cand}") printf '100\\n' ;;
        /) printf '10\\n' ;;
        /var/lib/docker|/var/lib/containerd) printf '20\\n' ;;
        *) printf '100\\n' ;;
    esac
}}
protected_ancestors() {{ return 0; }}
protected_dir() {{ return 0; }}
check_candidate_build_storage
"""
            ok = self._run_bash(script_ok)
            self.assertEqual(ok.returncode, 0, ok.stderr)

            coupled = script_ok.replace("printf '10\\n'", "printf '100\\n'")
            bad = self._run_bash(coupled)
            self.assertNotEqual(bad.returncode, 0)
            self.assertIn("build_storage_not_isolated", bad.stderr)

            (cand / "unexpected_foreign_dir").mkdir()
            foreign = self._run_bash(script_ok)
            self.assertNotEqual(foreign.returncode, 0)
            self.assertIn("build_storage_foreign_entry", foreign.stderr)

    def test_privileged_dispatch_forwards_install_subcommand(self):
        script = f"""
set +e
source "{QUALIFY_DIR / 'qualify.sh'}"
require_deployment_host() {{ return 0; }}
candidate_verify_sources() {{ return 0; }}
candidate_privileged_dispatch install --package-dir /nonexistent-pkg \\
    --expected-descriptor-sha256 {"a" * 64} \\
    --install-bin-dir /tmp --backup-dir /tmp/unused-bak
"""
        res = self._run_bash(script)
        self.assertEqual(res.returncode, 1, res.stderr)
        self.assertIn("package_dir_invalid", res.stdout)


class ReleaseDescriptorPackagingTests(unittest.TestCase):
    def _populate_source_and_artifacts(self, base: Path) -> tuple[Path, Path]:
        src = base / "src"
        art = base / "artifacts"
        (src / "third_party" / "genai").mkdir(parents=True)
        art.mkdir(parents=True)
        for name in ("duskweave", "dw-boundary-launcher", "dw-dns-capture", "dw-dns-exporter"):
            (art / name).write_bytes(make_synthetic_elf_aarch64(name.encode()))
        for notice in ("LICENSE-APACHE", "LICENSE-MIT", "source-manifest.json"):
            (src / "third_party" / "genai" / notice).write_text(f"notice:{notice}\n", encoding="utf-8")
        return src, art

    def test_package_and_verify_descriptor_round_trip(self):
        with tempfile.TemporaryDirectory() as tmp:
            base = Path(tmp)
            src, art = self._populate_source_and_artifacts(base)
            pkg = base / "pkg"
            digest = "sha256:" + ("a" * 64)
            result = release_descriptor.package_release(
                package_dir=pkg,
                source_root=src,
                artifact_dir=art,
                image_ref="dw-boundary-cand:20261115r1",
                image_digest=digest,
                build_source_commit=release_descriptor.PR154_BASELINE_COMMIT,
                build_source_tree=release_descriptor.PR154_BASELINE_TREE,
                driver_commit="1" * 40,
                driver_tree="2" * 40,
                synthetic_fixture=True,
                verify_git_baseline=False,
            )
            self.assertEqual(result["status"], "SUCCESS")
            verified = release_descriptor.verify_descriptor(
                pkg,
                expected_descriptor_sha256=result["descriptor_sha256"],
                allow_synthetic=True,
            )
            self.assertEqual(verified["schema_revision"], "0035")
            self.assertEqual(len(verified["members"]), 7)

    def test_rejects_duplicate_unknown_tamper_extra_hidden_symlink_and_bad_elf(self):
        with tempfile.TemporaryDirectory() as tmp:
            base = Path(tmp)
            src, art = self._populate_source_and_artifacts(base)
            pkg = base / "pkg"
            res = release_descriptor.package_release(
                package_dir=pkg,
                source_root=src,
                artifact_dir=art,
                image_ref="dw-boundary-cand:20261115r1",
                image_digest="sha256:" + ("b" * 64),
                build_source_commit=release_descriptor.PR154_BASELINE_COMMIT,
                build_source_tree=release_descriptor.PR154_BASELINE_TREE,
                driver_commit="1" * 40,
                driver_tree="2" * 40,
                synthetic_fixture=True,
                verify_git_baseline=False,
            )
            exp = res["descriptor_sha256"]

            # Wrong expected hash refused
            with self.assertRaises(release_descriptor.ReleaseError):
                release_descriptor.verify_descriptor(pkg, expected_descriptor_sha256="f" * 64, allow_synthetic=True)

            # Synthetic refused when real release required
            with self.assertRaises(release_descriptor.ReleaseError):
                release_descriptor.verify_descriptor(pkg, expected_descriptor_sha256=exp, allow_synthetic=False)

            # Duplicate JSON key refused
            raw = (pkg / "release-descriptor.json").read_text(encoding="utf-8")
            dup = '{"schema_version":1,' + raw[1:]
            (pkg / ".tmpcheck").write_text("hidden", encoding="utf-8")
            with self.assertRaises(release_descriptor.ReleaseError):
                release_descriptor.verify_descriptor(pkg, expected_descriptor_sha256=exp, allow_synthetic=True)
            (pkg / ".tmpcheck").unlink()

            with self.assertRaises(release_descriptor.ReleaseError):
                release_descriptor.parse_descriptor_bytes(dup.encode("utf-8"))

            # Unknown JSON key refused
            obj = json.loads(raw)
            obj["unexpected_key"] = True
            with self.assertRaises(release_descriptor.ReleaseError):
                release_descriptor.parse_descriptor_bytes(json.dumps(obj).encode("utf-8"))

            # Tampered member binary refused
            os.chmod(pkg / "duskweave", 0o755)
            (pkg / "duskweave").write_bytes(make_synthetic_elf_aarch64(b"tampered"))
            os.chmod(pkg / "duskweave", 0o555)
            with self.assertRaises(release_descriptor.ReleaseError):
                release_descriptor.verify_descriptor(pkg, expected_descriptor_sha256=exp, allow_synthetic=True)

            # Non-aarch64 ELF refused during packaging
            pkg2 = base / "pkg2"
            bad_elf = bytearray(make_synthetic_elf_aarch64(b"x86"))
            struct.pack_into("<H", bad_elf, 18, 62)  # EM_X86_64
            (art / "duskweave").write_bytes(bytes(bad_elf))
            with self.assertRaises(release_descriptor.ReleaseError):
                release_descriptor.package_release(
                    package_dir=pkg2,
                    source_root=src,
                    artifact_dir=art,
                    image_ref="dw-boundary-cand:20261115r1",
                    image_digest="sha256:" + ("b" * 64),
                    build_source_commit=release_descriptor.PR154_BASELINE_COMMIT,
                    build_source_tree=release_descriptor.PR154_BASELINE_TREE,
                    driver_commit="1" * 40,
                    driver_tree="2" * 40,
                    synthetic_fixture=True,
                    verify_git_baseline=False,
                )

    def test_refuses_secret_bearing_env_and_reports_partial_install_incomplete(self):
        with tempfile.TemporaryDirectory() as tmp:
            base = Path(tmp)
            src, art = self._populate_source_and_artifacts(base)
            pkg = base / "pkg"
            res = release_descriptor.package_release(
                package_dir=pkg,
                source_root=src,
                artifact_dir=art,
                image_ref="dw-boundary-cand:20261115r1",
                image_digest="sha256:" + ("c" * 64),
                build_source_commit=release_descriptor.PR154_BASELINE_COMMIT,
                build_source_tree=release_descriptor.PR154_BASELINE_TREE,
                driver_commit="1" * 40,
                driver_tree="2" * 40,
                synthetic_fixture=True,
                verify_git_baseline=False,
            )
            install_bin = base / "bin"
            install_bin.mkdir(mode=0o755)
            old_bytes = make_synthetic_elf_aarch64(b"old-duskweave")
            (install_bin / "duskweave").write_bytes(old_bytes)
            backup_dir = base / "backup"

            with self.assertRaises(release_descriptor.ReleaseError):
                release_descriptor.install_release(
                    package_dir=pkg,
                    expected_descriptor_sha256=res["descriptor_sha256"],
                    install_bin_dir=install_bin,
                    backup_dir=backup_dir,
                    quiescence_check=lambda: None,
                    sudoers_check=lambda: None,
                    image_worker_check=lambda desc: None,
                    env={"DEEPSEEK_API_KEY": "secret"},
                    allow_synthetic=True,
                )

            replace_calls = 0

            def flaky_replace(src_path: Path, dst_path: Path) -> None:
                nonlocal replace_calls
                replace_calls += 1
                if replace_calls == 2:
                    raise OSError("disk_full_mid_install")
                os.replace(src_path, dst_path)

            receipt = release_descriptor.install_release(
                package_dir=pkg,
                expected_descriptor_sha256=res["descriptor_sha256"],
                install_bin_dir=install_bin,
                backup_dir=backup_dir,
                quiescence_check=lambda: None,
                sudoers_check=lambda: None,
                image_worker_check=lambda desc: None,
                env={"PATH": "/usr/bin"},
                replacer=flaky_replace,
                allow_synthetic=True,
            )
            self.assertEqual(receipt["status"], "INCOMPLETE")
            self.assertTrue((backup_dir / "duskweave").is_file())
            self.assertEqual((backup_dir / "duskweave").read_bytes(), old_bytes)
            self.assertEqual((install_bin / "duskweave").read_bytes(), (pkg / "duskweave").read_bytes())
