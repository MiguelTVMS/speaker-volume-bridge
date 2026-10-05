#!/usr/bin/env python3
"""Shared release-completion/recovery entry point; never merge a proposal."""
import importlib.util
import json
import os
import subprocess
from pathlib import Path

SPEC = importlib.util.spec_from_file_location('release_catalog', Path(__file__).with_name('release-catalog.py'))
release_catalog = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(release_catalog)
REPO = release_catalog.REPOSITORY
PATHS = [f'pages/updates/{f}/catalog.json' for f in release_catalog.FEEDS]


def command(*args):
    return subprocess.check_output(args, text=True).strip()


def propose(tag, invoke=command, metadata=release_catalog.release_metadata, prepare=release_catalog.prepare_release):
    # Validate untrusted input before using it as a ref or command argument.
    release = metadata(tag)
    release_catalog.publication._semver(tag.removeprefix('v'))
    branch = 'chore/catalog-' + tag.replace('+', '-')
    for attempt in range(3):
        invoke('git', 'fetch', 'origin', 'develop')
        base = invoke('git', 'rev-parse', 'origin/develop')
        prs = json.loads(invoke('gh', 'pr', 'list', '--repo', REPO, '--head', branch, '--base', 'develop', '--state', 'open', '--json', 'number'))
        if prs:
            invoke('git', 'fetch', 'origin', branch)
            invoke('git', 'checkout', '-B', branch, 'FETCH_HEAD')
            # Normal merge/push preserves reviewer history. Reprepare both files
            # against develop, rather than trusting conflict resolution of JSON.
            invoke('git', 'merge', '--no-edit', '-X', 'theirs', 'origin/develop')
            invoke('git', 'checkout', 'origin/develop', '--', *PATHS)
        else:
            invoke('git', 'checkout', '-B', branch, 'origin/develop')
        catalogs = {f: Path(p).read_bytes() for f, p in zip(release_catalog.FEEDS, PATHS)}
        result = prepare(catalogs, release)
        for feed, path in zip(release_catalog.FEEDS, PATHS):
            release_catalog.publication._write_atomic(Path(path), result[feed])
        invoke('git', 'fetch', 'origin', 'develop')
        if invoke('git', 'rev-parse', 'origin/develop') != base:
            invoke('git', 'restore', '--source=HEAD', '--staged', '--worktree', '--', *PATHS)
            continue
        # Ensure the proposal contains no stale application changes.
        changed = invoke('git', 'diff', '--name-only', 'origin/develop').splitlines()
        if any(p not in PATHS for p in changed):
            raise ValueError('proposal must contain only catalogs')
        if not changed:
            if prs:
                invoke('gh', 'pr', 'close', str(prs[0]['number']), '--repo', REPO)
            return 'catalog-pending: no new verified entries; live state is checked by delivery'
        invoke('git', 'add', *PATHS)
        staged = invoke('git', 'diff', '--cached', '--name-only')
        if staged:
            invoke('git', 'commit', '-m', 'chore: prepare verified release catalogs')
        invoke('git', 'push', 'origin', 'HEAD:refs/heads/' + branch)
        if not prs:
            invoke('gh', 'pr', 'create', '--repo', REPO, '--base', 'develop', '--head', branch,
                   '--title', 'chore: prepare verified release catalogs', '--label', 'enhancement', '--label', 'pages',
                   '--body', 'Verified public direct packages prepare the applicable feeds. Store entries are preserved. Approval and required checks remain necessary before independent catalog delivery. Refs #183, #159.')
        return 'release-published / catalog-pending: review PR prepared; no merge or deployment performed'
    raise ValueError('develop advanced repeatedly; retry catalog preparation')


def reconcile(invoke=command):
    # GitHub concurrency may replace pending events. A periodic public-release
    # sweep recovers them using the exact same independently verified entry point.
    pages = json.loads(invoke('gh', 'api', 'repos/' + REPO + '/releases', '--paginate', '--slurp'))
    candidates = [r for page in pages for r in page if r.get('draft') is False
                  and set(release_catalog.ASSETS) <= {a['name'] for a in r.get('assets', [])}]
    candidates.sort(key=lambda r: release_catalog.publication._semver(r['tag_name'].removeprefix('v')))
    failures = []
    for release in candidates:
        try:
            print(propose(release['tag_name']))
        except (ValueError, OSError, subprocess.CalledProcessError) as error:
            # No public diagnostics or identifying metadata in PR descriptions.
            print('catalog-pending: a public release requires operator retry')
            failures.append(error)
    if failures:
        raise ValueError('catalog reconciliation incomplete; inspect private workflow diagnostics and retry')


if __name__ == '__main__':
    import argparse
    parser = argparse.ArgumentParser()
    parser.add_argument('--reconcile', action='store_true')
    args = parser.parse_args()
    if args.reconcile:
        reconcile()
    else:
        print(propose(os.environ['RELEASE_TAG']))
