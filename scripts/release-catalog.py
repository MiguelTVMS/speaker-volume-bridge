#!/usr/bin/env python3
"""Public release -> verified packages -> atomic, reviewable catalog pair.

No write to GitHub or hosting occurs in prepare mode. Network and package readers
are injected into the shared orchestration for offline regression coverage.
"""
import argparse
import hashlib
import importlib.util
import json
import plistlib
import re
import subprocess
import tempfile
from pathlib import Path
from urllib.request import urlopen

SPEC = importlib.util.spec_from_file_location('publication', Path(__file__).with_name('prepare-update-catalog.py'))
publication = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(publication)
REPOSITORY = 'MiguelTVMS/speaker-volume-bridge'
API = 'https://api.github.com/repos/' + REPOSITORY
ASSETS = {
    'speaker-volume-bridge-macos.dmg': ('direct_macos', 'macos', None),
    'speaker-volume-bridge-windows-x64-unsigned.exe': ('direct_windows', 'windows', 'x86_64'),
    'speaker-volume-bridge-windows-arm64-unsigned.exe': ('direct_windows', 'windows', 'aarch64'),
    'speaker-volume-bridge-linux-x64.deb': ('debian', 'linux', 'x86_64'),
    'speaker-volume-bridge-linux-arm64.deb': ('debian', 'linux', 'aarch64'),
}
FEEDS = ('v1', 'v2')


def fetch(url):
    if not url.startswith(('https://github.com/' + REPOSITORY + '/releases/', API + '/releases')):
        raise ValueError('unexpected public release location')
    with urlopen(url, timeout=120) as response:
        return response.read(512 * 1024 * 1024 + 1)


def release_metadata(tag):
    if not re.fullmatch(r'v[0-9A-Za-z.+-]+', tag):
        raise ValueError('invalid release tag')
    return publication._object(fetch(API + '/releases/tags/' + tag), 'public release')


def run(*args):
    return subprocess.check_output(args, text=True).strip()


def verify_provenance(raw, edition, classification):
    provenance = publication._object(raw, 'package provenance')
    if provenance.get('schemaVersion') != 1 or provenance.get('edition') != edition or provenance.get('publisher') != REPOSITORY:
        raise ValueError('package distribution provenance mismatch')
    stamped = provenance.get('releaseClassification')
    if stamped is not None and stamped != classification:
        raise ValueError('package classification conflicts with public release')


def inspect_package(path, target):
    """Read the installed payload, never the installer stub's architecture."""
    edition, _, expected_arch, classification = target
    with tempfile.TemporaryDirectory() as directory:
        root = Path(directory)
        if edition == 'direct_macos':
            mount = root / 'mount'
            run('hdiutil', 'attach', '-readonly', '-nobrowse', '-mountpoint', str(mount), str(path))
            try:
                apps = list(mount.glob('*.app'))
                if len(apps) != 1:
                    raise ValueError('expected one application in DMG')
                info = plistlib.loads((apps[0] / 'Contents/Info.plist').read_bytes())
                if info.get('CFBundleIdentifier') != 'ms.miguel.sonosvolumebridge.desktop':
                    raise ValueError('unexpected macOS package identity')
                verify_provenance((apps[0] / 'Contents/Resources/distribution.json').read_bytes(), edition, classification)
                binary = apps[0] / 'Contents/MacOS' / info['CFBundleExecutable']
                archs = run('lipo', '-archs', str(binary)).split()
                translated = {'arm64': 'aarch64', 'x86_64': 'x86_64'}
                if not archs or any(a not in translated for a in archs):
                    raise ValueError('unsupported macOS application architecture')
                return info['CFBundleShortVersionString'], [translated[a] for a in archs]
            finally:
                run('hdiutil', 'detach', str(mount))
        if edition == 'direct_windows':
            run('7zz', 'x', '-y', '-o' + str(root), str(path))
            binaries = list(root.rglob('speaker-volume-bridge.exe'))
            if len(binaries) != 1:
                raise ValueError('expected one Windows application payload')
            provenances = list(root.rglob('distribution.json'))
            if len(provenances) != 1:
                raise ValueError('missing Windows distribution provenance')
            verify_provenance(provenances[0].read_bytes(), edition, classification)
            import pefile
            pe = pefile.PE(str(binaries[0]))
            arch = {0x8664: 'x86_64', 0xAA64: 'aarch64'}.get(pe.FILE_HEADER.Machine)
            values = {}
            for group in getattr(pe, 'FileInfo', []):
                for item in group:
                    for table in getattr(item, 'StringTable', []):
                        values.update(table.entries)
            version = values.get(b'ProductVersion', b'').decode().strip()
            pe.close()
            if arch != expected_arch or not version:
                raise ValueError('Windows application identity/version unavailable')
            return version, [arch]
        import io
        import tarfile
        fields = dict(line.split(': ', 1) for line in run('dpkg-deb', '--field', str(path)).splitlines() if ': ' in line and not line.startswith(' '))
        data = subprocess.check_output(['dpkg-deb', '--fsys-tarfile', str(path)])
        arch = {'amd64': 'x86_64', 'arm64': 'aarch64'}.get(fields.get('Architecture'))
        with tarfile.open(fileobj=io.BytesIO(data)) as archive:
            members = [m for m in archive.getmembers() if m.name.lstrip('./') == 'usr/bin/speaker-volume-bridge' and m.isfile()]
            if len(members) != 1:
                raise ValueError('missing Debian application')
            elf = archive.extractfile(members[0]).read(20)
            provenances = [m for m in archive.getmembers() if m.name.endswith('/distribution.json') and m.isfile()]
            if len(provenances) != 1:
                raise ValueError('missing Debian distribution provenance')
            verify_provenance(archive.extractfile(provenances[0]).read(), edition, classification)
        machine = int.from_bytes(elf[18:20], 'little' if elf[5] == 1 else 'big')
        if fields.get('Package') != 'speaker-volume-bridge' or arch != expected_arch or elf[:5] != b'\x7fELF\x02' or {62: 'x86_64', 183: 'aarch64'}.get(machine) != arch:
            raise ValueError('Debian payload target mismatch')
        return fields['Version'], [arch]


def prepare_release(catalogs, release, download=fetch, inspect=inspect_package):
    """All packages are verified before either feed is changed."""
    if release.get('draft') is not False or not release.get('published_at'):
        raise ValueError('release is not publicly published')
    tag = release.get('tag_name', '')
    version = tag.removeprefix('v')
    publication._semver(version)
    markers = re.findall(r'^\*\*Release channel:\*\* (GA|Alpha|Beta)\s*$', release.get('body', ''), re.M)
    if len(markers) != 1 or release.get('prerelease') is not (markers[0] != 'GA'):
        raise ValueError('ambiguous release classification')
    classification = markers[0]
    assets = release.get('assets', [])
    named = {asset['name']: asset for asset in assets}
    if len(named) != len(assets) or not set(ASSETS) <= named.keys():
        raise ValueError('incomplete release package set')
    entries = []
    with tempfile.TemporaryDirectory() as directory:
        for name, target in ASSETS.items():
            asset = named[name]
            if asset.get('state') != 'uploaded' or not isinstance(asset.get('size'), int) or asset['size'] <= 0:
                raise ValueError('release asset is not public and complete')
            raw = download(asset['browser_download_url'])
            if len(raw) != asset['size'] or len(raw) > 512 * 1024 * 1024:
                raise ValueError('release asset length mismatch')
            digest = asset.get('digest')
            if digest and digest != 'sha256:' + hashlib.sha256(raw).hexdigest():
                raise ValueError('release asset digest mismatch')
            path = Path(directory) / name
            path.write_bytes(raw)
            actual_version, architectures = inspect(path, (*target, classification))
            if actual_version != version or not architectures or len(set(architectures)) != len(architectures) or any(a not in {'aarch64', 'x86_64'} for a in architectures):
                raise ValueError('package version or architecture mismatch')
            if target[2] and architectures != [target[2]]:
                raise ValueError('package architecture conflicts with release asset')
            for architecture in architectures:
                entries.append(dict(edition=target[0], os=target[1], architecture=architecture, version=version,
                    classification=classification, channel='stable' if classification == 'GA' else 'prereleases',
                    publishedAt=release['published_at'], releaseNotes='Public ' + classification + ' release ' + version + '.',
                    action={'type': 'open_url', 'url': 'https://github.com/' + REPOSITORY + '/releases/tag/' + tag}))
    output = dict(catalogs)
    for feed in FEEDS:
        if feed == 'v1' and classification != 'GA':
            continue
        for entry in entries:
            catalog = publication.VALIDATOR.validate_catalog_bytes(output[feed])
            fields = publication.TARGET_FIELDS + (('classification',) if feed == 'v2' else ())
            current = next((e for e in catalog['entries'] if all(e[f] == entry[f] for f in fields)), None)
            tombstones = catalog.get('withdrawals', [])
            if not isinstance(tombstones, list):
                raise ValueError('invalid withdrawal ledger')
            blocked = any(all(w.get(f) == entry[f] for f in fields) and publication._semver(version) <= publication._semver(w['version']) for w in tombstones)
            if blocked or (current and publication._semver(version) <= publication._semver(current['version'])):
                continue
            value = dict(current or {})
            value.update(entry)
            value['action'] = dict((current or {}).get('action', {}), **entry['action'])
            if feed == 'v1':
                value['action']['url'] = 'https://svb.miguel.ms/guide/Upgrading.html'
            record = dict(operation='upsert', verifiedAvailable=True, entry=value, source=dict(kind='github_release',
                draft=False, prerelease=release['prerelease'], public=True, releaseBody=release['body'], tagName=tag,
                requiredAssets=list(ASSETS), availableAssets=list(named)))
            output[feed] = publication.prepare_catalog(output[feed], json.dumps(record).encode(), release['published_at'])
    return output


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('tags', nargs='+')
    parser.add_argument('--root', type=Path, default=Path('.'))
    parser.add_argument('--output', type=Path, required=True, help='Review directory, never deployed by this command')
    args = parser.parse_args()
    catalogs = {f: (args.root / f'pages/updates/{f}/catalog.json').read_bytes() for f in FEEDS}
    for tag in args.tags:
        catalogs = prepare_release(catalogs, release_metadata(tag))
    for feed, raw in catalogs.items():
        publication._write_atomic(args.output / f'pages/updates/{feed}/catalog.json', raw)
    print('catalog-pending: verified review files prepared; approval and delivery remain separate')


if __name__ == '__main__':
    main()
