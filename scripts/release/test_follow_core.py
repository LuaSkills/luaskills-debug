"""Offline regressions for following LuaSkills core patches.
跟随 LuaSkills 核心补丁的离线回归测试。
"""

import json
import unittest

import follow_core

MANIFEST = '[package]\nname = "luaskills-debug"\nversion = "0.9.1"\n\n[dependencies]\nluaskills = "=0.9.0"\nserde = "1"\n'


def index(*records):
    """Return sparse index text for `(version, yanked)` records.
    为 `(version, yanked)` 记录返回稀疏索引文本。
    """
    return "\n".join(json.dumps({"name": "luaskills", "vers": version, "yanked": yanked}) for version, yanked in records)


class FollowCoreTests(unittest.TestCase):
    """Patches are followed within the series; minors and yanked versions are not.
    系列内补丁会被跟随；次版本与已撤回版本不会。
    """

    def test_newest_patch_in_the_series_becomes_the_next_debugger_patch(self):
        published = follow_core.published_versions(index(("0.9.0", False), ("0.9.2", False), ("0.9.3", True), ("1.0.0", False), ("0.10.0", False)))
        self.assertEqual(follow_core.plan(MANIFEST, published), {"core": "0.9.2", "version": "0.9.2"})

    def test_current_pin_needs_no_release(self):
        self.assertIsNone(follow_core.plan(MANIFEST, ["0.8.2", "0.9.0"]))

    def test_apply_rewrites_only_the_package_version_and_the_pin(self):
        updated = follow_core.apply(MANIFEST, "0.9.2", "0.9.2")
        self.assertEqual(follow_core.manifest_versions(updated), ("0.9.2", "0.9.2"))
        self.assertIn('serde = "1"', updated)

    def test_series_mismatch_is_rejected(self):
        with self.assertRaisesRegex(ValueError, "major.minor"):
            follow_core.plan(MANIFEST.replace('"=0.9.0"', '"=0.8.2"'), ["0.8.3"])

    def test_inexact_pin_is_rejected(self):
        with self.assertRaisesRegex(ValueError, "exact luaskills pin"):
            follow_core.plan(MANIFEST.replace('"=0.9.0"', '"0.9"'), [])


if __name__ == "__main__":
    unittest.main()
