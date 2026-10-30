"""Own one fresh native fixture and keep credentials in this process and children."""
import argparse
import json
import math
import os
from pathlib import Path
import re
import secrets
import signal
import subprocess
import sys
import threading
import time
from urllib.parse import quote

IMAGE = "postgres@sha256:d74eeac9a635390a49bc21bd49fccd973de707e2a53a76ac49b552b8712ec46f"
NAMESPACE = "dw_native_"
KINDS = ("fixture", "live")
SCHEMA_REVISIONS = ("0020", "0021", "0022")
ROOT = Path(__file__).resolve().parents[2]
SUPERVISOR_VARIABLE = "DW_TEST_FIXTURE_SUPERVISOR"
# Development-tool policy ceiling for legacy no-ceiling callers; selected
# policy, not measured fit and not a product timer.
LEGACY_CEILING_MS = 3_600_000
SUPERVISOR_GRACE_SECONDS = 120
RESULT_STATUSES = ("completed", "action_failed", "harness_timeout")
CLOSED_CATEGORIES = frozenset((
    "native_service_unqualified", "native_supervisor_usage_refused",
    "native_supervisor_request_refused", "native_report_collision",
    "native_action_spawn_failed", "native_report_write_failed",
    "native_supervisor_unavailable", "native_supervisor_protocol_refused",
    "native_supervisor_result_inconsistent", "native_supervisor_unresponsive",
))


def identifier(value):
    if not re.fullmatch(r"[a-z][a-z0-9_]{0,62}", value):
        raise RuntimeError("native_identifier_refused")
    return value


def database_name(kind, explicit=None):
    """Validate or generate a unique database name in the dw_native_ namespace."""
    name = explicit if explicit is not None else f"{NAMESPACE}{kind}_{secrets.token_hex(8)}"
    if len(name) <= len(NAMESPACE) or not name.startswith(NAMESPACE):
        raise RuntimeError("native_identifier_refused")
    return identifier(name)


def private_run(args, data=None):
    result = subprocess.run(args, input=data, capture_output=True, text=True, timeout=60)
    if result.returncode:
        raise RuntimeError("native_provisioning_failed")
    return result.stdout


def supervisor_path(environment):
    """The development supervisor is selected by absolute path only."""
    value = environment.get(SUPERVISOR_VARIABLE, "")
    if not value or not Path(value).is_absolute():
        raise RuntimeError("native_supervisor_unavailable")
    return Path(value)


def protocol_category(stdout):
    """Validate one closed refusal token; never echo helper diagnostics."""
    try:
        category = json.loads(stdout.strip().splitlines()[-1]).get("category")
    except (ValueError, IndexError, AttributeError):
        category = None
    return category if category in CLOSED_CATEGORIES else "native_supervisor_protocol_refused"


def supervisor_request(arguments, environment):
    """One closed helper invocation; failures map to closed categories."""
    path = supervisor_path(environment)
    try:
        result = subprocess.run([str(path), *arguments], capture_output=True, text=True, timeout=60)
    except (OSError, subprocess.SubprocessError):
        raise RuntimeError("native_supervisor_unavailable") from None
    if result.returncode:
        raise RuntimeError(protocol_category(result.stdout))
    return result.stdout


def service_qualified(container, ci=False, version_prefix=None, version_exact=None):
    """Delegate the fixed Docker/PostgreSQL service qualification to the helper."""
    arguments = ["service-check", "--container", container]
    if ci:
        arguments.append("--ci")
    if version_prefix is not None:
        arguments += ["--version-prefix", version_prefix]
    else:
        arguments += ["--version-exact", version_exact]
    return supervisor_request(arguments, os.environ)


def relay_progress(stream):
    """Forward only closed progress lines from the helper's private channel."""
    for line in stream:
        try:
            record = json.loads(line)
        except ValueError:
            continue
        if set(record) == {"action", "status", "elapsed_seconds"} and record.get("status") == "running":
            print(json.dumps(record), flush=True)


def stop_supervisor(process):
    """Last-resort bound when the helper itself fails to return; never a product guard."""
    if sys.platform == "win32":
        subprocess.run(["taskkill", "/PID", str(process.pid), "/T", "/F"],
                       capture_output=True, timeout=10)
        return
    try:
        os.killpg(process.pid, signal.SIGKILL)
    except OSError:
        process.kill()


def validate_publication(protocol_line, report, label):
    """Validate the closed result line against the published metadata."""
    try:
        result = json.loads(protocol_line.strip().splitlines()[-1])
    except (ValueError, IndexError):
        raise RuntimeError("native_supervisor_protocol_refused") from None
    if not isinstance(result, dict):
        raise RuntimeError("native_supervisor_protocol_refused")
    if result.get("ok") is False and result.get("category") in CLOSED_CATEGORIES:
        raise RuntimeError(result["category"])
    if not isinstance(result, dict) or result.get("ok") is not True:
        raise RuntimeError("native_supervisor_protocol_refused")
    if result.get("status") not in RESULT_STATUSES or type(result.get("exit")) is not int:
        raise RuntimeError("native_supervisor_protocol_refused")
    try:
        metadata = json.loads((report / f"{label}.result.json").read_text(encoding="utf-8"))
    except (OSError, ValueError):
        raise RuntimeError("native_supervisor_result_inconsistent") from None
    fields = ("status", "exit", "child_exit", "capture", "cleanup", "omissions", "timing_origin")
    if not isinstance(metadata, dict) or any(metadata.get(k) != result.get(k) for k in fields):
        raise RuntimeError("native_supervisor_result_inconsistent")
    validate_supervised_result(result)
    validate_supervised_result(metadata)
    return result


def validate_supervised_result(result):
    """Reject contradictory success and preserve supervised failure end to end."""
    status, code, child = (result.get(k) for k in ("status", "exit", "child_exit"))
    capture, cleanup = result.get("capture"), result.get("cleanup")
    if (status not in RESULT_STATUSES or type(code) is not int
            or (child is not None and type(child) is not int)
            or capture not in ("complete", "incomplete") or cleanup not in ("completed", "unknown")
            or result.get("timing_origin") != "request_admission"):
        raise RuntimeError("native_supervisor_protocol_refused")
    omissions = result.get("omissions")
    if (not isinstance(omissions, list) or any(type(x) is not str or x not in ("stdout", "stderr") for x in omissions)
            or len(set(omissions)) != len(omissions) or (capture == "complete") != (omissions == [])):
        raise RuntimeError("native_supervisor_protocol_refused")
    success = child == 0 and capture == "complete" and cleanup == "completed"
    if status == "completed" and (code != 0 or not success):
        raise RuntimeError("native_supervisor_result_inconsistent")
    if status == "action_failed" and (code == 0 or (child not in (None, 0) and code != child)
                                      or (child in (None, 0) and code != 1)):
        raise RuntimeError("native_supervisor_result_inconsistent")
    if status == "harness_timeout" and code != 124:
        raise RuntimeError("native_supervisor_result_inconsistent")


def validated_role_prefix(value):
    if not isinstance(value, str) or not value.startswith("dw_"):
        raise RuntimeError("native_identifier_refused")
    identifier(value)
    identifier(f"{value}_{'0' * 12}_terrainsetonly")
    return value


def validate_action(label, args, report):
    if not isinstance(label, str) or not re.fullmatch(r"[a-z0-9_-]+", label) or not isinstance(args, list) or not args:
        raise RuntimeError("native_action_refused")
    if any(not isinstance(arg, str) or len(arg.encode()) > 8192 for arg in args):
        raise RuntimeError("native_action_refused")
    if (report / f"{label}.log").exists() or (report / f"{label}.result.json").exists():
        raise RuntimeError("native_report_collision")


class Fixture:
    def __init__(self, container, kind, ci=False, database=None, schema_revision="0021", role_prefix="dw_nf60"):
        if schema_revision not in SCHEMA_REVISIONS or (kind == "live" and schema_revision != "0021"):
            raise RuntimeError("native_schema_revision_refused")
        self.schema_revision = schema_revision
        if container not in ("dwpg-r3", "dwpg"):
            raise RuntimeError("native_service_unqualified")
        if ci and container != "dwpg":
            raise RuntimeError("native_service_unqualified")
        if kind not in KINDS:
            raise RuntimeError("native_identifier_refused")
        self.role_prefix = validated_role_prefix(role_prefix)
        self.container = container
        # Container names allow the existing owned dash spelling, never SQL use.
        self.database = database_name(kind, database)
        self.env = dict(os.environ)
        self.env.pop("DEEPSEEK_API_KEY", None)
        self.env["PYTHONDONTWRITEBYTECODE"] = "1"
        self.secrets = []
        # The helper owns inspect/port/version qualification; null port
        # metadata and unsafe bindings are refused there as one closed category.
        service_qualified(container, ci=ci, version_prefix="17.")
        if self.sql(f"SELECT 1 FROM pg_database WHERE datname='{self.database}'", database="postgres").strip():
            raise RuntimeError("native_database_collision")
        self.role_suffix = secrets.token_hex(6)
        suffix = self.role_suffix
        owners = ["admin", "core", "broker", "cognition", "history", "terrain"]
        if kind == "fixture":
            owners.append("otherbroker")
        self.roles = {name: identifier(f"{self.role_prefix}_{suffix}_{name}") for name in owners}
        self.migration_actor = "postgres" if ci else self.roles["admin"]
        for role in self.roles.values():
            if self.sql(f"SELECT 1 FROM pg_roles WHERE rolname='{role}'", database="postgres").strip():
                raise RuntimeError("native_login_collision")
        passwords = {}
        for name, role in self.roles.items():
            password = secrets.token_urlsafe(36)
            passwords[name] = password
            self.secrets.append(password)
            private_run(["docker", "exec", "-i", container, "createuser", "-U", "postgres",
                         "--no-superuser", "--no-createdb", "--no-createrole", "--no-replication",
                         "--no-bypassrls", "--pwprompt", role], password + "\n" + password + "\n")
        self.sql(f"CREATE DATABASE {self.database} OWNER {self.roles['admin']}", database="postgres")
        for group in ("dw_runtime", "dw_m1_broker", "dw_m1_cognition", "dw_m1_trajectory", "dw_m1_terrain"):
            self.sql(f"DO $$ BEGIN IF NOT EXISTS(SELECT 1 FROM pg_roles WHERE rolname='{group}') "
                     f"THEN CREATE ROLE {group} NOLOGIN NOSUPERUSER NOBYPASSRLS; END IF; END $$")
        self.migrate(initial=True)
        for name, groups in (("core", "dw_runtime"), ("broker", "dw_m1_broker"),
                             ("otherbroker", "dw_m1_broker"),
                             ("cognition", "dw_m1_cognition"), ("history", "dw_m1_trajectory"),
                             ("terrain", "dw_m1_terrain")):
            if name not in self.roles:
                continue
            self.sql(f"GRANT {groups} TO {self.roles[name]}")
        variables = {"admin": "DW_TEST_ADMIN_DATABASE_URL", "core": "DW_TEST_DATABASE_URL",
                     "broker": "DW_INFERENCE_BROKER_DATABASE_URL", "cognition": "DW_COGNITION_DATABASE_URL",
                     "history": "DW_TRAJECTORY_DATABASE_URL", "terrain": "DW_TERRAIN_DATABASE_URL",
                     "otherbroker": "DW_NATIVE_OTHER_BROKER_DATABASE_URL"}
        for owner, variable in variables.items():
            if owner not in self.roles:
                continue
            self.env[variable] = (f"postgresql://{self.roles[owner]}:{quote(passwords[owner], safe='')}"
                                  f"@127.0.0.1:5432/{self.database}?sslmode=disable")
        if ci:
            # CI's isolated cluster retains its explicitly supplied administrative
            # DSN for the legacy suite; local native tests never use legacy setup.
            self.env["DW_TEST_ADMIN_DATABASE_URL"] = os.environ["DW_TEST_ADMIN_DATABASE_URL"]
        self.env["DW_DATABASE_CONFIG_MODE"] = "env-local"
        self.env["DW_DATABASE_URL"] = self.env["DW_TEST_DATABASE_URL"]
        self.env["DW_NATIVE_FIXTURE_DATABASE"] = self.database
        for variable in variables.values():
            if variable in self.env:
                self.secrets.append(self.env[variable])
        if kind == "fixture":
            self.negative_owners()

    def negative_owners(self):
        """Fresh hostile-principal controls; no retained principal is widened."""
        suffix = self.role_suffix
        names = {kind: identifier(f"{self.role_prefix}_{suffix}_{kind}") for kind in (
            "sibling", "foreign", "setonly", "column", "corebad", "terraincolumn", "terrainsetonly", "prepare")}
        for kind, role in names.items():
            if self.sql(f"SELECT 1 FROM pg_roles WHERE rolname='{role}'", database="postgres").strip():
                raise RuntimeError("native_login_collision")
            password = secrets.token_urlsafe(36)
            self.secrets.append(password)
            private_run(["docker", "exec", "-i", self.container, "createuser", "-U", "postgres",
                         "--no-superuser", "--no-createdb", "--no-createrole", "--no-replication",
                         "--no-bypassrls", "--pwprompt", role], password + "\n" + password + "\n")
            groups = {"sibling": "dw_m1_broker,dw_m1_cognition", "column": "dw_m1_broker", "corebad": "dw_runtime",
                      "terraincolumn": "dw_m1_terrain", "terrainsetonly": "dw_m1_terrain",
                      "prepare": "dw_runtime,dw_m1_broker"}.get(kind, "dw_m1_cognition")
            self.sql(f"GRANT {groups} TO {role}")
            variable = f"DW_NATIVE_{kind.upper()}_DATABASE_URL"
            self.env[variable] = f"postgresql://{role}:{quote(password, safe='')}@127.0.0.1:5432/{self.database}?sslmode=disable"
            self.secrets.append(self.env[variable])
        self.sql(f"GRANT USAGE ON SCHEMA mission TO {names['foreign']}; GRANT INSERT ON mission.missions TO {names['foreign']}")
        self.sql(f"GRANT {names['foreign']} TO {names['setonly']} WITH INHERIT FALSE, SET TRUE")
        self.sql(f"GRANT UPDATE(ends_at) ON mission.missions TO {names['column']}; GRANT INSERT ON cognition.reasoning_decisions,trajectory.reasoning_history TO {names['corebad']}")
        self.sql(f"GRANT UPDATE(valid_until) ON terrain.current_claims TO {names['terraincolumn']}; "
                 f"GRANT {names['terraincolumn']} TO {names['terrainsetonly']} WITH INHERIT FALSE, SET TRUE")

    def sql(self, statement, database=None):
        return private_run(["docker", "exec", "-i", self.container, "psql", "-X", "-U", "postgres",
                            "-d", database or self.database, "-v", "ON_ERROR_STOP=1", "-At"], statement)

    def migrate(self, initial=False):
        numbers = (1, 2, 3, 5, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22) if initial else (12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22)
        for number in numbers:
            if number > int(self.schema_revision):
                continue
            source = next((ROOT / "migrations").glob(f"{number:04d}_*.sql")).read_text(encoding="utf-8")
            if not initial:
                source = re.sub(r"CREATE SCHEMA (?!IF NOT EXISTS )", "CREATE SCHEMA IF NOT EXISTS ", source)
                source = re.sub(r"CREATE TABLE (?!IF NOT EXISTS )", "CREATE TABLE IF NOT EXISTS ", source)
                source = re.sub(r"CREATE UNIQUE INDEX (?!IF NOT EXISTS )", "CREATE UNIQUE INDEX IF NOT EXISTS ", source)
                source = source.replace("CREATE FUNCTION ", "CREATE OR REPLACE FUNCTION ")
                source = source.replace("CREATE VIEW ", "CREATE OR REPLACE VIEW ")
                source = re.sub(r"CREATE TRIGGER\s+(\w+)\s+(.*?);", lambda m:
                                "DO $refresh$ BEGIN IF NOT EXISTS(SELECT 1 FROM pg_trigger WHERE tgname='" + m[1] +
                                "') THEN " + m[0] + " END IF; END $refresh$;", source, flags=re.S)
            try:
                self.sql(f"SET ROLE {self.migration_actor};\n" + source)
            except RuntimeError:
                raise RuntimeError(f"native_migration_{number:04d}_failed") from None

    def run(self, label, args, report, max_duration_seconds=None):
        """Delegate action execution to the development supervisor selected by
        DW_TEST_FIXTURE_SUPERVISOR; there is no Python supervision fallback."""
        validate_action(label, args, report)
        supervised = max_duration_seconds is not None
        if supervised and (not math.isfinite(max_duration_seconds) or max_duration_seconds <= 0):
            raise RuntimeError("native_action_refused")
        budget_ms = LEGACY_CEILING_MS if not supervised else int(max_duration_seconds * 1000)
        result = self.supervised_result(label, args, report, budget_ms)
        print(json.dumps({"action": label, "exit": result["exit"]}), flush=True)
        tail = (report / f"{label}.log").read_text(encoding="utf-8", errors="replace")
        print("\n".join(tail.splitlines()[-35:]), flush=True)
        return result["exit"]

    def supervised_result(self, label, args, report, budget_ms):
        request = json.dumps({"version": 1, "argv": args, "cwd": str(ROOT), "label": label,
                              "report_root": str(report), "action_budget_ms": budget_ms,
                              "redactions": self.secrets})
        protocol = self.invoke_supervisor(request, budget_ms)
        return validate_publication(protocol, report, label)

    def invoke_supervisor(self, request, budget_ms):
        path = supervisor_path(self.env)
        options = {} if sys.platform == "win32" else {"start_new_session": True}
        child = subprocess.Popen([str(path), "run-action"], cwd=ROOT, env=self.env,
                                 stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                                 stderr=subprocess.PIPE, text=True, **options)
        threading.Thread(target=relay_progress, args=(child.stderr,), daemon=True).start()
        collected = []
        reader = threading.Thread(target=lambda: collected.append(child.stdout.read()), daemon=True)
        reader.start()
        try:
            child.stdin.write(request)
            child.stdin.close()
        except (OSError, ValueError):
            pass
        self.await_supervisor(child, reader, budget_ms / 1000 + SUPERVISOR_GRACE_SECONDS)
        return collected[0]

    def await_supervisor(self, child, reader, limit_seconds):
        deadline = time.monotonic() + limit_seconds
        reader.join(max(0.0, deadline - time.monotonic()))
        if reader.is_alive():
            stop_supervisor(child)
            raise RuntimeError("native_supervisor_unresponsive")
        try:
            child.wait(timeout=10)
        except subprocess.TimeoutExpired:
            stop_supervisor(child)
            raise RuntimeError("native_supervisor_unresponsive") from None


def run_session(fixture, source, report):
    for line in source:
        try:
            action = json.loads(line)
            if not isinstance(action, dict):
                raise ValueError
            if action.get("exit") is True:
                return 0
            if not action.get("refresh"):
                validate_action(action["label"], action["args"], report)
        except (ValueError, KeyError, TypeError, RuntimeError) as error:
            category = "native_report_collision" if str(error) == "native_report_collision" else "native_action_refused"
            print(json.dumps({"ok": False, "exit": 1, "category": category}), flush=True)
            continue
        # Actual migration/supervisor failures stay fatal: custody may be uncertain.
        if action.get("refresh"):
            fixture.migrate()
            print('{"migration_refresh":"complete"}', flush=True)
        else:
            fixture.run(action["label"], action["args"], report)
    return 0


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--container", default="dwpg-r3")
    parser.add_argument("--database", choices=KINDS, default="fixture")
    parser.add_argument("--database-name", default=None)
    parser.add_argument("--schema-revision", choices=SCHEMA_REVISIONS, default="0021")
    parser.add_argument("--report", type=Path, required=True)
    parser.add_argument("--role-prefix", default="dw_nf60")
    parser.add_argument("--ci", action="store_true")
    parser.add_argument("--session", action="store_true")
    parser.add_argument("command", nargs=argparse.REMAINDER)
    options = parser.parse_args()
    if options.database == "live" and options.schema_revision != "0021":
        raise RuntimeError("native_schema_revision_refused")
    if not options.session and (options.report / "qualification.log").exists():
        raise RuntimeError("native_report_collision")
    options.report.mkdir(parents=True, exist_ok=True)
    fixture = Fixture(options.container, options.database, options.ci, database=options.database_name,
                      schema_revision=options.schema_revision, role_prefix=options.role_prefix)
    print(json.dumps({"fixture": fixture.database, "status": "ready", "role_prefix": options.role_prefix}), flush=True)
    if options.session:
        return run_session(fixture, sys.stdin, options.report)
    command = options.command[1:] if options.command[:1] == ["--"] else options.command
    return fixture.run("qualification", command, options.report)


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (RuntimeError, subprocess.SubprocessError, OSError, KeyError, ValueError) as error:
        category = str(error)
        if not re.fullmatch(r"native_migration_[0-9]{4}_failed", category):
            category = "native_fixture_bootstrap_failed"
        print(category, file=sys.stderr)
        raise SystemExit(1)
