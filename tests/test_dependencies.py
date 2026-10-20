"""Actual import, alias, re-export and qualified-path counterexamples."""
import importlib.util
from pathlib import Path
import unittest

ROOT = Path(__file__).resolve().parents[1]
spec = importlib.util.spec_from_file_location('dependency_direction', ROOT / 'scripts/check_dependencies.py')
check = importlib.util.module_from_spec(spec)
spec.loader.exec_module(check)


class Direction(unittest.TestCase):
    def test_actual_contracts_keep_the_inward_boundary(self):
        for name in check.PROTECTED:
            self.assertEqual(check.violations(name, (ROOT / 'src' / name).read_text()), [], name)

    def test_import_alias_reexport_and_qualified_bypass_fail(self):
        for source in (
            'use postgres::Client;',
            'use crate::postgres_mission as reader;',
            'pub use crate::{mission::AssetRef, postgres_mission::{PgMissionStore as Narrow}};',
            'fn x() { let _ = crate::postgres_mission::PgMissionStore::new(c); }',
            'pub use crate::{m1_reasoning::{transport as wire}};',
            'extern crate curl as wire;',
            'use std::{process::Command as Typed};',
            'use crate::m1_run::RunRecord;',
        ):
            self.assertTrue(check.violations('m1_context.rs', source), source)
        self.assertTrue(check.violations('m1_reasoning/inference.rs', 'use super::{transport as p};'))

    def test_comments_strings_and_typed_contracts_are_not_imports(self):
        source = '''
        // use postgres::Client;
        /* nested /* use curl::Easy; */ comment */
        const X: &str = r##"use postgres::Client;"##;
        const Y: &str = "crate::postgres_mission::PgMissionStore";
        use crate::{mission::AssetRef, m1_terrain_view::{TerrainDependency as Input}};
        '''
        self.assertEqual(check.violations('m1_context.rs', source), [])
