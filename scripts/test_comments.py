import unittest

from comments import scan_source, strip_source, verdict


def texts(src, ext):
    return [c.text for c in scan_source(src, ext)]


class Scanning(unittest.TestCase):
    def test_comment_markers_inside_strings_are_code(self):
        self.assertEqual(texts('let u = "http://x"; // real\n', ".rs"), ["// real"])
        self.assertEqual(texts('let u = r#"a // b "# ;\n', ".rs"), [])
        self.assertEqual(texts("fn f<'a>(x: &'a str) -> char { '/' }\n", ".rs"), [])
        self.assertEqual(texts("const a = `x ${b ? '//' : `//`} y`; // c\n", ".ts"), ["// c"])
        self.assertEqual(texts("const r = /\\/\\//g; /* c */\n", ".ts"), ["/* c */"])
        self.assertEqual(texts('val s = """\n// not\n"""\n', ".kt"), [])
        self.assertEqual(texts('x = "# no"  # yes\n', ".py"), ["# yes"])
        self.assertEqual(texts("echo $# ${#a}  # yes\n", ".sh"), ["# yes"])
        self.assertEqual(texts("SELECT '--' -- yes\n", ".sql"), ["-- yes"])

    def test_nested_rust_block_comment_is_one_comment(self):
        self.assertEqual(texts("/* a /* b */ c */ x\n", ".rs"), ["/* a /* b */ c */"])

    def test_directives_and_safety_notes_are_not_comments(self):
        src = "#!/usr/bin/env node\n// eslint-disable-next-line x\n// @ts-expect-error y\n// SAFETY: z\n"
        self.assertEqual(texts(src, ".ts"), [])
        self.assertEqual(texts("import x  # noqa: F401\n", ".py"), [])

    def test_doc_and_trailing_comments_are_classified(self):
        [doc, trailing] = scan_source("/// d\nfn f() {} // t\n", ".rs")
        self.assertEqual((doc.kind, doc.own_lines), ("doc", True))
        self.assertEqual((trailing.kind, trailing.own_lines), ("line", False))


class Stripping(unittest.TestCase):
    def test_strips_own_line_and_trailing_comments_and_the_gap_they_leave(self):
        src = "//! m\n\nuse a;\n\n// why\n\nfn f() {} // t\n"
        self.assertEqual(strip_source(src, ".rs", {1, 5, 7}), "use a;\n\nfn f() {}\n")

    def test_leaves_other_blank_lines_alone(self):
        src = "a = 1\n\n\ndef f():\n    # c\n    return 1\n"
        self.assertEqual(strip_source(src, ".py", {5}), "a = 1\n\n\ndef f():\n    return 1\n")


class Ratchet(unittest.TestCase):
    def test_new_and_grown_files_fail(self):
        result = verdict({"a.rs": 2}, {"a.rs": 3, "b.rs": 1})
        self.assertEqual(result.grown, {"a.rs": (3, 2), "b.rs": (1, 0)})

    def test_shrunk_files_ask_for_a_baseline_update(self):
        result = verdict({"a.rs": 2, "gone.rs": 1}, {"a.rs": 1})
        self.assertEqual((result.grown, result.shrunk), ({}, ["a.rs", "gone.rs"]))


if __name__ == "__main__":
    unittest.main()
