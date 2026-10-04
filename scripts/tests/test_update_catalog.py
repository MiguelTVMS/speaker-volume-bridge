import importlib.util
import json
from pathlib import Path
import unittest
import subprocess

ROOT = Path(__file__).resolve().parents[2]
SPEC = importlib.util.spec_from_file_location("update_catalog", ROOT / "scripts/validate-update-catalog.py")
CATALOG = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(CATALOG)


def entry(**changes):
    value = {
        "edition": "direct_macos",
        "channel": "stable",
        "os": "macos",
        "architecture": "aarch64",
        "version": "2.0.0",
        "publishedAt": "2026-10-04T12:00:00Z",
        "releaseNotes": "A verified public release.",
        "action": {"type": "open_url", "url": "https://example.invalid/downloads"},
    }
    value.update(changes)
    return value


def raw(entries=None, **changes):
    value = {"schemaVersion": 1, "generatedAt": "2026-10-04T12:00:00Z", "entries": entries or []}
    value.update(changes)
    return json.dumps(value).encode()


class UpdateCatalogTests(unittest.TestCase):
    def test_repository_catalog_is_valid_and_empty_until_published(self):
        parsed = CATALOG.validate_catalog_bytes((ROOT / "pages/updates/v1/catalog.json").read_bytes())
        self.assertEqual(parsed["entries"], [])

    def test_shared_cross_consumer_fixtures(self):
        fixtures = ROOT / "tests/fixtures/update-catalog"
        for name in ("additive-metadata.json", "missing-required.json", "unsupported-action.json", "duplicate-key.json", "duplicate-target.json"):
            with self.subTest(name=name):
                raw_fixture = (fixtures / name).read_bytes()
                if name == "additive-metadata.json":
                    CATALOG.validate_catalog_bytes(raw_fixture)
                else:
                    with self.assertRaises(CATALOG.CatalogError):
                        CATALOG.validate_catalog_bytes(raw_fixture)
        result = subprocess.run(["cargo", "test", "-p", "speaker-volume-bridge", "updates::tests::shared_catalog_fixture", "--", "--nocapture"], cwd=ROOT, capture_output=True, text=True)
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)

    def test_accepts_each_supported_distribution(self):
        entries = [
            entry(),
            entry(edition="direct_windows", os="windows", architecture="x86_64"),
            entry(edition="microsoft_store", os="windows", architecture="aarch64"),
            entry(edition="mac_app_store"),
            entry(edition="debian", os="linux", architecture="x86_64"),
        ]
        self.assertEqual(len(CATALOG.validate_catalog_bytes(raw(entries))["entries"]), 5)

    def assert_invalid(self, payload, message):
        with self.assertRaisesRegex(CATALOG.CatalogError, message):
            CATALOG.validate_catalog_bytes(payload)

    def test_rejects_malformed_and_duplicate_json_members(self):
        self.assert_invalid(b"{", "valid UTF-8 JSON")
        self.assert_invalid(b'{"schemaVersion":1,"schemaVersion":1}', "duplicate object member")

    def test_rejects_duplicate_targets(self):
        self.assert_invalid(raw([entry(), entry(version="2.0.1")]), "duplicate target")

    def test_rejects_unsupported_schema_and_action(self):
        self.assert_invalid(raw(schemaVersion=2), "unsupported schemaVersion")
        self.assert_invalid(raw([entry(action={"type": "execute", "url": "https://example.invalid"})]), "unsupported action")

    def test_rejects_missing_or_unknown_edition(self):
        missing = entry()
        del missing["edition"]
        self.assert_invalid(raw([missing]), "missing fields: edition")
        self.assert_invalid(raw([entry(edition="custom")]), "unsupported edition")

    def test_rejects_invalid_version_and_prerelease_on_stable(self):
        self.assert_invalid(raw([entry(version="v2")]), "invalid semantic version")
        self.assert_invalid(raw([entry(version="2.0.0-beta.1")]), "stable entry cannot use a prerelease")

    def test_rejects_insecure_or_executable_action_targets(self):
        self.assert_invalid(raw([entry(action={"type": "open_url", "url": "http://example.invalid"})]), "absolute HTTPS URL")
        self.assert_invalid(raw([entry(action={"type": "open_url", "url": "file:///tmp/installer"})]), "absolute HTTPS URL")


if __name__ == "__main__":
    unittest.main()
