"""Physical-line counting and rename-identity controls for check_file_loc."""
import ast
import unittest

from test_file_loc import LocTest, git, run_loc, write_text

NONPHYSICAL = ("\u2028", "\u2029", "\x0c", "\x0b", "\x85")


class PhysicalCounting(LocTest):
    def test_unicode_separators_are_content_not_terminators(self):
        for sep in NONPHYSICAL:
            with self.subTest(sep=repr(sep)):
                fx = self.make()
                body = "\n".join(f"s{i} = 'a{sep}b'" for i in range(400))
                write_text(fx.repo, "src/a.py", body + "\n")
                res = fx.loc()
                self.assertEqual(res.returncode, 0,
                                 res.stdout + res.stderr)
                self.assertIn("current=400", res.stdout)
                write_text(fx.repo, "src/a.py",
                           body + "\nextra = 1\n")
                res = fx.loc()
                self.assertEqual(res.returncode, 1, res.stdout)

    def test_test_category_unicode_separator_boundaries(self):
        fx = self.make()
        body = "\n".join(f"s{i} = 'a\u2028b'" for i in range(500))
        write_text(fx.repo, "tests/t.py", body + "\n")
        self.assertEqual(fx.loc().returncode, 0)
        write_text(fx.repo, "tests/t.py", body + "\nextra = 1\n")
        self.assertEqual(fx.loc().returncode, 1)

    def test_terminator_forms_and_edge_counts(self):
        fx = self.make()
        for eol in ("\n", "\r\n", "\r"):
            write_text(fx.repo, "src/a.rs",
                       eol.join(["x"] * 400) + eol)
            res = fx.loc()
            self.assertEqual(res.returncode, 0, (repr(eol), res.stdout))
            self.assertIn("current=400", res.stdout)
        write_text(fx.repo, "src/empty.rs", "")
        write_text(fx.repo, "src/one.rs", "x")
        write_text(fx.repo, "src/blanks.rs", "a\n\n\nb")
        write_text(fx.repo, "src/mix.rs", "a\nb\r\nc\rd")
        res = fx.loc()
        self.assertEqual(res.returncode, 0, res.stdout)
        for rel, count in (("src/empty.rs", 0), ("src/one.rs", 1),
                           ("src/blanks.rs", 4), ("src/mix.rs", 4)):
            row = next(l for l in res.stdout.splitlines()
                       if l.startswith("WITHIN    " + rel))
            self.assertIn(f"current={count} ", row, rel)


class RenameIdentity(LocTest):
    def test_eol_and_final_terminator_conversion_keeps_allowance(self):
        fx = self.make()
        fx.lines("src/old.py", 450, eol="\n")
        fx.commit_all()
        fx.rebase()
        git(fx.repo, "mv", "src/old.py", "src/new.py")
        fx.lines("src/new.py", 450, eol="\r\n", final=False)
        res = fx.loc()
        self.assertEqual(res.returncode, 0, res.stdout)
        self.assertIn("RETAINED", res.stdout)

    def test_lone_cr_rename_variant(self):
        fx = self.make()
        fx.lines("src/old.py", 450, eol="\n")
        fx.commit_all()
        fx.rebase()
        git(fx.repo, "mv", "src/old.py", "src/new.py")
        fx.lines("src/new.py", 450, eol="\r")
        res = fx.loc()
        self.assertEqual(res.returncode, 0, res.stdout)
        self.assertIn("RETAINED", res.stdout)

    def test_extra_blank_lines_break_rename_allowance(self):
        fx = self.make()
        fx.lines("src/old.py", 450)
        fx.commit_all()
        fx.rebase()
        git(fx.repo, "mv", "src/old.py", "src/new.py")
        write_text(fx.repo, "src/new.py",
                   "line 0\n\n" + "\n".join(f"line {i}" for i in range(1, 450))
                   + "\n")
        res = fx.loc()
        self.assertEqual(res.returncode, 1, res.stdout)
        self.assertIn("NEW_OVER", res.stdout)

    def test_u2028_to_lf_swap_is_modified_move(self):
        fx = self.make()
        old = "\n".join(f"s{i} = '''x\u2028y'''" for i in range(450)) + "\n"
        write_text(fx.repo, "src/old.py", old)
        fx.commit_all()
        fx.rebase()
        git(fx.repo, "mv", "src/old.py", "src/new.py")
        modified = old.replace("\u2028", "\n")
        # Both are valid Python but the string VALUES differ: prove the
        # semantic distinction via AST constants, never by executing.
        self.assertNotEqual(ast.parse(old).body[0].value.value,
                            ast.parse(modified).body[0].value.value)
        write_text(fx.repo, "src/new.py", modified)
        for mode in ({}, {"head": None}):
            res = run_loc(fx.repo, fx.base,
                          fx.commit_all() if mode else None)
            self.assertEqual(res.returncode, 1, mode)
            self.assertIn("NEW_OVER", res.stdout)
            self.assertNotIn("RETAINED", res.stdout)


if __name__ == "__main__":
    unittest.main()
