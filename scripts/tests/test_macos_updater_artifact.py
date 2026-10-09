"""Post-processing packaging regressions. Fixtures do not prove Apple acceptance."""
import importlib.util
import json
import os
from pathlib import Path
import plistlib
import subprocess
import tarfile
import tempfile
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[2]
spec = importlib.util.spec_from_file_location('macos_updater', ROOT / 'scripts/macos-updater-artifact.py')
updater = importlib.util.module_from_spec(spec)
spec.loader.exec_module(updater)


def make_app(root):
    app = root / 'Speaker Volume Bridge.app'
    (app / 'Contents/MacOS').mkdir(parents=True)
    (app / 'Contents/Resources').mkdir()
    (app / 'Contents/Info.plist').write_bytes(plistlib.dumps(dict(CFBundleIdentifier=updater.IDENTIFIER,
        CFBundleShortVersionString='2.0.0', CFBundleExecutable='speaker-volume-bridge')))
    executable = app / 'Contents/MacOS/speaker-volume-bridge'
    executable.write_bytes(b'fixture executable')
    executable.chmod(0o755)
    (app / 'Contents/Resources/distribution.json').write_text(json.dumps(dict(schemaVersion=1, edition='direct_macos', publisher='MiguelTVMS/speaker-volume-bridge')))
    (app / 'Contents/Resources/ticket').write_bytes(b'final stapled content fixture')
    return app


class MacosUpdaterTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.app = make_app(self.root)
        self.output = self.root / 'export'

    def inspect(self):
        with patch.object(updater.subprocess, 'check_output', return_value='arm64\n'):
            updater.inspect_bundle(self.app, '2.0.0')

    def test_bundle_version_and_identity_bind_release(self):
        self.inspect()
        for field, invalid in [('CFBundleIdentifier', 'other'), ('CFBundleShortVersionString', '1.0.0')]:
            path = self.app / 'Contents/Info.plist'
            original = path.read_bytes()
            info = plistlib.loads(original)
            info[field] = invalid
            path.write_bytes(plistlib.dumps(info))
            with self.assertRaises(ValueError):
                self.inspect()
            path.write_bytes(original)

    def test_store_custom_and_unknown_provenance_rejected(self):
        path = self.app / 'Contents/Resources/distribution.json'
        original = json.loads(path.read_text())
        for field, values in [('edition', ['mac_app_store', 'custom', 'direct_windows']), ('publisher', ['other']), ('schemaVersion', [2])]:
            for value in values:
                path.write_text(json.dumps(original | {field: value}))
                with self.assertRaises(ValueError):
                    self.inspect()
        path.write_text(json.dumps(original))
        (self.app / 'Contents/_MASReceipt').mkdir()
        with self.assertRaises(ValueError):
            self.inspect()

    def test_release_classification_is_additive_but_not_arbitrary(self):
        path = self.app / 'Contents/Resources/distribution.json'
        original = json.loads(path.read_text())
        for value in ['GA', 'Alpha', 'Beta']:
            path.write_text(json.dumps(original | {'releaseClassification': value}))
            self.inspect()
        path.write_text(json.dumps(original | {'releaseClassification': 'other'}))
        with self.assertRaises(ValueError):
            self.inspect()

    def test_missing_executable_and_wrong_architecture_fail(self):
        for architectures in ['x86_64', 'arm64 x86_64']:
            with patch.object(updater.subprocess, 'check_output', return_value=architectures):
                with self.assertRaises(ValueError):
                    updater.inspect_bundle(self.app, '2.0.0')
        (self.app / 'Contents/MacOS/speaker-volume-bridge').unlink()
        with self.assertRaises(ValueError):
            self.inspect()

    def archive(self):
        archive = self.root / 'payload.app.tar.gz'
        with tarfile.open(archive, 'w:gz') as bundle:
            bundle.add(self.app, arcname=self.app.name)
        return archive

    def test_archive_preserves_final_bytes_permissions_and_links(self):
        (self.app / 'Contents/Resources/link').symlink_to('ticket')
        archive = self.archive()
        updater.verify_archive(archive, self.app)
        ticket = self.app / 'Contents/Resources/ticket'
        ticket.write_bytes(b'changed after approval')
        with self.assertRaises(ValueError):
            updater.verify_archive(archive, self.app)

    def test_archive_rejects_changed_executable_permissions(self):
        archive = self.archive()
        (self.app / 'Contents/MacOS/speaker-volume-bridge').chmod(0o644)
        with self.assertRaises(ValueError):
            updater.verify_archive(archive, self.app)

    def test_archive_rejects_missing_duplicate_foreign_entries_and_escaping_links(self):
        for name in ['../escape', self.app.name + '/Contents/Resources/ticket']:
            archive = self.archive()
            with tarfile.open(archive, 'r:gz') as source:
                entries = [(item, source.extractfile(item).read() if item.isfile() else None) for item in source]
            import io
            with tarfile.open(archive, 'w:gz') as bundle:
                for item, data in entries:
                    bundle.addfile(item, io.BytesIO(data) if data is not None else None)
                bundle.addfile(tarfile.TarInfo(name))
            with self.assertRaises(ValueError):
                updater.verify_archive(archive, self.app)
        archive = self.archive()
        (self.app / 'Contents/Resources/new').write_text('missing from archive')
        with self.assertRaises(ValueError):
            updater.verify_archive(archive, self.app)
        (self.app / 'Contents/Resources/escape').symlink_to('/tmp')
        with self.assertRaises(ValueError):
            updater.verify_archive(self.archive(), self.app)

    def test_native_approval_signer_and_verifier_failures_export_nothing(self):
        real_run = subprocess.run
        for fail in ['verify-macos-artifact.sh', 'stapler', 'spctl', 'signer', 'verifier']:
            def run(command, **kwargs):
                if any(fail in str(arg) for arg in command):
                    raise subprocess.CalledProcessError(1, command)
                if command[0] == 'tar':
                    return real_run(command, **kwargs)
                if 'signer' in command:
                    Path(command[-1] + '.sig').write_text('fixture signature')
                return subprocess.CompletedProcess(command, 0)
            with patch.object(updater.subprocess, 'check_output', return_value='arm64'), patch.object(updater.subprocess, 'run', side_effect=run), patch.dict(os.environ, TAURI_SIGNING_PRIVATE_KEY='fixture'):
                with self.assertRaises(subprocess.CalledProcessError):
                    updater.package(self.app, '2.0.0', self.output, self.root / 'key.pub', Path('verifier'))
            self.assertFalse(self.output.exists() and list(self.output.iterdir()))

    @unittest.skipUnless(os.environ.get('UPDATER_SIGNATURE_INTEGRATION') == '1', 'real Tauri CLI signature integration runs in the macOS packaging CI job')
    def test_real_tauri_signature_round_trip_tampering_and_wrong_key(self):
        verifier = ROOT / 'target/debug/speaker-volume-bridge-updater-artifact'
        public_keys = []
        for name in ['one', 'two']:
            key = self.root / name
            subprocess.run(updater.CLI + ['signer', 'generate', '--ci', '-p', '', '-w', str(key)], check=True, stdout=subprocess.DEVNULL)
            public_keys.append(Path(str(key) + '.pub'))
        real_run = subprocess.run
        def native_fixture(command, **kwargs):
            if command[0] in ['bash', 'xcrun', 'spctl']:
                return subprocess.CompletedProcess(command, 0)
            return real_run(command, **kwargs)
        with patch.object(updater.subprocess, 'check_output', return_value='arm64'), patch.object(updater.subprocess, 'run', side_effect=native_fixture), patch.dict(os.environ, TAURI_SIGNING_PRIVATE_KEY_PATH=str(self.root / 'one'), TAURI_SIGNING_PRIVATE_KEY_PASSWORD=''):
            updater.package(self.app, '2.0.0', self.output, public_keys[0], verifier)
        descriptor = json.loads(next(self.output.glob('*.json')).read_text())
        self.assertFalse(descriptor['installCapability'])
        self.assertEqual(descriptor['fallback'], 'open_url')
        archive = self.output / descriptor['artifact']
        signature = self.output / descriptor['signature']
        self.assertNotEqual(real_run([str(verifier), str(archive), str(signature), str(public_keys[1])], capture_output=True).returncode, 0)
        archive.write_bytes(archive.read_bytes() + b'tampered')
        self.assertNotEqual(real_run([str(verifier), str(archive), str(signature), str(public_keys[0])], capture_output=True).returncode, 0)


if __name__ == '__main__':
    unittest.main()
