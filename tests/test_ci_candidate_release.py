"""Contract tests for candidate build storage, release descriptor packaging, and atomic install."""

import errno
import hashlib
import json
import os
import stat
import struct
import subprocess
import sys
import tempfile
import unittest
from unittest import mock
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
QUALIFY_DIR = ROOT / "boundary" / "qualify"
if str(QUALIFY_DIR) not in sys.path:
    sys.path.insert(0, str(QUALIFY_DIR))

import candidate_files  # noqa: E402
import candidate_host  # noqa: E402
import candidate_install  # noqa: E402
import release_descriptor  # noqa: E402


def make_synthetic_elf_aarch64(payload: bytes = b"candidate-bin") -> bytes:
    """Build a minimal valid 64-bit little-endian ARM64 ELF header + payload (SYNTHETIC)."""
    e_ident = b"\x7fELF\x02\x01\x01" + (b"\x00" * 9)
    fields = struct.pack("<HHIQQQIHHHHHH", 2, 183, 1, 0x400000, 64, 0, 0, 64, 56, 0, 64, 0, 0)
    return e_ident + fields + payload


def _init_clean_git_repo(repo: Path, files: dict[str, bytes]) -> tuple[str, str]:
    for rel, content in files.items():
        target = repo / rel
        target.parent.mkdir(parents=True, exist_ok=True, mode=0o755)
        target.write_bytes(content)
        os.chmod(target, 0o444)
    for d in [repo, *repo.rglob("*")]:
        if d.is_dir():
            os.chmod(d, 0o755)
    env = {"PATH": "/usr/bin:/bin", "LC_ALL": "C"}
    subprocess.run(["git", "-C", str(repo), "init", "-q"], check=True, env=env)
    subprocess.run(["git", "-C", str(repo), "config", "user.email", "t@e.invalid"], check=True, env=env)
    subprocess.run(["git", "-C", str(repo), "config", "user.name", "Test"], check=True, env=env)
    subprocess.run(["git", "-C", str(repo), "add", "."], check=True, env=env)
    subprocess.run(["git", "-C", str(repo), "commit", "-q", "-m", "init"], check=True, env=env)
    head = subprocess.check_output(["git", "-C", str(repo), "rev-parse", "HEAD"], text=True, env=env).strip()
    tree = subprocess.check_output(["git", "-C", str(repo), "rev-parse", "HEAD^{tree}"], text=True, env=env).strip()
    return head, tree


class CandidateShellContractTests(unittest.TestCase):
    def _run_bash(self, script: str, env: dict[str, str] | None = None) -> subprocess.CompletedProcess:
        clean_env = {"PATH": os.environ.get("PATH", "/usr/bin:/bin"), "HOME": "/tmp"}
        if env:
            clean_env.update(env)
        return subprocess.run(["bash", "-c", script], capture_output=True, text=True, env=clean_env, timeout=15)

    def test_qualify_is_sourceable_without_side_effects_or_eager_candidate_source(self):
        script = f"""
set -euo pipefail
source "{QUALIFY_DIR / 'qualify.sh'}"
! declare -F candidate_main >/dev/null
declare -F qualify_verify_driver_before_load >/dev/null
declare -F build_release >/dev/null
qualify_load_candidate_sh
declare -F candidate_main >/dev/null
declare -F check_candidate_build_storage >/dev/null
"""
        res = self._run_bash(script)
        self.assertEqual(res.returncode, 0, res.stderr)
        self.assertEqual(res.stdout, "")

    def test_poisoned_candidate_sh_is_never_sourced_on_untrusted_location(self):
        with tempfile.TemporaryDirectory() as tmp:
            repo = Path(tmp) / "repo"
            qdir = repo / "boundary" / "qualify"
            qdir.mkdir(parents=True, mode=0o755)
            for name in ("qualify.sh", "deployment_custody.sh"):
                (qdir / name).write_text((QUALIFY_DIR / name).read_text(encoding="utf-8"), encoding="utf-8")
            (qdir / "candidate.sh").write_text("echo POISONED >&2\nexit 99\n", encoding="utf-8")
            res = self._run_bash(
                f'bash "{qdir / "qualify.sh"}" candidate install --expected-driver-commit {"a" * 40} --expected-driver-tree {"b" * 40} --expected-descriptor-sha256 {"c" * 64}'
            )
            self.assertEqual(res.returncode, 1)
            self.assertNotIn("POISONED", res.stderr)

    def test_unknown_and_malformed_candidate_commands_refuse_off_host(self):
        for args in ("candidate", "candidate bogus", "candidate build-storage extra", "candidate build"):
            res = self._run_bash(f'bash "{QUALIFY_DIR / "qualify.sh"}" {args}')
            self.assertNotEqual(res.returncode, 0, f"expected refusal for {args}")

    def test_clean_env_refuses_override_vars(self):
        for key, val in (("CANDIDATE_BUILD_ROOT", "/tmp/evil"), ("CARGO_TARGET_DIR", "/tmp/evil"), ("DEEPSEEK_API_KEY", "sk-1")):
            res = self._run_bash(
                f'source "{QUALIFY_DIR / "qualify.sh"}"; qualify_load_candidate_sh; candidate_require_clean_env',
                env={key: val},
            )
            self.assertNotEqual(res.returncode, 0, f"expected env refusal for {key}")
            self.assertIn("candidate_env_refused", res.stderr)

    def test_storage_checks_components_nested_mountinfo_and_coupled_dev(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            build_mount = root / "duskweave-build"
            build_mount.mkdir(mode=0o755)
            (build_mount / "retained").mkdir(mode=0o755)
            cand = build_mount / "linux-prov2-qual-r1"
            cand.mkdir(mode=0o755)
            for sub in ("normal", "normal/release", "normal/release/build", "go-cache", "tmp", "tmp/go", "tmp/build", "go-release", "boundary-release"):
                (cand / sub).mkdir(parents=True, exist_ok=True, mode=0o755)

            base_script = f"""
set -euo pipefail
source "{QUALIFY_DIR / 'qualify.sh'}"
qualify_load_candidate_sh
mount_fstype() {{ printf 'ext4\\n'; }}
candidate_mountpoint() {{ [[ $1 == "{build_mount}" ]]; }}
protected_ancestors() {{ return 0; }}
protected_dir() {{ return 0; }}
no_nested_mount() {{ return 0; }}
mount_dev() {{
    case "$1" in
        /) printf '10\\n' ;;
        /var/lib/docker|/var/lib/containerd) printf '20\\n' ;;
        *) printf '100\\n' ;;
    esac
}}
check_candidate_build_storage "{build_mount}" "{cand}" no_nested_mount
"""
            ok = self._run_bash(base_script)
            self.assertEqual(ok.returncode, 0, ok.stderr)

            bad_nested = self._run_bash(base_script.replace("no_nested_mount() { return 0; }", "no_nested_mount() { candidate_refuse build_storage_nested_mount; }"))
            self.assertNotEqual(bad_nested.returncode, 0)
            self.assertIn("build_storage_nested_mount", bad_nested.stderr)

            coupled = self._run_bash(base_script.replace("printf '10\\n'", "printf '100\\n'"))
            self.assertNotEqual(coupled.returncode, 0)
            self.assertIn("build_storage_not_isolated", coupled.stderr)

            (cand / "unexpected_foreign_dir").mkdir()
            foreign = self._run_bash(base_script)
            self.assertNotEqual(foreign.returncode, 0)
            self.assertIn("build_storage_foreign_entry", foreign.stderr)

    def test_shell_install_dispatch_preserves_install_subcommand_and_distinguishes_parse_errors(self):
        base_flags = (
            f'--expected-driver-commit {"a" * 40} '
            f'--expected-driver-tree {"b" * 40} '
            f'--expected-descriptor-sha256 {"c" * 64}'
        )
        prefix = (
            f'set -euo pipefail; source "{QUALIFY_DIR / "qualify.sh"}"; qualify_load_candidate_sh; '
            f'CANDIDATE_FIXED_SOURCE="{ROOT}"; '
            f'require_candidate_host() {{ return 0; }}; candidate_verify_sources() {{ return 0; }}; '
        )
        accepted = self._run_bash(prefix + f'candidate_privileged_dispatch install {base_flags}')
        self.assertEqual(accepted.returncode, 1, accepted.stderr)
        out = json.loads(accepted.stdout)
        self.assertEqual(out["status"], "REFUSED")
        self.assertEqual(out["reason"], "privileged_driver_outside_fixed_source")

        missing_flag = self._run_bash(
            prefix + f'candidate_privileged_dispatch install --expected-driver-commit {"a" * 40} --expected-driver-tree {"b" * 40}'
        )
        self.assertEqual(missing_flag.returncode, 2)
        self.assertIn("--expected-descriptor-sha256", missing_flag.stderr)

        direct_missing_subcmd = subprocess.run(
            [sys.executable, "-B", str(QUALIFY_DIR / "candidate_install.py"), "--expected-driver-commit", "a" * 40],
            capture_output=True, text=True, timeout=10,
        )
        self.assertEqual(direct_missing_subcmd.returncode, 2)
        self.assertIn("invalid choice", direct_missing_subcmd.stderr)


class ReleaseDescriptorAndPinnedSourceTests(unittest.TestCase):
    def test_reviewed_source_pin_accepts_exact_tree_and_refuses_later_production_edits(self):
        with tempfile.TemporaryDirectory() as tmp:
            repo = Path(tmp) / "source"
            path = "src/bin/dw-boundary-launcher/qualification.rs"
            head, tree = _init_clean_git_repo(repo, {path: b"fn main() {}\n"})
            with mock.patch.multiple(release_descriptor, RELEASE_BASELINE_COMMIT=head, RELEASE_BASELINE_TREE=tree):
                self.assertEqual(release_descriptor.verify_release_source(repo, expected_commit=head, expected_tree=tree), (head, tree))
                with mock.patch.object(release_descriptor, "RELEASE_BASELINE_TREE", "0" * 40):
                    with self.assertRaisesRegex(release_descriptor.ReleaseError, "release_baseline_tree_mismatch"):
                        release_descriptor.verify_release_source(repo)
                target = repo / path
                os.chmod(target, 0o644)
                target.write_text("fn main() {}\n#[cfg(test)] mod injected {}\n")
                env = {"PATH": "/usr/bin:/bin", "LC_ALL": "C"}
                subprocess.run(["git", "-C", str(repo), "add", "."], check=True, env=env)
                subprocess.run(["git", "-C", str(repo), "commit", "-qm", "changed source"], check=True, env=env)
                with self.assertRaisesRegex(release_descriptor.ReleaseError, "production_source_drift"):
                    release_descriptor.verify_release_source(repo)

    def test_release_diff_allows_only_exact_operator_and_ci_paths_and_refuses_neighbors(self):
        env = {"PATH": "/usr/bin:/bin", "LC_ALL": "C"}
        with tempfile.TemporaryDirectory() as tmp:
            repo = Path(tmp) / "op_diff"
            base_files = {
                "src/bin/dw-qualify/markdown.rs": b"// base\n",
                "src/bin/dw-qualify/links.rs": b"// base\n",
                "src/bin/dw-boundary-launcher/qualification.rs": b"// base\n",
                ".github/workflows/doc-check.yml": b"name: doc\n",
                ".github/workflows/other.yml": b"name: other\n",
                "scripts/ci_rust_shards.py": b"# base\n",
                "scripts/other.py": b"# other\n",
                "Cargo.toml": b"[package]\nname='dw'\n",
                "migrations/0036.sql": b"-- sql\n",
                "boundary/cmd/cap/main.go": b"package main\n",
            }
            base_head, base_tree = _init_clean_git_repo(repo, base_files)

            def _commit_rel(rel: str, data: bytes) -> tuple[str, str]:
                p = repo / rel
                p.parent.mkdir(parents=True, exist_ok=True, mode=0o755)
                if p.exists():
                    os.chmod(p, 0o644)
                p.write_bytes(data)
                os.chmod(p, 0o444)
                subprocess.run(["git", "-C", str(repo), "add", rel], check=True, env=env)
                subprocess.run(["git", "-C", str(repo), "commit", "-qm", f"up:{rel}"], check=True, env=env)
                return (
                    subprocess.check_output(["git", "-C", str(repo), "rev-parse", "HEAD"], text=True, env=env).strip(),
                    subprocess.check_output(["git", "-C", str(repo), "rev-parse", "HEAD^{tree}"], text=True, env=env).strip(),
                )

            with mock.patch.multiple(release_descriptor, RELEASE_BASELINE_COMMIT=base_head, RELEASE_BASELINE_TREE=base_tree):
                for allowed in ("src/bin/dw-qualify/markdown.rs", ".github/workflows/doc-check.yml", "scripts/ci_rust_shards.py"):
                    head, tree = _commit_rel(allowed, b"// updated\n")
                    self.assertEqual(release_descriptor.verify_release_source(repo, expected_commit=head, expected_tree=tree), (head, tree))
                for denied, err in (
                    ("src/bin/dw-qualify/links.rs", "production_source_drift"),
                    ("src/bin/dw-qualify/extra.rs", "production_source_drift"),
                    ("src/bin/dw-boundary-launcher/qualification.rs", "production_source_drift"),
                    ("Cargo.toml", "production_source_drift"),
                    ("migrations/0036.sql", "production_source_drift"),
                    ("boundary/cmd/cap/main.go", "production_source_drift"),
                    (".github/workflows/other.yml", "unexpected_candidate_diff"),
                    ("scripts/other.py", "unexpected_candidate_diff"),
                ):
                    _commit_rel(denied, b"// drifted\n")
                    with self.assertRaisesRegex(release_descriptor.ReleaseError, err, msg=denied):
                        release_descriptor.verify_release_source(repo)
                    subprocess.run(["git", "-C", str(repo), "reset", "--hard", "-q", "HEAD~1"], check=True, env=env)

    def _populate_source_and_artifacts(self, base: Path) -> tuple[Path, Path]:
        src, art = base / "src", base / "artifacts"
        (src / "third_party" / "genai").mkdir(parents=True)
        art.mkdir(parents=True)
        for name in ("duskweave", "dw-boundary-launcher", "dw-dns-capture", "dw-dns-exporter"):
            (art / name).write_bytes(make_synthetic_elf_aarch64(name.encode()))
        for notice in ("LICENSE-APACHE", "LICENSE-MIT", "source-manifest.json"):
            (src / "third_party" / "genai" / notice).write_text(f"notice:{notice}\n", encoding="utf-8")
        return src, art

    def test_verify_release_rejects_cargo_build_rs_dirty_worktree_and_go_mod_drift(self):
        with tempfile.TemporaryDirectory() as tmp:
            repo = Path(tmp) / "repo"
            files = {
                "Cargo.toml": b"[package]\nname='x'\n",
                "Cargo.lock": b"# lock\n",
                "boundary/go.mod": b"module dw\n",
                "boundary/go.sum": b"",
            }
            head, tree = _init_clean_git_repo(repo, files)
            with self.assertRaises(release_descriptor.ReleaseError) as wrong_pin:
                release_descriptor.verify_release_source(repo, expected_commit="0" * 40, expected_tree=tree)
            self.assertEqual(str(wrong_pin.exception), "source_commit_mismatch")

            (repo / ".cargo").mkdir(mode=0o755)
            with self.assertRaises(release_descriptor.ReleaseError) as cargo_err:
                release_descriptor.verify_release_source(repo, expected_commit=head, expected_tree=tree)
            self.assertEqual(str(cargo_err.exception), "candidate_cargo_config_present")
            (repo / ".cargo").rmdir()

            (repo / "build.rs").write_text("fn main() {}\n", encoding="utf-8")
            with self.assertRaises(release_descriptor.ReleaseError) as br_err:
                release_descriptor.verify_release_source(repo, expected_commit=head, expected_tree=tree)
            self.assertEqual(str(br_err.exception), "unexpected_root_build_rs")
            (repo / "build.rs").unlink()

            os.chmod(repo / "boundary" / "go.mod", 0o644)
            (repo / "boundary" / "go.mod").write_text("module hijacked\n", encoding="utf-8")
            with self.assertRaises(release_descriptor.ReleaseError) as dirty_err:
                release_descriptor.verify_release_source(repo, expected_commit=head, expected_tree=tree)
            self.assertEqual(str(dirty_err.exception), "dirty_source_worktree")

    def test_package_and_verify_descriptor_round_trip_and_tamper_guards(self):
        with tempfile.TemporaryDirectory() as tmp:
            base = Path(tmp)
            src, art = self._populate_source_and_artifacts(base)
            pkg = base / "pkg"
            res = release_descriptor.package_release(
                package_dir=pkg, source_root=src, artifact_dir=art,
                image_ref="dw-boundary-cand:20261115r1", image_digest="sha256:" + ("a" * 64),
                build_source_commit=release_descriptor.RELEASE_BASELINE_COMMIT, build_source_tree=release_descriptor.RELEASE_BASELINE_TREE,
                driver_commit="1" * 40, driver_tree="2" * 40, synthetic_fixture=True, verify_git_baseline=False,
            )
            self.assertEqual(res["status"], "SUCCESS")
            verified = release_descriptor.verify_descriptor(pkg, res["descriptor_sha256"], allow_synthetic=True)
            self.assertEqual(len(verified["members"]), 7)
            self.assertEqual(verified["schema_revision"], "0036")
            for field, value in (("schema_version", 1), ("schema_revision", "0035"), ("release_baseline_tree", "0" * 40)):
                changed = {**verified, field: value}
                with self.assertRaises(release_descriptor.ReleaseError):
                    release_descriptor.parse_descriptor_bytes(json.dumps(changed).encode())
            with self.assertRaises(release_descriptor.ReleaseError):
                release_descriptor.verify_descriptor(pkg, res["descriptor_sha256"], allow_synthetic=False)
            os.chmod(pkg / "duskweave", 0o755)
            (pkg / "duskweave").write_bytes(make_synthetic_elf_aarch64(b"tampered"))
            os.chmod(pkg / "duskweave", 0o555)
            with self.assertRaises(release_descriptor.ReleaseError):
                release_descriptor.verify_descriptor(pkg, res["descriptor_sha256"], allow_synthetic=True)


class CandidateInstallSafetyTests(unittest.TestCase):
    def _setup_install_env(self, base: Path):
        src, art = base / "src", base / "artifacts"
        (src / "third_party" / "genai").mkdir(parents=True)
        art.mkdir(parents=True)
        for name in ("duskweave", "dw-boundary-launcher", "dw-dns-capture", "dw-dns-exporter"):
            (art / name).write_bytes(make_synthetic_elf_aarch64(name.encode()))
        for notice in ("LICENSE-APACHE", "LICENSE-MIT", "source-manifest.json"):
            (src / "third_party" / "genai" / notice).write_text(f"n:{notice}\n", encoding="utf-8")
        pkg = base / "pkg"
        res = release_descriptor.package_release(
            package_dir=pkg, source_root=src, artifact_dir=art,
            image_ref="dw-boundary-cand:20261115r1", image_digest="sha256:" + ("c" * 64),
            build_source_commit=release_descriptor.RELEASE_BASELINE_COMMIT, build_source_tree=release_descriptor.RELEASE_BASELINE_TREE,
            driver_commit="1" * 40, driver_tree="2" * 40, synthetic_fixture=True, verify_git_baseline=False,
        )
        install_bin = base / "bin"
        install_bin.mkdir(mode=0o755)
        old_hashes = {}
        for name in ("duskweave", "dw-boundary-launcher", "dw-dns-capture", "dw-dns-exporter"):
            old_b = make_synthetic_elf_aarch64(f"old-{name}".encode())
            (install_bin / name).write_bytes(old_b)
            os.chmod(install_bin / name, 0o555)
            old_hashes[name] = hashlib.sha256(old_b).hexdigest()
        mfst = base / "deployment.json"
        mfst.write_bytes(b'{"prior":true}\n')
        os.chmod(mfst, 0o444)
        return pkg, res["descriptor_sha256"], install_bin, mfst, old_hashes

    def _run(self, base: Path, pkg: Path, dsha: str, install_bin: Path, mfst: Path, backup: Path, rcpt: Path, **kw):
        uid = os.geteuid()
        img_shas = {w: hashlib.sha256((pkg / w).read_bytes()).hexdigest() for w in ("dw-dns-capture", "dw-dns-exporter")}
        def h_obs():
            bins = candidate_host.observe_installed_binaries(install_bin, stop_at=base, expected_uids=(uid,), require_all=True)
            return {"core_uid": uid, "core_gid": os.getegid(), "boot_id": "11111111-1111-4111-8111-111111111111",
                    "installed_hashes": {k: v["sha256"] for k, v in bins.items()}, "binaries": bins}
        return candidate_install.run_install(
            expected_driver_commit="1" * 40, expected_driver_tree="2" * 40, expected_descriptor_sha256=dsha,
            package_dir=pkg, install_bin_dir=install_bin, backup_dir=backup, manifest_path=mfst, receipt_path=rcpt,
            expected_uids=(uid,), stop_at=base, allow_synthetic=True,
            driver_verifier=lambda *_a, **_k: None, host_observer=h_obs,
            image_extractor=lambda _ref: {"image_id": "sha256:" + ("c" * 64), "os": "linux", "architecture": "arm64", "workers": img_shas},
            quiescence_checker=kw.get("quiescence_checker", lambda: {"quiescent": True}),
            raw_env=kw.get("raw_env", {"PATH": "/usr/bin"}),
            replacer=kw.get("replacer", os.replace),
        )

    def test_refuses_missing_old_member_and_staging_quiescence_loss(self):
        with tempfile.TemporaryDirectory() as tmp:
            base = Path(tmp)
            os.chmod(base, 0o755)
            pkg, dsha, install_bin, mfst, old_hashes = self._setup_install_env(base)

            q_calls = 0

            def lose_q():
                nonlocal q_calls
                q_calls += 1
                if q_calls == 2:
                    raise candidate_install.HostObservationError("REFUSED", "active_runtime_processes_present")

            r_ref = self._run(base, pkg, dsha, install_bin, mfst, base / "bak1", base / "rcpt1.json", quiescence_checker=lose_q)
            self.assertEqual(r_ref["status"], "REFUSED")
            self.assertEqual(r_ref["replaced"], [])
            for name, oh in old_hashes.items():
                self.assertEqual(hashlib.sha256((install_bin / name).read_bytes()).hexdigest(), oh)
            self.assertEqual(list(install_bin.glob("*.stage*")), [])
            self.assertTrue((base / "bak1" / "backup-manifest.json").is_file())
            self.assertTrue((base / "bak1" / "deployment.json").is_file())

            (install_bin / "dw-dns-exporter").unlink()
            with self.assertRaises(candidate_install.InstallError) as miss:
                self._run(base, pkg, dsha, install_bin, mfst, base / "bak2", base / "rcpt2.json")
            self.assertEqual(miss.exception.reason, "installed_binary_missing:dw-dns-exporter")

    def test_partial_install_is_incomplete_without_rollback_and_receipt_fail_is_unknown(self):
        with tempfile.TemporaryDirectory() as tmp:
            base = Path(tmp)
            os.chmod(base, 0o755)
            pkg, dsha, install_bin, mfst, old_hashes = self._setup_install_env(base)

            replaces = 0

            def flaky_replace(s: Path, d: Path):
                nonlocal replaces
                replaces += 1
                if replaces == 2:
                    raise OSError("disk_full")
                os.replace(s, d)

            r_inc = self._run(base, pkg, dsha, install_bin, mfst, base / "bak3", base / "rcpt3.json", replacer=flaky_replace)
            self.assertEqual(r_inc["status"], "INCOMPLETE")
            self.assertEqual(r_inc["replaced"], ["duskweave"])
            self.assertEqual(hashlib.sha256((base / "bak3" / "duskweave").read_bytes()).hexdigest(), old_hashes["duskweave"])
            self.assertEqual((install_bin / "duskweave").read_bytes(), (pkg / "duskweave").read_bytes())

            base4 = base / "b4"
            base4.mkdir(mode=0o755)
            os.chmod(base4, 0o755)
            pkg4, dsha4, bin4, mfst4, old_h4 = self._setup_install_env(base4)
            rcpt4_dir = base4 / "rcpt4_dir"
            rcpt4_dir.mkdir(mode=0o755)
            os.chmod(rcpt4_dir, 0o755)
            rcpt4 = rcpt4_dir / "rcpt4.json"
            real_fsync_dir = candidate_files.fsync_dir
            replace_log4: list[str] = []

            def fail_rcpt_dir_sync(directory: Path, fsync_fn=os.fsync) -> None:
                if directory == rcpt4_dir:
                    def _err(_fd: int) -> None:
                        raise OSError(errno.EIO, "/secret/rcpt/dir")
                    return real_fsync_dir(directory, fsync_fn=_err)
                return real_fsync_dir(directory, fsync_fn=fsync_fn)

            def track_replace4(s: Path, d: Path) -> None:
                replace_log4.append(d.name)
                os.replace(s, d)

            with mock.patch.object(candidate_files, "fsync_dir", side_effect=fail_rcpt_dir_sync), \
                 mock.patch.object(candidate_install, "fsync_dir", side_effect=fail_rcpt_dir_sync):
                r_unk = self._run(base4, pkg4, dsha4, bin4, mfst4, base4 / "bak4", rcpt4, replacer=track_replace4)
            self.assertEqual(r_unk["status"], "UNKNOWN")
            self.assertEqual(r_unk["replaced"], list(candidate_host.INSTALL_BINARIES))
            self.assertEqual(replace_log4, list(candidate_host.INSTALL_BINARIES))
            self.assertEqual(r_unk["reason"], f"install_receipt_persist_failed:custody_dir_fsync_failed:{errno.EIO}")
            self.assertNotIn("/secret/rcpt/dir", r_unk["reason"])
            self.assertEqual((bin4 / "duskweave").read_bytes(), (pkg4 / "duskweave").read_bytes())

            base5 = base / "b5"
            base5.mkdir(mode=0o755)
            os.chmod(base5, 0o755)
            pkg5, dsha5, bin5, mfst5, old_h5 = self._setup_install_env(base5)
            replace_order5: list[str] = []

            def fail_second_bin_sync(directory: Path, fsync_fn=os.fsync) -> None:
                if directory == bin5 and len(replace_order5) == 2:
                    def _err(_fd: int) -> None:
                        raise OSError(errno.EIO, "/secret/bin/dir")
                    return real_fsync_dir(directory, fsync_fn=_err)
                return real_fsync_dir(directory, fsync_fn=fsync_fn)

            def track_replace5(s: Path, d: Path) -> None:
                replace_order5.append(d.name)
                os.replace(s, d)

            with mock.patch.object(candidate_files, "fsync_dir", side_effect=fail_second_bin_sync), \
                 mock.patch.object(candidate_install, "fsync_dir", side_effect=fail_second_bin_sync):
                r_sync = self._run(base5, pkg5, dsha5, bin5, mfst5, base5 / "bak5", base5 / "rcpt5.json", replacer=track_replace5)
            self.assertEqual(r_sync["status"], "INCOMPLETE")
            self.assertEqual(r_sync["replaced"], ["duskweave", "dw-boundary-launcher"])
            self.assertEqual(replace_order5, ["duskweave", "dw-boundary-launcher"])
            self.assertEqual(r_sync["reason"], f"custody_dir_fsync_failed:{errno.EIO}")
            self.assertEqual((bin5 / "duskweave").read_bytes(), (pkg5 / "duskweave").read_bytes())
            self.assertEqual((bin5 / "dw-boundary-launcher").read_bytes(), (pkg5 / "dw-boundary-launcher").read_bytes())
            self.assertEqual(hashlib.sha256((bin5 / "dw-dns-capture").read_bytes()).hexdigest(), old_h5["dw-dns-capture"])
            self.assertEqual(hashlib.sha256((base5 / "bak5" / "dw-boundary-launcher").read_bytes()).hexdigest(), old_h5["dw-boundary-launcher"])
            self.assertTrue((base5 / "rcpt5.json").is_file())


if __name__ == "__main__":
    unittest.main()
