#!/usr/bin/env python3
"""Package the final direct bundle, sign it with Tauri, verify before exporting.

No discovery, installation, catalog, or release publication occurs here.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import plistlib
import re
import subprocess
import tarfile
import tempfile

ROOT = Path(__file__).resolve().parents[1]
IDENTIFIER = json.loads((ROOT / 'src-tauri/tauri.conf.json').read_text())['identifier']
CLI = ['pnpm', 'dlx', '@tauri-apps/cli@2.11.3']


def inspect_bundle(app, version):
    if not re.fullmatch(r'\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?', version):
        raise ValueError('Invalid release version')
    if app.is_symlink() or app.name != 'Speaker Volume Bridge.app':
        raise ValueError('Expected the official application bundle')
    info = plistlib.loads((app / 'Contents/Info.plist').read_bytes())
    if info.get('CFBundleIdentifier') != IDENTIFIER:
        raise ValueError('Unexpected application identity')
    if info.get('CFBundleShortVersionString') != version:
        raise ValueError('Bundle and release versions differ')
    provenance = json.loads((app / 'Contents/Resources/distribution.json').read_text())
    classification = provenance.pop('releaseClassification', None)
    expected = dict(schemaVersion=1, edition='direct_macos', publisher='MiguelTVMS/speaker-volume-bridge')
    if provenance != expected or classification not in (None, 'GA', 'Alpha', 'Beta'):
        raise ValueError('Not an official direct macOS package')
    if (app / 'Contents/_MASReceipt').exists():
        raise ValueError('Store bundles cannot produce direct updater artifacts')
    executable = app / 'Contents/MacOS' / info['CFBundleExecutable']
    if not executable.is_file() or not os.access(executable, os.X_OK):
        raise ValueError('Missing executable')
    # lipo checks the compiled binary, never the runner architecture or filename.
    if subprocess.check_output(['lipo', '-archs', str(executable)], text=True).strip() != 'arm64':
        raise ValueError('Only the official ARM64 direct target is supported')


def verify_archive(archive, app):
    """Compare every entry to the approved source including modes and symlinks."""
    expected = {app.name: app}
    expected.update({str(p.relative_to(app.parent)): p for p in app.rglob('*')})
    seen = set()
    with tarfile.open(archive, 'r:gz') as bundle:
        for entry in bundle:
            name = entry.name.rstrip('/')
            if name not in expected or name in seen:
                raise ValueError('Unexpected or duplicate archive entry')
            seen.add(name)
            source = expected[name]
            if entry.mode != source.lstat().st_mode & 0o7777:
                raise ValueError('Archive changed entry permissions')
            if source.is_symlink():
                if not entry.issym() or entry.linkname != os.readlink(source):
                    raise ValueError('Archive changed symlink')
                resolved = source.resolve()
                if not resolved.is_relative_to(app.resolve()):
                    raise ValueError('Symlink leaves the application bundle')
            elif source.is_dir():
                if not entry.isdir():
                    raise ValueError('Archive changed directory')
            elif source.is_file():
                if not entry.isfile() or bundle.extractfile(entry).read() != source.read_bytes():
                    raise ValueError('Archive changed bundle bytes')
            else:
                raise ValueError('Unsupported bundle entry')
    if seen != set(expected):
        raise ValueError('Incomplete updater archive')


def package(app, version, output, public_key, verifier):
    inspect_bundle(app, version)
    app = app.resolve()
    # All native approval checks must pass before any payload can be exported.
    subprocess.run(['bash', str(ROOT / 'scripts/verify-macos-artifact.sh'), 'direct', str(app)], check=True)
    subprocess.run(['xcrun', 'stapler', 'validate', str(app)], check=True)
    subprocess.run(['spctl', '--assess', '--type', 'execute', str(app)], check=True)
    if not os.environ.get('TAURI_SIGNING_PRIVATE_KEY') and not os.environ.get('TAURI_SIGNING_PRIVATE_KEY_PATH'):
        raise ValueError('Updater signing key is required')
    output.mkdir(parents=True, exist_ok=True)
    names = ['speaker-volume-bridge-macos-aarch64.app.tar.gz', 'speaker-volume-bridge-macos-aarch64.app.tar.gz.sig', 'speaker-volume-bridge-macos-aarch64.updater.json']
    if any((output / name).exists() for name in names):
        raise ValueError('Refusing to overwrite an existing updater artifact')
    # Stage on the output volume. A failed signer/verifier exports nothing.
    with tempfile.TemporaryDirectory(dir=output) as stage:
        archive = Path(stage) / names[0]
        # Match Tauri's single .app-root tar format, without AppleDouble sidecars.
        subprocess.run(['tar', '-czf', str(archive), '-C', str(app.parent), app.name], check=True,
                       env=dict(os.environ, COPYFILE_DISABLE='1'))
        verify_archive(archive, app)
        # Tauri CLI writes a base64-encoded minisign detached signature.
        subprocess.run(CLI + ['signer', 'sign', str(archive)], check=True, stdout=subprocess.DEVNULL)
        signature = Path(str(archive) + '.sig')
        subprocess.run([str(verifier), str(archive), str(signature), str(public_key)], check=True)
        # The signature binds version/identity/provenance through the bundle bytes.
        # This is an audit descriptor, deliberately not a discoverable install offer.
        descriptor = dict(schemaVersion=1, version=version, edition='direct_macos', target='darwin-aarch64',
                          artifact=names[0], signature=names[1], sha256=hashlib.sha256(archive.read_bytes()).hexdigest(),
                          installCapability=False, nativeAcceptance='pending', fallback='open_url')
        (Path(stage) / names[2]).write_text(json.dumps(descriptor, indent=2) + '\n')
        for name in names:
            (Path(stage) / name).rename(output / name)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('app', type=Path)
    parser.add_argument('version')
    parser.add_argument('output', type=Path)
    parser.add_argument('--public-key', required=True, type=Path)
    parser.add_argument('--verifier', type=Path, default=ROOT / 'target/release/speaker-volume-bridge-updater-artifact')
    args = parser.parse_args()
    package(args.app, args.version, args.output, args.public_key, args.verifier)


if __name__ == '__main__':
    main()
