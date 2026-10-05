#!/usr/bin/env python3
"""Adopt the actual served GA artifact, with no hosting mutation."""
import argparse
import importlib.util
import json
import subprocess
import tarfile
from pathlib import Path, PurePosixPath

SPEC = importlib.util.spec_from_file_location('site', Path(__file__).with_name('compose-catalog-site.py'))
site = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(site)


def extract(archive_path, destination):
    destination.mkdir(parents=True, exist_ok=True)
    seen = set()
    total = 0
    with tarfile.open(archive_path) as archive:
        for item in archive:
            name = PurePosixPath(item.name)
            if name.is_absolute() or '..' in name.parts or item.issym() or item.islnk():
                raise ValueError('unsafe website archive')
            if item.isdir():
                continue
            total += item.size
            if not item.isfile() or str(name) in seen or total > site.MAX_BUNDLE:
                raise ValueError('invalid website archive')
            seen.add(str(name))
            path = destination / name
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_bytes(archive.extractfile(item).read())
    site.files(destination)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--run', required=True)
    parser.add_argument('--url', required=True)
    parser.add_argument('--destination', type=Path, required=True)
    parser.add_argument('--download', type=Path, required=True)
    args = parser.parse_args()
    if not args.run.isdigit():
        raise ValueError('expected successful GA workflow run')
    run = json.loads(subprocess.check_output(['gh', 'api', 'repos/MiguelTVMS/speaker-volume-bridge/actions/runs/' + args.run]))
    if run['conclusion'] != 'success' or run['head_branch'] != 'main' or run['path'] != '.github/workflows/pages.yml':
        raise ValueError('bootstrap requires a successful main website workflow')
    subprocess.run(['gh', 'run', 'download', args.run, '--repo', 'MiguelTVMS/speaker-volume-bridge', '--name', 'github-pages', '--dir', str(args.download)], check=True)
    extract(args.download / 'artifact.tar', args.destination)
    site.verify_served(args.destination, args.url, all_files=True)
    print(run['head_sha'])


if __name__ == '__main__':
    main()
