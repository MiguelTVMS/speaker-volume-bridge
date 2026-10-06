#!/usr/bin/env python3
"""Compose and verify complete Pages artifacts without rebuilding GA pages."""
import argparse
import hashlib
import importlib.util
import json
import shutil
import subprocess
import time
import zipfile
from pathlib import Path, PurePosixPath
from urllib.parse import quote, urlsplit
from urllib.request import Request, urlopen

SPEC = importlib.util.spec_from_file_location('validator', Path(__file__).with_name('validate-update-catalog.py'))
validator = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(validator)
CATALOGS = ('updates/v1/catalog.json', 'updates/v2/catalog.json')
MAX_BUNDLE = 90 * 1024 * 1024


def ga_source_changed(repository, revision, current):
    """Workflow-only promotion must retain the actual served GA page bytes."""
    if not revision:
        raise ValueError('missing published GA revision; exact bootstrap required')
    for ref in (revision, current):
        subprocess.run(['git', '-C', str(repository), 'rev-parse', '--verify', ref + '^{commit}'],
                       check=True, stdout=subprocess.DEVNULL)
    result = subprocess.run(['git', '-C', str(repository), 'diff', '--quiet', revision, current,
                             '--', '.', ':(exclude).github/workflows/**'])
    if result.returncode not in (0, 1):
        raise ValueError('cannot compare approved GA source')
    return result.returncode == 1


def files(root):
    result = {}
    for path in sorted(root.rglob('*')):
        if path.is_symlink():
            raise ValueError('website snapshots cannot contain links')
        if path.is_file():
            result[path.relative_to(root).as_posix()] = path.read_bytes()
    if not result or 'index.html' not in result:
        raise ValueError('missing retained GA website; bootstrap or exact recovery required')
    if sum(map(len, result.values())) > MAX_BUNDLE:
        raise ValueError('website exceeds durable Git snapshot limit; provision approved storage')
    return result


def manifest(contents, exclude_catalogs=False):
    return {p: hashlib.sha256(raw).hexdigest() for p, raw in contents.items() if not exclude_catalogs or p not in CATALOGS}


def retain(root, storage, revision):
    contents = files(root)
    identity = hashlib.sha256(json.dumps(manifest(contents), sort_keys=True).encode()).hexdigest()
    archive = storage / 'snapshots' / (identity + '.zip')
    archive.parent.mkdir(parents=True, exist_ok=True)
    if not archive.exists():
        with zipfile.ZipFile(archive, 'w', zipfile.ZIP_DEFLATED) as output:
            for name, raw in contents.items():
                output.writestr(name, raw)
    reference = {'archive': archive.relative_to(storage).as_posix(), 'files': manifest(contents), 'revision': revision}
    # Verify even an already present immutable object before trusting it.
    restore(storage, reference, storage / '.verify-snapshot')
    shutil.rmtree(storage / '.verify-snapshot')
    return reference


def restore(storage, reference, destination):
    archive_path = PurePosixPath(reference['archive'])
    if archive_path.is_absolute() or '..' in archive_path.parts or archive_path.parts[:1] != ('snapshots',):
        raise ValueError('invalid retained archive reference')
    contents = {}
    with zipfile.ZipFile(storage / str(archive_path)) as archive:
        total = 0
        for item in archive.infolist():
            name = PurePosixPath(item.filename)
            total += item.file_size
            if name.is_absolute() or '..' in name.parts or item.filename in contents or item.is_dir() or total > MAX_BUNDLE:
                raise ValueError('invalid retained archive')
            contents[item.filename] = archive.read(item)
    if manifest(contents) != reference['files']:
        raise ValueError('retained website snapshot failed integrity verification')
    if destination.exists():
        shutil.rmtree(destination)
    destination.mkdir(parents=True)
    for name, raw in contents.items():
        path = destination / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(raw)
    files(destination)


def compose(website, catalogs, destination):
    contents = files(website)
    originals = manifest(contents, exclude_catalogs=True)
    for path in CATALOGS:
        raw = (catalogs / path).read_bytes()
        validator.validate_catalog_bytes(raw)
        contents[path] = raw
    if destination.exists():
        shutil.rmtree(destination)
    for name, raw in contents.items():
        path = destination / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(raw)
    if manifest(files(destination), exclude_catalogs=True) != originals:
        raise ValueError('composition changed non-catalog GA bytes')
    return manifest(contents)


def guard(expected, latest):
    for path in CATALOGS:
        if (expected / path).read_bytes() != (latest / path).read_bytes():
            raise ValueError('approved catalogs advanced; retry with current develop')


def http_read(url, refresh=False):
    request = Request(url, headers={'Cache-Control': 'no-cache'} if refresh else {})
    with urlopen(request, timeout=30) as response:
        return response.read(MAX_BUNDLE + 1), response.headers


def verify_served(root, base_url, read=http_read, attempts=6, pause=time.sleep, all_files=False):
    if urlsplit(base_url).scheme != 'https':
        raise ValueError('served verification requires HTTPS')
    contents = files(root)
    selected = contents if all_files else {p: contents[p] for p in CATALOGS}
    last_error = None
    for attempt in range(attempts):
        try:
            for name, expected in selected.items():
                url = base_url.rstrip('/') + '/' + quote(name)
                for refresh in (False, True):
                    raw, headers = read(url, refresh)
                    if raw != expected:
                        raise ValueError('served content is stale or differs from intended snapshot')
                    if name in CATALOGS:
                        validator.validate_catalog_bytes(raw)
                        if not headers.get('Cache-Control'):
                            raise ValueError('served catalog has no cache policy')
            return
        except (OSError, ValueError, validator.CatalogError) as error:
            last_error = error
            if attempt + 1 < attempts:
                pause(10)
    raise ValueError('catalog-live unverified; retry delivery/verification: ' + str(last_error))


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('operation', choices=['retain', 'restore', 'compose', 'guard', 'verify', 'source-mode'])
    parser.add_argument('--website', type=Path)
    parser.add_argument('--catalogs', type=Path)
    parser.add_argument('--destination', type=Path)
    parser.add_argument('--storage', type=Path)
    parser.add_argument('--revision')
    parser.add_argument('--repository', type=Path)
    parser.add_argument('--current')
    parser.add_argument('--url')
    parser.add_argument('--all-files', action='store_true')
    args = parser.parse_args()
    state_path = args.storage / 'state.json' if args.storage else None
    if args.operation == 'source-mode':
        print('ga' if ga_source_changed(args.repository, args.revision, args.current) else 'catalog')
    elif args.operation == 'retain':
        reference = retain(args.website, args.storage, args.revision)
        state = json.loads(state_path.read_text()) if state_path.exists() else {}
        state['candidate'] = reference
        state_path.write_text(json.dumps(state, indent=2) + '\n')
    elif args.operation == 'restore':
        state = json.loads(state_path.read_text())
        # Never replace a pending/unverified GA deployment with an older bundle.
        # Operator retries or verifies that candidate before catalog delivery.
        if 'candidate' in state:
            raise ValueError('unverified GA candidate requires explicit GA recovery/verification')
        restore(args.storage, state['published'], args.destination)
    elif args.operation == 'compose':
        compose(args.website, args.catalogs, args.destination)
    elif args.operation == 'guard':
        guard(args.catalogs, args.destination)
    else:
        verify_served(args.website, args.url, all_files=args.all_files)
        if state_path:
            state = json.loads(state_path.read_text())
            if 'candidate' in state:
                # Catalog overlay may differ from the retained GA source. Only
                # non-catalog bytes identify the immutable website bundle.
                actual = manifest(files(args.website), exclude_catalogs=True)
                expected = {p: d for p, d in state['candidate']['files'].items() if p not in CATALOGS}
                if actual != expected:
                    raise ValueError('verified artifact does not match retained GA candidate')
                state['published'] = state.pop('candidate')
            state['served'] = manifest(files(args.website))
            state_path.write_text(json.dumps(state, indent=2) + '\n')
        print('catalog-live: both served feeds match intended bytes and cache refresh')


if __name__ == '__main__':
    main()
