import json
from pathlib import Path
import unittest

ROOT = Path(__file__).resolve().parents[2]


class DistributionPackagingTests(unittest.TestCase):
    def test_each_package_kind_embeds_distinct_provenance(self):
        expected = {
            "tauri.direct.conf.json": "direct-macos.json",
            "tauri.direct-profile.conf.json": "direct-macos.json",
            "tauri.appstore.conf.json": "mac-app-store.json",
            "tauri.windows.conf.json": "direct-windows.json",
            "tauri.linux.conf.json": "debian.json",
        }
        for config_name, source in expected.items():
            with self.subTest(config=config_name):
                config = json.loads((ROOT / "src-tauri" / config_name).read_text())
                self.assertEqual(
                    config["bundle"]["resources"],
                    {f"distribution/{source}": "distribution.json"},
                )

    def test_store_packaging_overrides_reused_executable_provenance(self):
        script = (ROOT / "scripts/build-msix.ps1").read_text()
        self.assertIn("microsoft-store.json", script)
        self.assertIn("$provenance.edition -ne 'microsoft_store'", script)

    def test_provenance_files_are_minimal_and_official(self):
        editions = set()
        for path in (ROOT / "src-tauri/distribution").glob("*.json"):
            value = json.loads(path.read_text())
            self.assertEqual(value["schemaVersion"], 1)
            self.assertEqual(value["publisher"], "MiguelTVMS/speaker-volume-bridge")
            editions.add(value["edition"])
        self.assertEqual(
            editions,
            {
                "direct_macos",
                "direct_windows",
                "microsoft_store",
                "mac_app_store",
                "debian",
            },
        )


if __name__ == "__main__":
    unittest.main()
