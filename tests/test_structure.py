"""Regression tests for category size limits and preserved hygiene."""
import contextlib
import importlib.util
import io
import hashlib
import json
import shutil
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

SCRIPT = Path(__file__).resolve().parent.parent / "scripts/check_structure.py"


def check_files(files):
    spec = importlib.util.spec_from_file_location("structure", SCRIPT)
    checker = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(checker)
    with tempfile.TemporaryDirectory() as tmp:
        for rel, text in files.items():
            path = Path(tmp) / rel
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text(text, encoding="utf-8", newline="\n")
        checker.ROOT = tmp
        output = io.StringIO()
        with contextlib.redirect_stdout(output):
            code = checker.main()
        return code, output.getvalue()


class StructureTests(unittest.TestCase):
    def assert_boundary(self, path, cap):
        for final_newline in (False, True):
            for size in (cap, cap + 1):
                with self.subTest(path=path, size=size, newline=final_newline):
                    text = "\n".join("// bounded" for _ in range(size))
                    if final_newline:
                        text += "\n"
                    code, out = check_files({path: text})
                    self.assertEqual(code, 0, out)
                    if size > cap:
                        self.assertIn(f"{size} lines > {cap}", out)
                        self.assertIn("WARN", out)

    def test_production_cap_stays_400(self):
        self.assert_boundary("src/domain.rs", 400)

    def test_tooling_cap_stays_400(self):
        self.assert_boundary("scripts/task.py", 400)

    def test_tests_and_benchmarks_cap_500(self):
        for path in ("tests/domain.rs", "tests/test_gate.py", "benches/domain.rs",
                     "src/domain/tests.rs"):
            self.assert_boundary(path, 500)

    def test_markdown_cap_600_independent_of_location(self):
        for path in ("docs/contract.md", "AGENTS.md", "tests/README.md"):
            self.assert_boundary(path, 600)

    def test_sql_gets_400_reference_not_600(self):
        self.assert_boundary("migrations/0001_init.sql", 400)

    def test_physical_lines_agree_with_enforced_counting(self):
        # Only LF/CRLF/CR delimit physical lines; other Unicode/control
        # separators inside text are content, matching the LOC gate.
        for sep in ("\u2028", "\u2029", "\x0c", "\x0b", "\x85"):
            with self.subTest(sep=repr(sep)):
                body = "\n".join(f'const S{i}: &str = "a{sep}b";' for i in range(401))
                code, out = check_files({"src/domain.rs": body + "\n"})
                self.assertEqual(code, 0, out)
                self.assertIn("401 lines > 400", out)
                short = "\n".join(f'const S{i}: &str = "a{sep}b";' for i in range(400))
                code, out = check_files({"src/domain.rs": short + "\n"})
                self.assertNotIn("lines > 400", out)

    def test_terminator_forms_and_edge_counts(self):
        cases = {"src/e.rs": ("", 0), "src/one.rs": ("x", 1),
                 "src/blanks.rs": ("a\n\n\nb", 4),
                 "src/mix.rs": ("a\nb\r\nc\rd", 4),
                 "src/cr.rs": ("x\r" * 401, 401)}
        for rel, (text, want) in cases.items():
            with self.subTest(rel=rel):
                code, out = check_files({rel: text})
                if want > 400:
                    self.assertIn(f"{want} lines > 400", out)
                else:
                    self.assertNotIn("lines > 400", out)

    def test_sql_conflict_markers_still_fail(self):
        for marker in ("<<<<<<< HEAD\n", ">>>>>>> branch\n", "=======\n"):
            code, out = check_files({"migrations/0001_init.sql": marker})
            self.assertEqual(code, 1, marker)
            self.assertIn("Conflict markers: 1", out)

    def test_comments_and_blanks_still_count(self):
        code, out = check_files({"src/domain.rs": "// comment\n" * 200 + "\n" * 201})
        self.assertEqual(code, 0, out)
        self.assertIn("401 lines > 400", out)

    def test_forbidden_paths_still_fail(self):
        for path in ("src/helpers.rs", "tests/common/test_case.rs"):
            code, out = check_files({path: "// small\n"})
            self.assertEqual(code, 1, out)
            self.assertIn("forbidden:", out)

    def test_conflict_markers_still_fail(self):
        code, out = check_files({"docs/contract.md": "<<<<<<< HEAD\n"})
        self.assertEqual(code, 1, out)
        self.assertIn("Conflict markers: 1", out)

    def test_production_hygiene_still_fails(self):
        for text in ("fn bad() { value.unwrap(); }", "unsafe { action(); }"):
            code, out = check_files({"src/domain.rs": text})
            self.assertEqual(code, 1, out)
            self.assertIn("Rust production hygiene: 1", out)

    def test_child_tests_filename_does_not_waive_hygiene(self):
        code, out = check_files({"src/domain/tests.rs": "fn bad() { value.unwrap(); }"})
        self.assertEqual(code, 1, out)
        self.assertIn("Rust production hygiene: 1", out)

    def test_existing_test_hygiene_exemptions_preserved(self):
        for path, text in (
            ("tests/domain.rs", "fn test() { value.unwrap(); }"),
            ("src/domain.rs", "#[cfg(test)]\nmod tests { fn test() { value.unwrap(); } }"),
        ):
            code, out = check_files({path: text})
            self.assertEqual(code, 0, out)

    def test_production_unwrap_after_external_cfg_test_module_fails(self):
        text = (
            "#[cfg(test)]\n"
            "#[path = \"../tests/support/m1_native_cli.rs\"]\n"
            "mod tests;\n"
            "\n"
            "pub fn prod() { value.unwrap(); }\n"
        )
        code, out = check_files({"src/m1_reasoning_cli.rs": text})
        self.assertEqual(code, 1, out)
        self.assertIn("src/m1_reasoning_cli.rs:5", out)

    def test_before_after_inline_and_external_cfg_test_preserve_line_numbers(self):
        cases = (
            ("unwrap", "a.unwrap();", "b.unwrap();"),
            ("expect", 'a.expect("x");', 'b.expect("y");'),
            ("unsafe", "unsafe { a(); }", "unsafe { b(); }"),
        )
        for label, before_stmt, after_stmt in cases:
            with self.subTest(kind=label):
                text = (
                    f"fn first() {{ {before_stmt} }}\n"
                    "#[cfg(test)]\n"
                    "mod tests { fn ok() { x.unwrap(); y.expect(\"z\"); unsafe { z(); } } }\n"
                    "#[cfg(test)]\n"
                    "#[path = \"ext.rs\"]\n"
                    "mod ext_tests;\n"
                    f"fn second() {{ {after_stmt} }}\n"
                )
                code, out = check_files({"src/domain.rs": text})
                self.assertEqual(code, 1, out)
                self.assertIn("src/domain.rs:1", out)
                self.assertIn("src/domain.rs:7", out)
                self.assertIn("Rust production hygiene: 2", out)

    def test_same_line_trailing_production_and_multiple_cfg_items_fail(self):
        for text, expected_line, expected_msg in (
            ('#[cfg(test)] mod t { fn ok() { x.unwrap(); } } fn prod() { y.unwrap(); }\n', 1, "unwrap/expect on production path"),
            ('#[cfg(test)] #[path = "t.rs"] mod t; fn prod() { y.expect("e"); }\n', 1, "unwrap/expect on production path"),
            ('#[cfg(test)] fn t() { x.unwrap(); }\n#[cfg(test)] mod m { fn k() { z.unwrap(); } }\nfn prod() { unsafe { f(); } }\n', 3, "unsafe block"),
            ('#[cfg(test)] const {} let _ = Some(1).unwrap();\n', 1, "unwrap/expect on production path"),
        ):
            with self.subTest(text=text):
                code, out = check_files({"src/domain.rs": text})
                self.assertEqual(code, 1, out)
                self.assertIn(f"src/domain.rs:{expected_line} {expected_msg}", out)

    def test_repo_cfg_test_item_forms_and_clean_external_cfg_pass(self):
        text = (
            "#[cfg(test)]\n"
            "use std::process::{Command, Stdio};\n"
            "#[cfg(test)]\n"
            "type Runner = dyn FnMut(&[&str]) -> i32;\n"
            "#[cfg(test)]\n"
            "pub(super) struct Scope(Option<i32>);\n"
            "#[cfg(test)]\n"
            "impl Drop for Scope { fn drop(&mut self) { let _ = self.0.unwrap(); } }\n"
            "#[cfg(test)]\n"
            "thread_local! { static CELL: i32 = { let x = Some(1); x.unwrap() }; }\n"
            "#[cfg(test)]\n"
            "pub(super) fn helper<F, R>(f: F) -> R where F: FnOnce() -> R { let _ = Some(1).unwrap(); f() }\n"
            "fn prod() -> i32 {\n"
            "    #[cfg(test)]\n"
            "    let _started = Some(1).unwrap();\n"
            "    #[cfg(test)]\n"
            "    eprintln!(\"{}\", Some(1).unwrap());\n"
            "    #[cfg(test)]\n"
            "    if let Some(v) = Some(1) { return v; }\n"
            "    #[cfg(test)]\n"
            "    { let _ = Some(1).unwrap(); }\n"
            "    #[cfg(test)]\n"
            "    const { let _ = Some(1).unwrap(); }\n"
            "    #[cfg(test)]\n"
            "    if { true } { let _ = Some(1).unwrap(); }\n"
            "    if !{ #[cfg(test)] let _ = Some(1).unwrap(); true } {}\n"
            "    42\n"
            "}\n"
            "#[cfg(test)]\n"
            "#[cfg(target_os = \"linux\")]\n"
            "#[path = \"unit.rs\"]\n"
            "mod tests;\n"
        )
        code, out = check_files({"src/domain.rs": text})
        self.assertEqual(code, 0, out)

    def test_comments_literals_lifetimes_and_macros_do_not_corrupt_boundaries(self):
        safe_with_tricky_tokens = (
            "#[cfg(test)]\n"
            "fn with_lifetime<'a, 'β>(s: &'a str, _u: &'β str) -> &'a str {\n"
            "    /* nested /* } #[cfg(test)] */ */\n"
            "    let _s = \"\\\" } #[cfg(test)]\";\n"
            "    let _r = r#\" } #[cfg(test)] \"#;\n"
            "    let _b = br#\" } \\\" #[cfg(test)] \"#;\n"
            "    let _c = '}'; let _q = '\\''; let _d = '\"'; let _u = 'β';\n"
            "    let _ = Some(1).unwrap();\n"
            "    s\n"
            "}\n"
            "fn clean() {}\n"
        )
        code, out = check_files({"src/domain.rs": safe_with_tricky_tokens})
        self.assertEqual(code, 0, out)

        for bad_after in (
            "// #[cfg(test)]\nmod tests { fn bad() { x.unwrap(); } }\n",
            "/* nested /* #[cfg(test)] */ */\nmod tests { fn bad() { x.unwrap(); } }\n",
            'const S: &str = "#[cfg(test)]";\nmod tests { fn bad() { x.unwrap(); } }\n',
            'const R: &str = r#"#[cfg(test)]"#;\nmod tests { fn bad() { x.unwrap(); } }\n',
            'const B: &[u8] = br#"#[cfg(test)]"#;\nmod tests { fn bad() { x.unwrap(); } }\n',
            "#[cfg(test)]\nfn life<'a>(x: &'a str) -> &'a str { x }\nfn prod() { y.unwrap(); }\n",
            "my_macro! { #[cfg(test)] }\nfn prod() { y.unwrap(); }\n",
            "stringify!(#[cfg(test)] mod m;);\nfn prod() { y.unwrap(); }\n",
            "mod tests { fn bad() { x.unwrap(); } }\n",
            "#[cfg(testβ)]\nmod tests { fn bad() { x.unwrap(); } }\n",
            "#[cfg(all(test, target_os = \"linux\"))]\nmod tests { fn bad() { x.unwrap(); } }\n",
        ):
            with self.subTest(bad_after=bad_after):
                code, out = check_files({"src/domain.rs": bad_after})
                self.assertEqual(code, 1, out)
                self.assertIn("Rust production hygiene: 1", out)

    def test_malformed_lexical_delimiters_and_orphaned_cfg_test_fail(self):
        for label, bad in (
            ("unclosed_block_comment", "/* /* inner */ fn ok() {}\n"),
            ("unclosed_string", 'const S: &str = "unterminated;\n'),
            ("unclosed_raw_string", 'const R: &str = r#"unterminated";\n'),
            ("unclosed_raw_byte_string", 'const B: &[u8] = br#"unterminated";\n'),
            ("unclosed_char", "const C: char = '; fn ok() {}\n"),
            ("unclosed_byte_char", "const B: u8 = b'x; fn ok() {}\n"),
            ("unclosed_test_mod", "#[cfg(test)]\nmod tests { fn ok() {}\n"),
            ("mismatched_test_delim", "#[cfg(test)]\nmod tests { fn ok( {} }\n"),
            ("orphaned_cfg_test", "#[cfg(test)]\n"),
            ("orphaned_in_mod", "mod m { #[cfg(test)] }\n"),
            ("inner_cfg_test", "#![cfg(test)]\nfn ok() {}\n"),
            ("unsupported_target", "#[cfg(test)] ;\n"),
        ):
            with self.subTest(case=label):
                code, out = check_files({"src/domain.rs": bad})
                self.assertEqual(code, 1, out)
                self.assertIn("FAIL  src/domain.rs:", out)

    def test_subprocess_cli_legal_and_illegal_repositories(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            script = root / "scripts" / "check_structure.py"
            script.parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(SCRIPT, script)
            src = root / "src" / "m1_reasoning_cli.rs"
            src.parent.mkdir(parents=True, exist_ok=True)
            src.write_text('#[cfg(test)]\n#[path = "../tests/t.rs"]\nmod tests;\npub fn ok() {}\n', encoding="utf-8")
            ok = subprocess.run([sys.executable, str(script)], capture_output=True, text=True)
            self.assertEqual(ok.returncode, 0, ok.stdout + ok.stderr)
            self.assertIn("Rust production hygiene: 0", ok.stdout)
            src.write_text('#[cfg(test)]\n#[path = "../tests/t.rs"]\nmod tests;\npub fn bad() { x.unwrap(); }\n', encoding="utf-8")
            bad = subprocess.run([sys.executable, str(script)], capture_output=True, text=True)
            self.assertEqual(bad.returncode, 1, bad.stdout + bad.stderr)
            self.assertIn("src/m1_reasoning_cli.rs:4", bad.stdout)





class VendoredSdkTests(unittest.TestCase):
    def snapshot(self):
        source = SCRIPT.parent.parent / 'third_party' / 'genai'
        return {'third_party/genai/' + p.relative_to(source).as_posix(): p.read_text(encoding='utf-8')
                for p in source.rglob('*') if p.is_file()}

    def test_exact_snapshot_waives_only_upstream_hygiene_and_size(self):
        code, out = check_files(self.snapshot())
        self.assertEqual(code, 0, out)
        self.assertNotIn('third_party/genai/src/common/', out)

    def test_first_party_and_similarly_named_paths_remain_checked(self):
        for name in ('src/domain.rs', 'third_party/genai-other/common.rs'):
            files = self.snapshot()
            files[name] = 'fn bad() { value.unwrap(); }'
            code, out = check_files(files)
            self.assertEqual(code, 1, out)
            self.assertIn('FAIL', out)

    def test_extra_or_altered_sdk_source_invalidates_exemption(self):
        for path in ('third_party/genai/src/campaign.rs', 'third_party/genai/src/common/mod.rs'):
            files = self.snapshot()
            files[path] = 'fn bad() { value.unwrap(); }'
            code, out = check_files(files)
            self.assertEqual(code, 1, out)
            self.assertIn('SDK provenance', out)

    def test_cosmetic_member_cannot_admit_code_changes_through_manifest(self):
        path = 'src/adapter/adapters/zai/mod.rs'
        files = self.snapshot()
        files['third_party/genai/' + path] += '\npub fn injected() {}\n'
        manifest = json.loads(files['third_party/genai/source-manifest.json'])
        member = next(entry for entry in manifest['files'] if entry['path'] == path)
        member['sha256'] = hashlib.sha256(files['third_party/genai/' + path].encode()).hexdigest()
        files['third_party/genai/source-manifest.json'] = json.dumps(manifest)
        code, out = check_files(files)
        self.assertEqual(code, 1, out)
        self.assertIn('out-of-map SDK modification', out)

    def test_sdk_conflict_markers_remain_checked(self):
        files = self.snapshot()
        files['third_party/genai/src/client/client_impl.rs'] += '\n<<<<<<< HEAD\n'
        code, out = check_files(files)
        self.assertEqual(code, 1, out)
        self.assertIn('Conflict markers: 1', out)

    def test_canonical_snapshot_survives_windows_crlf_checkout(self):
        files={name:text.replace("\n","\r\n") for name,text in self.snapshot().items()}
        code,out=check_files(files)
        self.assertEqual(code,0,out)


if __name__ == "__main__":
    unittest.main()
