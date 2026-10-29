"""Versioned-migration classification controls for check_file_loc."""
import unittest

from test_file_loc import LocTest

ADVISORY = (
    "migrations/0001_x.sql",
    "migrations/0021_inference_lifecycle.sql",
    "migrations/0099_a_b_c.sql",
)
ENFORCED = (
    "migrations/scratch.sql",       # no versioned NNNN_ prefix
    "migrations/0099.sql",          # digits but no _name
    "migrations/tools/0001_x.sql",  # nested, not a direct child
    "migrations/0001_x.SQL",        # suffix must be lowercase .sql
    "migrations/000a_x.sql",        # non-ASCII digit in version field
    "migrations/00012_x.sql",       # five digits
    "migrations/0001_.sql",         # empty name after version
    "migrations/١٢٣٤_x.sql",         # non-ASCII digits do not match [0-9]
    "scripts/query.sql",            # ordinary first-party SQL
)


class MigrationScope(LocTest):
    def test_versioned_migrations_stay_advisory(self):
        fx = self.make()
        for rel in ADVISORY:
            fx.lines(rel, 450)
        res = fx.loc()
        self.assertEqual(res.returncode, 0, res.stdout + res.stderr)
        for rel in ADVISORY:
            self.assertNotIn(rel, res.stdout)

    def test_nonmatching_sql_is_enforced(self):
        for rel in ENFORCED:
            with self.subTest(rel=rel):
                fx = self.make()
                fx.lines(rel, 401)
                res = fx.loc()
                self.assertEqual(res.returncode, 1,
                                 rel + ": " + res.stdout)
                self.assertIn("NEW_OVER", res.stdout)
                shown = rel.encode("ascii", "backslashreplace").decode()
                self.assertIn(shown, res.stdout)

    def test_oversized_existing_migration_growth_also_exempt(self):
        fx = self.make()
        fx.lines("migrations/0001_x.sql", 450)
        fx.commit_all()
        fx.rebase()
        fx.lines("migrations/0001_x.sql", 600)
        self.assertEqual(fx.loc().returncode, 0)


if __name__ == "__main__":
    unittest.main()
