import unittest

from release_notes import extract_notes


class ReleaseNotesTests(unittest.TestCase):
    def test_extracts_only_exact_version_body(self):
        changelog = """# Changelog
## [Unreleased]
- Future change
## [1.2.0] - 2026-11-07
### Added
- New feature
### Fixed
- Bug fix
## [1.1.0] - 2026-10-01
- Older change
"""
        self.assertEqual(
            extract_notes(changelog, "v1.2.0"),
            "### Added\n- New feature\n### Fixed\n- Bug fix\n",
        )

    def test_supports_prerelease_and_last_section(self):
        self.assertEqual(
            extract_notes("## [1.2.0-beta.1]\n\n- Preview\n", "v1.2.0-beta.1"),
            "- Preview\n",
        )

    def test_rejects_missing_duplicate_or_empty_entries(self):
        for changelog in [
            "## [Unreleased]\n- Future change\n",
            "## [1.2.0]\n- A\n## [1.2.0]\n- B\n",
            "## [1.2.0]\n\n## [1.1.0]\n- Old\n",
            "## [1.2.0]\n### Added\n<!-- TODO -->\n",
        ]:
            with self.subTest(changelog=changelog), self.assertRaises(ValueError):
                extract_notes(changelog, "v1.2.0")

    def test_rejects_invalid_tag_and_unreleased(self):
        for tag in ["1.2.0", "V1.2.0", "vUnreleased"]:
            with self.subTest(tag=tag), self.assertRaises(ValueError):
                extract_notes("## [Unreleased]\n- Future\n", tag)


if __name__ == "__main__":
    unittest.main()
