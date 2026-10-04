import json
import os
import plistlib
import shutil
import subprocess
import tempfile
from pathlib import Path
import unittest

ROOT = Path(__file__).resolve().parents[2]


class DistributionPackagingTests(unittest.TestCase):
    def test_release_bundle_resolves_entitlements_with_and_without_profile(self):
        workflow = (ROOT / ".github/workflows/release-candidate.yml").read_text()
        step = workflow.split("      - name: Bundle, sign, notarize, and staple the existing executable\n", 1)[1]
        block = step.split("      - name:", 1)[0].split("        run: |\n", 1)[1]
        command = "\n".join(line[10:] for line in block.splitlines())
        for with_profile in (False, True):
            with self.subTest(with_profile=with_profile), tempfile.TemporaryDirectory() as directory:
                workspace = Path(directory)
                shutil.copytree(ROOT / "src-tauri", workspace / "src-tauri",
                                ignore=shutil.ignore_patterns("target"))
                generated = workspace / "src-tauri/Entitlements.direct.plist"
                generated.unlink(missing_ok=True)
                if with_profile:
                    generated.write_bytes(plistlib.dumps({
                        "com.apple.security.app-sandbox": True,
                        "com.apple.security.network.client": True,
                        "com.apple.security.network.server": True,
                        "com.apple.application-identifier": "TEST.example.app",
                        "com.apple.developer.team-identifier": "TEST",
                    }))
                    (workspace / "src-tauri/macos-developer-id.provisionprofile").write_text("fixture")
                bin_dir = workspace / "bin"
                bin_dir.mkdir()
                cargo = bin_dir / "cargo"
                cargo.write_text('#!/bin/sh\nprintf "%s\\n" "$@" > bundle-arguments\n')
                cargo.chmod(0o755)
                subprocess.run(["bash", "-e", "-c", command], cwd=workspace, check=True,
                               env={**os.environ, "PATH": str(bin_dir) + os.pathsep + os.environ["PATH"],
                                    "HAS_DEVELOPER_ID_PROFILE": str(with_profile).lower()})
                arguments = (workspace / "bundle-arguments").read_text().splitlines()
                config_path = workspace / arguments[arguments.index("--config") + 1]
                bundle = json.loads(config_path.read_text())["bundle"]
                entitlements = plistlib.loads((config_path.parent / bundle["macOS"]["entitlements"]).read_bytes())
                for key in ("app-sandbox", "network.client", "network.server"):
                    self.assertTrue(entitlements["com.apple.security." + key])
                self.assertEqual(bundle["resources"], {"distribution/direct-macos.json": "distribution.json"})
                if with_profile:
                    self.assertEqual(entitlements["com.apple.application-identifier"], "TEST.example.app")
                    self.assertTrue((config_path.parent / bundle["macOS"]["files"]["embedded.provisionprofile"]).is_file())
                else:
                    self.assertNotIn("files", bundle["macOS"])
                    self.assertNotIn("com.apple.application-identifier", entitlements)

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

    def test_release_classification_stamping_preserves_package_identity(self):
        import importlib.util
        import tempfile
        spec = importlib.util.spec_from_file_location("stamp", ROOT / "scripts/stamp-release-classification.py")
        stamp = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(stamp)
        for classification in ["GA", "Alpha", "Beta"]:
            with tempfile.TemporaryDirectory() as temporary:
                directory = Path(temporary)
                original = json.loads((ROOT / "src-tauri/distribution/direct-macos.json").read_text())
                (directory / "direct.json").write_text(json.dumps(original))
                stamp.stamp(directory, classification)
                self.assertEqual(json.loads((directory / "direct.json").read_text()), dict(original, releaseClassification=classification))
                with self.assertRaises(ValueError):
                    stamp.stamp(directory, "Unknown")


if __name__ == "__main__":
    unittest.main()
