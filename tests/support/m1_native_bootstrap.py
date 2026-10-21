"""Own one fresh native fixture and keep credentials in this process and children."""
import argparse
import json
import os
from pathlib import Path
import re
import secrets
import subprocess
import sys
import time
from urllib.parse import quote

IMAGE = "postgres@sha256:d74eeac9a635390a49bc21bd49fccd973de707e2a53a76ac49b552b8712ec46f"
NAMESPACE = "dw_native_"
KINDS = ("fixture", "live")
ROOT = Path(__file__).resolve().parents[2]


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


def wait_with_progress(process, label):
    """Drain both pipes privately; emit only bounded, non-content progress."""
    started = time.monotonic()
    while True:
        try:
            return process.communicate(timeout=15)
        except subprocess.TimeoutExpired:
            print(json.dumps({"action": label, "status": "running",
                              "elapsed_seconds": int(time.monotonic() - started)}), flush=True)


class Fixture:
    def __init__(self, container, kind, ci=False, database=None):
        if container not in ("dwpg-r3", "dwpg"):
            raise RuntimeError("native_service_unqualified")
        if ci and container != "dwpg":
            raise RuntimeError("native_service_unqualified")
        if kind not in KINDS:
            raise RuntimeError("native_identifier_refused")
        self.container = container
        # Container names allow the existing owned dash spelling, never SQL use.
        self.database = database_name(kind, database)
        self.env = dict(os.environ)
        self.env.pop("DEEPSEEK_API_KEY", None)
        self.env["PYTHONDONTWRITEBYTECODE"] = "1"
        self.secrets = []
        info = json.loads(private_run(["docker", "inspect", container]))[0]
        ports = info["NetworkSettings"]["Ports"].get("5432/tcp", [])
        if info["Config"]["Image"] != IMAGE or not info["State"]["Running"]:
            raise RuntimeError("native_service_unqualified")
        if not any(p["HostIp"] == "127.0.0.1" and p["HostPort"] == "5432" for p in ports):
            raise RuntimeError("native_service_unqualified")
        if not self.sql("SELECT version()", database="postgres").startswith("PostgreSQL 17."):
            raise RuntimeError("native_service_unqualified")
        if self.sql(f"SELECT 1 FROM pg_database WHERE datname='{self.database}'", database="postgres").strip():
            raise RuntimeError("native_database_collision")
        suffix = secrets.token_hex(6)
        owners = ["admin", "core", "broker", "cognition", "history", "terrain"]
        if kind == "fixture":
            owners.append("otherbroker")
        self.roles = {name: identifier(f"dw_nf60_{suffix}_{name}") for name in owners}
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
        suffix = secrets.token_hex(6)
        names = {kind: identifier(f"dw_nf60_{suffix}_{kind}") for kind in (
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
        numbers = (1, 2, 3, 5, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20) if initial else (12, 13, 14, 15, 16, 17, 18, 19, 20)
        for number in numbers:
            source = next((ROOT / "migrations").glob(f"{number:04d}_*.sql")).read_text(encoding="utf-8")
            if not initial:
                source = source.replace("CREATE SCHEMA ", "CREATE SCHEMA IF NOT EXISTS ")
                source = source.replace("CREATE TABLE ", "CREATE TABLE IF NOT EXISTS ")
                source = source.replace("CREATE UNIQUE INDEX ", "CREATE UNIQUE INDEX IF NOT EXISTS ")
                source = source.replace("CREATE FUNCTION ", "CREATE OR REPLACE FUNCTION ")
                source = source.replace("CREATE VIEW ", "CREATE OR REPLACE VIEW ")
                source = re.sub(r"CREATE TRIGGER\s+(\w+)\s+(.*?);", lambda m:
                                "DO $refresh$ BEGIN IF NOT EXISTS(SELECT 1 FROM pg_trigger WHERE tgname='" + m[1] +
                                "') THEN " + m[0] + " END IF; END $refresh$;", source, flags=re.S)
            try:
                self.sql(f"SET ROLE {self.migration_actor};\n" + source)
            except RuntimeError:
                raise RuntimeError(f"native_migration_{number:04d}_failed") from None

    def run(self, label, args, report):
        if not re.fullmatch(r"[a-z0-9_-]+", label) or not isinstance(args, list) or not args:
            raise RuntimeError("native_action_refused")
        path = report / f"{label}.log"
        if path.exists():
            raise RuntimeError("native_report_collision")
        with subprocess.Popen(args, cwd=ROOT, env=self.env, stdout=subprocess.PIPE,
                              stderr=subprocess.PIPE, text=True) as process:
            stdout, stderr = wait_with_progress(process, label)
        output = stderr + "\n" + stdout
        for value in self.secrets:
            output = output.replace(value, "[REDACTED]")
        path.write_text(output, encoding="utf-8")
        print(json.dumps({"action": label, "exit": process.returncode}), flush=True)
        print("\n".join(output.splitlines()[-35:]), flush=True)
        return process.returncode


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--container", default="dwpg-r3")
    parser.add_argument("--database", choices=KINDS, default="fixture")
    parser.add_argument("--database-name", default=None)
    parser.add_argument("--report", type=Path, required=True)
    parser.add_argument("--ci", action="store_true")
    parser.add_argument("--session", action="store_true")
    parser.add_argument("command", nargs=argparse.REMAINDER)
    options = parser.parse_args()
    options.report.mkdir(parents=True, exist_ok=True)
    fixture = Fixture(options.container, options.database, options.ci, database=options.database_name)
    print(json.dumps({"fixture": fixture.database, "status": "ready"}), flush=True)
    if options.session:
        for line in sys.stdin:
            action = json.loads(line)
            if action.get("exit"):
                return 0
            if action.get("refresh"):
                fixture.migrate()
                print('{"migration_refresh":"complete"}', flush=True)
            else:
                fixture.run(action["label"], action["args"], options.report)
        return 0
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
