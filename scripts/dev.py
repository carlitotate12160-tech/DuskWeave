#!/usr/bin/env python3
"""Single developer entrypoint for DuskWeave local workflows.

Owns the process-scoped artifact convention: each invocation is the
owning process and passes CARGO_TARGET_DIR (plus CARGO_LLVM_COV_TARGET_DIR
for coverage) to its children only. The build root derives from the
worktree name and pinned toolchain so parallel worktrees never share a
target directory:

    <repo-parent>/<repo-name>-build/rust-<channel>/
        normal/               ordinary build/test artifacts
        fixture-supervisor/   opt-in service-check/supervisor binary
        coverage/<packet>/    instrumented artifacts, one owner per run
        reports/<packet>/     cache qualification reports

A .env file is loaded for child processes when present (existing
environment wins); the production binary never reads dotenv. Variable
values are never printed, only presence. Failures use closed categories.

Usage: python scripts/dev.py <command> [options] [-- <tool arguments>]
Commands: env-check, build, test, coverage, supervisor, db, check.
"""
import argparse
import json
import os
from pathlib import Path
import re
import subprocess
import sys
import tomllib

ROOT = Path(__file__).resolve().parents[1]
BOOTSTRAP = ROOT / "tests" / "support" / "m1_native_bootstrap.py"
ENV_FILE = ROOT / ".env"
LLVM_COV_PINNED = "0.9.1"
TEST_VARIABLES = (
    "DW_DATABASE_CONFIG_MODE", "DW_DATABASE_URL", "DW_DATABASE_URL_FILE",
    "DW_TEST_DATABASE_URL", "DW_TEST_ADMIN_DATABASE_URL",
    "DW_TEST_FIXTURE_SUPERVISOR",
)
PACKET_ID = re.compile(r"[a-z0-9][a-z0-9-]{1,40}")


def toolchain_channel():
    data = tomllib.loads((ROOT / "rust-toolchain.toml").read_text(encoding="utf-8"))
    return data["toolchain"]["channel"]


def build_root():
    return ROOT.parent / f"{ROOT.name}-build" / f"rust-{toolchain_channel()}"


def supervisor_binary():
    name = "dw-fixture-supervisor.exe" if os.name == "nt" else "dw-fixture-supervisor"
    return build_root() / "fixture-supervisor" / "debug" / name


def child_env(target=None):
    """Environment for children: caller env, then .env defaults, then paths."""
    env = dict(os.environ)
    if ENV_FILE.is_file():
        for line in ENV_FILE.read_text(encoding="utf-8").splitlines():
            line = line.strip()
            if not line or line.startswith("#") or "=" not in line:
                continue
            key, _, value = line.partition("=")
            env.setdefault(key.strip(), value.strip())
    if target is not None:
        env["CARGO_TARGET_DIR"] = str(target)
    return env


def cargo(channel=True):
    return ["cargo", f"+{toolchain_channel()}"] if channel else ["cargo"]


def run(arguments, env=None):
    try:
        return subprocess.run(arguments, env=env or child_env()).returncode
    except FileNotFoundError:
        print(f"dev_tool_unavailable: {arguments[0]}", file=sys.stderr)
        return 2


def command_presence(name):
    try:
        return subprocess.run([name, "--version"], capture_output=True,
                              timeout=15).returncode == 0
    except (OSError, subprocess.SubprocessError):
        return False


def env_check(_options):
    channel = toolchain_channel()
    report = {
        "python": f"{sys.version_info.major}.{sys.version_info.minor}",
        "python_expected": "3.12",
        "rust_channel": channel,
        "cargo": command_presence("cargo"),
        "docker": command_presence("docker"),
        "build_root": str(build_root()),
        "env_file_present": ENV_FILE.is_file(),
        "supervisor_binary_present": supervisor_binary().is_file(),
        "variables": {name: ("set" if os.environ.get(name) else "absent")
                      for name in TEST_VARIABLES},
    }
    print(json.dumps(report, indent=2))
    return 0


def build(options):
    return run([*cargo(), "build", "--locked", *options.args],
               env=child_env(build_root() / "normal"))


def test(options):
    env = child_env(build_root() / "normal")
    env.setdefault("DW_TEST_FIXTURE_SUPERVISOR", str(supervisor_binary()))
    return run([*cargo(), "test", "--locked", *options.args], env=env)


def coverage(options):
    if not PACKET_ID.fullmatch(options.packet):
        print("dev_packet_id_refused", file=sys.stderr)
        return 2
    probe = subprocess.run(["cargo", "llvm-cov", "--version"],
                           capture_output=True, text=True)
    match = re.search(r"(\d+\.\d+\.\d+)", probe.stdout or "")
    if probe.returncode or not match or match.group(1) != LLVM_COV_PINNED:
        print("dev_coverage_tool_unqualified", file=sys.stderr)
        return 2
    target = build_root() / "coverage" / options.packet
    env = child_env(target)
    env["CARGO_LLVM_COV_TARGET_DIR"] = str(target)
    env.setdefault("DW_TEST_FIXTURE_SUPERVISOR", str(supervisor_binary()))
    print(json.dumps({"coverage_target": str(target),
                      "report_dir": str(build_root() / "reports" / options.packet)}))
    return run([*cargo(), "llvm-cov", "--locked", *options.args], env=env)


def supervisor(options):
    result = run([*cargo(), "build", "--locked", "--features", "fixture-supervisor",
                  "--bin", "dw-fixture-supervisor", *options.args],
                 env=child_env(build_root() / "fixture-supervisor"))
    if result == 0:
        print(json.dumps({"supervisor": str(supervisor_binary())}))
    return result


def db(options):
    env = child_env()
    if not env.get("DW_TEST_FIXTURE_SUPERVISOR"):
        if not supervisor_binary().is_file():
            print("native_supervisor_unavailable", file=sys.stderr)
            return 2
        env["DW_TEST_FIXTURE_SUPERVISOR"] = str(supervisor_binary())
    return run([sys.executable, str(BOOTSTRAP), *options.args], env=env)


def check(options):
    checks = ["check_links.py", "check_structure.py", "check_dependencies.py"]
    failed = 0
    for name in checks:
        code = run([sys.executable, str(ROOT / "scripts" / name)])
        print(f"dev_check:{name}={'pass' if code == 0 else 'fail'}")
        failed += code != 0
    if options.rust:
        env = child_env(build_root() / "normal")
        for args in (["fmt", "--check"], ["clippy", "--locked", "--", "-D", "warnings"]):
            code = run([*cargo(), *args], env=env)
            print(f"dev_check:cargo {' '.join(args)}={'pass' if code == 0 else 'fail'}")
            failed += code != 0
    else:
        print("dev_check:complexity=skipped(no analyzer bound)")
    return 1 if failed else 0


def main():
    parser = argparse.ArgumentParser(prog="dev.py", description=__doc__)
    sub = parser.add_subparsers(dest="command", required=True)
    for name, help_text in (
        ("env-check", "report toolchain, docker and variable presence"),
        ("build", "cargo build --locked into the normal artifact path"),
        ("test", "cargo test --locked into the normal artifact path"),
        ("coverage", "pinned llvm-cov into coverage/<packet>; --packet required"),
        ("supervisor", "build the opt-in dw-fixture-supervisor helper"),
        ("db", "run tests/support/m1_native_bootstrap.py with supervisor path"),
        ("check", "local static checks; --rust adds fmt/clippy"),
    ):
        p = sub.add_parser(name, help=help_text)
        p.add_argument("args", nargs=argparse.REMAINDER,
                       help="arguments after -- pass through to the tool")
        if name == "coverage":
            p.add_argument("--packet", required=True)
        if name == "check":
            p.add_argument("--rust", action="store_true")
    options = parser.parse_args()
    if options.args[:1] == ["--"]:
        options.args = options.args[1:]
    handler = {"env-check": env_check, "build": build, "test": test,
               "coverage": coverage, "supervisor": supervisor, "db": db,
               "check": check}[options.command]
    return handler(options)


if __name__ == "__main__":
    sys.exit(main())
