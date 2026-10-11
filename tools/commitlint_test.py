"""Tests of tools/commitlint.py: python tools/commitlint_test.py"""
import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from commitlint import check_message, check_title  # noqa: E402


class Titles(unittest.TestCase):
    def test_accepts_conventional_titles(self):
        for t in [
            "fix(engine): keep a slot's cooldown when its row leaves the closed HUD",
            "feat: start the game from the Scan key",
            "docs: say which page to read for each question",
            "ci: build the plugin on windows-latest",
            "chore(release): 0.2.0",
            "feat(plugin)!: drop the Python helper",
        ]:
            self.assertEqual(check_title(t), [], t)

    def test_lets_git_generated_titles_through(self):
        for t in ["Merge branch 'dev'", 'Revert "feat: x"', "fixup! fix: x", "squash! feat: y"]:
            self.assertEqual(check_title(t), [], t)

    def test_refuses_a_missing_type(self):
        self.assertTrue(check_title("Keep the cooldown when the row goes away"))

    def test_refuses_an_unknown_type(self):
        self.assertTrue(check_title("feature: start the game"))

    def test_refuses_a_capital_type_or_scope(self):
        self.assertTrue(check_title("Fix: x"))
        self.assertTrue(check_title("fix(Engine): x"))

    def test_refuses_a_final_period(self):
        self.assertTrue(check_title("fix: stop the crash."))

    def test_refuses_a_long_title(self):
        self.assertTrue(check_title("fix: " + "x" * 100))

    def test_refuses_no_space_after_the_colon(self):
        self.assertTrue(check_title("fix:stop the crash"))


class Messages(unittest.TestCase):
    def test_body_needs_an_empty_line(self):
        self.assertEqual(check_message("fix: a\n\nwhy it changed\n"), [])
        self.assertTrue(check_message("fix: a\nwhy it changed\n"))

    def test_comments_are_ignored(self):
        self.assertEqual(check_message("# Please enter the commit message\nfix: a\n# comment\n"), [])

    def test_empty_message(self):
        self.assertTrue(check_message("# only a comment\n\n"))

    def test_breaking_change_needs_the_bang(self):
        self.assertTrue(check_message("feat: a\n\nBREAKING CHANGE: b\n"))
        self.assertEqual(check_message("feat!: a\n\nBREAKING CHANGE: b\n"), [])


if __name__ == "__main__":
    unittest.main()
