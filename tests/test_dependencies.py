"""Actual import, alias, re-export and qualified-path counterexamples."""
import importlib.util
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
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
        for source in ('use reqwest::Client;', 'use crate::postgres_m1_terrain::https;', 'use curl::easy::Easy;', 'use std::process::Command;'):
            self.assertTrue(check.violations('m1_https.rs', source), source)
            self.assertTrue(check.violations('m1_https_terrain.rs', source), source)
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


class SdkBoundary(unittest.TestCase):
    def test_owner_local_extraction_does_not_escape_protected_imports(self):
        for owner in check.PROTECTED_OWNERS:
            for suffix in ('source.rs', 'nested/implementation.rs'):
                name = owner + suffix
                for source in ('use postgres::Client;', 'pub use crate::postgres_mission as hidden;',
                               'use std::process::Command;', 'use crate::m1_reasoning::sdk_transport::SdkProvider;',
                               'pub use genai::Client as Narrow;'):
                    with self.subTest(name=name, source=source):
                        self.assertTrue(check.violations(name, source))
        self.assertEqual(check.violations('m1_run/live.rs', 'use crate::m1_run::RunRequest;'), [])
        self.assertTrue(check.violations('m1_context/live.rs', 'use crate::m1_run::RunRequest;'))

    def test_sdk_aliases_reexports_and_paths_are_confined(self):
        for name in ('m1_reasoning/inference_intent.rs', 'm1_run_inference.rs', 'mission.rs'):
            for source in ('use genai::Client as Narrow;', 'pub use reqwest::{Client as Safe};',
                           'extern crate tokio as runtime;', 'fn x() { let _ = ::genai::Client::builder(); }',
                           'pub use ::tokio::{runtime as hidden};'):
                with self.subTest(name=name, source=source):
                    self.assertTrue(check.violations(name, source))
        for name in ('mission.rs', 'm1_reasoning/inference_intent.rs'):
            self.assertTrue(check.violations(name, 'use crate::m1_reasoning::sdk_transport as infrastructure;'))
        for name, bad in (
            ('m1_https/result.rs', 'use postgres::Client;'),
            ('m1_https/result.rs', 'use crate::m1_run::RunRecord;'),
            ('m1_reasoning/inference_intent/https_loop_template.rs', 'use crate::m1_reasoning::transport;'),
            ('m1_reasoning/inference_intent/https_loop_template.rs', 'use postgres::Client;'),
        ):
            with self.subTest(name=name, source=bad):
                self.assertTrue(check.violations(name, bad))
        self.assertEqual(check.violations('m1_run_inference.rs', 'use crate::m1_reasoning::sdk_transport::SdkProvider;'), [])
        self.assertEqual(check.violations('m1_reasoning/sdk_transport.rs', 'use genai::Client;'), [])

    def test_planning_importing_postgres_fails(self):
        self.assertTrue(check.violations('planning.rs', 'use postgres::Client;'))

    def test_m0_owners_and_children_reject_forbidden_and_accept_allowed_and_raw_bytes(self):
        m0_roots = ('planning.rs', 'registration.rs', 'withdrawal.rs', 'trajectory.rs')
        m0_children = tuple(f'{prefix}{sub}' for prefix in ('planning/', 'registration/', 'withdrawal/', 'trajectory/')
                            for sub in ('child.rs', 'nested/implementation.rs'))
        forbidden = (
            'use postgres::Client;',
            'use crate::postgres_planning_history as reader;',
            'pub use crate::{mission::AssetRef, postgres_trajectory::{PgTrajectory as Narrow}};',
            'fn x() { let _ = crate::postgres_withdrawal::PgWithdrawal::new(c); }',
            'extern crate curl as wire;',
            'use std::{process::Command as Typed};',
            'use crate::planning_cli::run;',
            'use crate::m1_reasoning::{transport as wire};',
            'use crate::m1_reasoning::postgres_terrain;',
            'use crate::m1_run::RunRecord;',
            'use genai::Client as Narrow;',
            'pub use reqwest::{Client as Safe};',
            'extern crate tokio as runtime;',
            'use crate::m1_reasoning::sdk_transport::SdkProvider;',
        )
        for name in m0_roots + m0_children:
            for source in forbidden:
                with self.subTest(name=name, source=source):
                    self.assertTrue(check.violations(name, source))
            allowed = '''
            // use postgres::Client;
            /* nested /* use curl::Easy; */ comment */
            const X: &[u8] = br#"text " use postgres::Client;"#;
            const Y: &str = r#"text " use postgres::Client;"#;
            const B: &[u8] = b"text \\" use postgres::Client;";
            const C: &core::ffi::CStr = c"text \\" use postgres::Client;";
            const CR: &core::ffi::CStr = cr#"text " use postgres::Client;"#;
            use crate::mission::{AssetRef, CampaignId, EngagementId};
            use crate::planning::PlanningAssessed;
            use crate::registration::OperationAllocator;
            use crate::trajectory::Delivered;
            '''
            with self.subTest(name=name, allowed=True):
                self.assertEqual(check.violations(name, allowed), [])

    def test_subprocess_cli_materializes_protected_inventory(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            script = root / 'scripts' / 'check_dependencies.py'
            script.parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(ROOT / 'scripts/check_dependencies.py', script)
            for rel in check.PROTECTED:
                path = root / 'src' / rel
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_text('use crate::mission::AssetRef;\n', encoding='utf-8')
            (root / 'src' / 'planning.rs').write_text(
                'const X: &[u8] = br#"text " use postgres::Client;"#;\n'
                'const Y: &str = r#"text " use postgres::Client;"#;\n'
                'use crate::mission::AssetRef;\n',
                encoding='utf-8',
            )
            ok = subprocess.run([sys.executable, str(script)], capture_output=True, text=True)
            self.assertEqual(ok.returncode, 0, ok.stdout + ok.stderr)
            self.assertIn(f'{len(check.PROTECTED)} protected modules, 0 violations', ok.stdout)

            (root / 'src' / 'planning.rs').write_text('use postgres::Client;\n', encoding='utf-8')
            bad = subprocess.run([sys.executable, str(script)], capture_output=True, text=True)
            self.assertEqual(bad.returncode, 1, bad.stdout + bad.stderr)
            self.assertIn('FAIL dependency direction planning.rs: postgres::Client', bad.stdout)
