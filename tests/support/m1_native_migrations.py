"""Ordered migration registry and idempotent replay helpers for native bootstrap."""

from pathlib import Path
import re

SCHEMA_REVISIONS = (
    "0020", "0021", "0022", "0030", "0031", "0032",
    "0033", "0034", "0035", "0036", "0037", "0038",
)
INITIAL_MIGRATIONS = (1, 2, 3, 5, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22)
REFRESH_MIGRATIONS = (12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22)


def migration_numbers(schema_revision: str, initial: bool) -> tuple[int, ...]:
    """Return ordered SQL migration file numbers up through schema_revision."""
    numbers = INITIAL_MIGRATIONS if initial else REFRESH_MIGRATIONS
    if schema_revision >= "0030":
        numbers += tuple(range(23, int(schema_revision) + 1))
    return tuple(n for n in numbers if n <= int(schema_revision))


def make_idempotent(source: str) -> str:
    """Rewrite DDL statements in retained migrations so refresh replay stays idempotent."""
    source = re.sub(r"CREATE SCHEMA (?!IF NOT EXISTS )", "CREATE SCHEMA IF NOT EXISTS ", source)
    source = re.sub(r"CREATE TABLE (?!IF NOT EXISTS )", "CREATE TABLE IF NOT EXISTS ", source)
    source = re.sub(r"CREATE UNIQUE INDEX (?!IF NOT EXISTS )", "CREATE UNIQUE INDEX IF NOT EXISTS ", source)
    source = source.replace("CREATE FUNCTION ", "CREATE OR REPLACE FUNCTION ")
    source = source.replace("CREATE VIEW ", "CREATE OR REPLACE VIEW ")
    # Dynamic CREATE TRIGGER inside EXECUTE format literals (0024)
    # stays a string; OR REPLACE keeps the DO replay idempotent.
    source = source.replace("format('CREATE TRIGGER ", "format('CREATE OR REPLACE TRIGGER ")
    return re.sub(
        r"(?m)^[ \t]*CREATE TRIGGER\s+(\w+)\s+(.*?);",
        lambda m: (
            "DO $refresh$ BEGIN IF NOT EXISTS(SELECT 1 FROM pg_trigger WHERE tgname='"
            + m[1] + "') THEN " + m[0] + " END IF; END $refresh$;"
        ),
        source,
        flags=re.S,
    )


def apply_migrations(root: Path, schema_revision: str, migration_actor: str, sql_fn, initial: bool = False) -> None:
    """Apply migrations up to schema_revision under migration_actor."""
    for number in migration_numbers(schema_revision, initial):
        source = next((root / "migrations").glob(f"{number:04d}_*.sql")).read_text(encoding="utf-8")
        if not initial:
            source = make_idempotent(source)
        try:
            sql_fn(f"SET ROLE {migration_actor};\n" + source)
        except RuntimeError:
            raise RuntimeError(f"native_migration_{number:04d}_failed") from None
