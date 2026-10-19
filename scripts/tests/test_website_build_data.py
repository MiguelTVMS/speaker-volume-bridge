"""Test website build-version resolution."""
import importlib.util
from pathlib import Path
import unittest


ROOT = Path(__file__).resolve().parents[2]
SPEC = importlib.util.spec_from_file_location("website_build_data", ROOT / "scripts" / "generate-website-build-data.py")
MODULE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(MODULE)


class WebsiteBuildDataTests(unittest.TestCase):
    def test_repository_uses_the_workspace_version_when_root_npm_has_none(self):
        self.assertIsNone(MODULE.npm_version())
        self.assertRegex(MODULE.cargo_version(), r"^\d+\.\d+\.\d+$")
        data = MODULE.resolve(ref="feat/example", revision="1234567890abcdef")
        self.assertEqual(data["version"], MODULE.cargo_version())
        self.assertEqual(data["version_source"], "Cargo.toml")
        self.assertEqual(data["ref"], "feat/example")
        self.assertEqual(data["revision"], "1234567890ab")

    def test_explicit_version_supports_future_npm_or_release_inputs(self):
        data = MODULE.resolve(ref="main", revision="abcdef", version="2.0.0")
        self.assertEqual(data["version"], "2.0.0")
        self.assertEqual(data["version_source"], "package.json")


if __name__ == "__main__":
    unittest.main()
