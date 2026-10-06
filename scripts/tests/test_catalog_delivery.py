"""Production entry points and release/hosting lifecycle regressions."""
import hashlib
import importlib.util
import json
import subprocess
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]


def load(name):
    spec = importlib.util.spec_from_file_location(name, ROOT / 'scripts' / (name + '.py'))
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


release = load('release-catalog')
site = load('compose-catalog-site')
proposer = load('propose-release-catalog')
STAMP = '2026-10-05T12:00:00Z'


def feeds():
    return {f: json.dumps({'schemaVersion': int(f[1:]), 'generatedAt': STAMP, 'futureCatalog': {'preserve': True}, 'entries': []}).encode() for f in release.FEEDS}


def published(version='1.8.0', classification='GA'):
    return {'tag_name': 'v' + version, 'draft': False, 'prerelease': classification != 'GA', 'published_at': STAMP,
            'body': '**Release channel:** ' + classification,
            'assets': [{'name': n, 'state': 'uploaded', 'size': 7, 'browser_download_url': n,
                        'digest': 'sha256:' + hashlib.sha256(b'package').hexdigest()} for n in release.ASSETS]}


def prepare(catalogs, metadata):
    return release.prepare_release(catalogs, metadata, lambda _: b'package',
        lambda path, target: (metadata['tag_name'][1:], [target[2] or 'aarch64']))


class OrchestrationTests(unittest.TestCase):
    def test_ga_both_feeds_beta_alpha_preview_only_supported_payload_targets(self):
        original = feeds()
        ga = prepare(original, published())
        for raw in ga.values():
            value = json.loads(raw)
            self.assertEqual(len(value['entries']), 5)
            self.assertTrue(value['futureCatalog']['preserve'])
            self.assertEqual([e['architecture'] for e in value['entries'] if e['edition'] == 'direct_macos'], ['aarch64'])
        for classification in ('Alpha', 'Beta'):
            preview = prepare(ga, published('1.9.0', classification))
            self.assertEqual(preview['v1'], ga['v1'])
            self.assertEqual(len(json.loads(preview['v2'])['entries']), 10)
            self.assertEqual(prepare(preview, published('1.9.0', classification)), preview)
        self.assertEqual(prepare(ga, published('1.7.0')), ga)

    def test_incomplete_failed_draft_or_mismatched_packages_make_no_write(self):
        for field, bad in (('draft', True), ('published_at', None), ('prerelease', True), ('assets', []), ('body', 'unknown')):
            metadata = published()
            metadata[field] = bad
            with self.subTest(field=field), self.assertRaises((ValueError, KeyError)):
                prepare(feeds(), metadata)
        for bad in (0, 1):
            metadata = published()
            metadata['assets'][0]['size'] = bad
            with self.assertRaises(ValueError):
                prepare(feeds(), metadata)
        for inspected in [('1.7.0', ['aarch64']), ('1.8.0', ['unsupported']), ('1.8.0', [])]:
            with self.assertRaises(ValueError):
                release.prepare_release(feeds(), published(), lambda _: b'package', lambda *_: inspected)
        metadata = published()
        metadata['assets'][0]['digest'] = 'sha256:' + '0' * 64
        with self.assertRaises(ValueError):
            prepare(feeds(), metadata)

    def test_latest_base_preserves_store_unrelated_metadata_and_withdrawals(self):
        ga = prepare(feeds(), published())
        stable = json.loads(ga['v1'])
        windows = next(e for e in stable['entries'] if e['edition'] == 'direct_windows')
        store = dict(windows, edition='microsoft_store', version='1.6.0')
        store['action'] = {'type': 'open_url', 'url': 'https://example.com/store'}
        stable['entries'].append(store)
        windows['futureEntry'] = {'preserve': True}
        windows['action']['futureAction'] = {'preserve': True}
        ga['v1'] = json.dumps(stable).encode()
        upgraded = prepare(ga, published('1.9.0'))
        entries = json.loads(upgraded['v1'])['entries']
        self.assertIn(store, entries)
        replaced = next(e for e in entries if e['edition'] == windows['edition'] and e['architecture'] == windows['architecture'])
        self.assertEqual(replaced['futureEntry'], windows['futureEntry'])
        self.assertEqual(replaced['action']['futureAction'], windows['action']['futureAction'])
        target = {f: replaced[f] for f in release.publication.TARGET_FIELDS}
        withdrawn = release.publication.prepare_catalog(upgraded['v1'], json.dumps({'operation': 'withdraw', 'target': target, 'reason': 'Withdrawn package'}).encode(), STAMP)
        pair = dict(upgraded, v1=withdrawn)
        self.assertEqual(prepare(pair, published('1.9.0')), pair)
        self.assertEqual(prepare(pair, published('1.8.0')), pair)
        with self.assertRaises(release.publication.PublicationError):
            release.publication.prepare_catalog(withdrawn, json.dumps({'operation': 'upsert', 'verifiedAvailable': True,
              'source': {'kind': 'github_release', 'draft': False, 'prerelease': False, 'public': True, 'releaseBody': '**Release channel:** GA', 'tagName': 'v1.9.0', 'requiredAssets': ['package'], 'availableAssets': ['package']}, 'entry': replaced}).encode(), STAMP)
        self.assertEqual(len(json.loads(prepare(pair, published('2.0.0'))['v1'])['entries']), 6)

    def test_explicit_withdrawal_cutoff_blocks_backfill_even_when_offer_already_absent(self):
        pair = feeds()
        target = {'edition': 'direct_macos', 'os': 'macos', 'architecture': 'aarch64', 'channel': 'stable'}
        record = json.dumps({'operation': 'withdraw', 'target': target, 'version': '1.8.0', 'reason': 'Withdrawn before backfill'}).encode()
        pair['v1'] = release.publication.prepare_catalog(pair['v1'], record, STAMP)
        self.assertEqual(release.publication.prepare_catalog(pair['v1'], record, STAMP), pair['v1'])
        result = prepare(pair, published())
        self.assertEqual(len(json.loads(result['v1'])['entries']), 4)
        self.assertEqual(json.loads(result['v1'])['withdrawals'], [{**target, 'version': '1.8.0'}])

    def test_backfill_and_out_of_order_distinct_classifications_converge(self):
        a, b = feeds(), feeds()
        sequence = [published('1.8.0'), published('1.7.4', 'Beta'), published('1.9.0', 'Alpha')]
        for metadata in sequence:
            a = prepare(a, metadata)
        for metadata in reversed(sequence):
            b = prepare(b, metadata)
        self.assertEqual({k: json.loads(v)['entries'] for k, v in a.items()}, {k: json.loads(v)['entries'] for k, v in b.items()})
        for metadata in sequence:
            a = prepare(a, metadata)
        self.assertEqual(a, b)

    def test_stamped_package_classification_and_distribution_must_match(self):
        provenance = {'schemaVersion': 1, 'publisher': release.REPOSITORY, 'edition': 'direct_macos', 'releaseClassification': 'Beta'}
        release.verify_provenance(json.dumps(provenance).encode(), 'direct_macos', 'Beta')
        for edition, classification in [('direct_windows', 'Beta'), ('direct_macos', 'GA')]:
            with self.assertRaises(ValueError):
                release.verify_provenance(json.dumps(provenance).encode(), edition, classification)

    def test_real_debian_package_reader_rejects_wrong_architecture_and_provenance(self):
        import shutil
        if not shutil.which('dpkg-deb'):
            self.skipTest('dpkg-deb required by production Linux CI')
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            package = root / 'package'
            control = package / 'DEBIAN/control'
            control.parent.mkdir(parents=True)
            control.write_text('Package: speaker-volume-bridge\nVersion: 1.8.0\nArchitecture: amd64\nMaintainer: Test <test@example.invalid>\nDescription: Test payload\n')
            binary = package / 'usr/bin/speaker-volume-bridge'
            binary.parent.mkdir(parents=True)
            elf = bytearray(20)
            elf[:6] = b'\x7fELF\x02\x01'
            elf[18:20] = (62).to_bytes(2, 'little')
            binary.write_bytes(elf)
            provenance = package / 'usr/lib/speaker-volume-bridge/distribution.json'
            provenance.parent.mkdir(parents=True)
            provenance.write_text(json.dumps({'schemaVersion': 1, 'publisher': release.REPOSITORY, 'edition': 'debian', 'releaseClassification': 'GA'}))
            archive = root / 'test.deb'
            subprocess.run(['dpkg-deb', '--build', '--root-owner-group', str(package), str(archive)], check=True, capture_output=True)
            self.assertEqual(release.inspect_package(archive, ('debian', 'linux', 'x86_64', 'GA')), ('1.8.0', ['x86_64']))
            for target in [('debian', 'linux', 'aarch64', 'GA'), ('debian', 'linux', 'x86_64', 'Beta')]:
                with self.assertRaises(ValueError):
                    release.inspect_package(archive, target)

    def test_real_git_proposals_preserve_concurrent_targets_on_retry_without_force(self):
        import os
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            remote, checkout = root / 'remote.git', root / 'checkout'
            subprocess.run(['git', 'init', '--bare', str(remote)], check=True, capture_output=True)
            subprocess.run(['git', 'clone', str(remote), str(checkout)], check=True, capture_output=True)
            old = Path.cwd()
            os.chdir(checkout)
            try:
                def git(*args):
                    return subprocess.check_output(['git', '-c', 'commit.gpgsign=false', *args], text=True, stderr=subprocess.DEVNULL).strip()
                git('config', 'user.name', 'Test')
                git('config', 'user.email', 'test@example.invalid')
                git('checkout', '-b', 'develop')
                for feed, raw in feeds().items():
                    path = Path('pages/updates') / feed / 'catalog.json'
                    path.parent.mkdir(parents=True)
                    path.write_bytes(raw)
                git('add', '.')
                git('commit', '-m', 'empty reviewed catalogs')
                git('push', 'origin', 'develop')
                prs, pushes = {}, []
                def invoke(*args):
                    if args[0] == 'git':
                        if args[1] == 'push':
                            pushes.append(args)
                        return git(*args[1:])
                    if args[:3] == ('gh', 'pr', 'list'):
                        head = args[args.index('--head') + 1]
                        return json.dumps([{'number': prs[head]}] if head in prs else [])
                    if args[:3] == ('gh', 'pr', 'create'):
                        head = args[args.index('--head') + 1]
                        prs[head] = len(prs) + 1
                        return 'review-pending'
                    raise AssertionError(args)
                def metadata(tag):
                    return published(tag[1:], 'GA' if tag == 'v1.8.0' else 'Beta')
                proposer.propose('v1.8.0', invoke, metadata, prepare)
                original = git('rev-parse', 'chore/catalog-v1.8.0')
                proposer.propose('v1.9.0', invoke, metadata, prepare)
                git('checkout', 'develop')
                git('merge', '--no-ff', '--no-edit', 'chore/catalog-v1.9.0')
                git('push', 'origin', 'develop')
                proposer.propose('v1.8.0', invoke, metadata, prepare)
                git('merge-base', '--is-ancestor', original, 'chore/catalog-v1.8.0')
                self.assertEqual(len(prs), 2)
                self.assertTrue(all('--force' not in args for args in pushes))
                preview = json.loads(git('show', 'origin/chore/catalog-v1.8.0:pages/updates/v2/catalog.json'))
                self.assertEqual({e['classification'] for e in preview['entries']}, {'GA', 'Beta'})
                self.assertEqual(len(preview['entries']), 10)
                self.assertEqual(set(git('diff', '--name-only', 'origin/develop', 'origin/chore/catalog-v1.8.0').splitlines()), set(proposer.PATHS))
            finally:
                os.chdir(old)

    def test_production_proposer_retries_latest_develop_reuses_pr_normal_push(self):
        with tempfile.TemporaryDirectory() as directory:
            import os
            old = Path.cwd()
            os.chdir(directory)
            try:
                for f, raw in feeds().items():
                    path = Path('pages/updates') / f / 'catalog.json'
                    path.parent.mkdir(parents=True)
                    path.write_bytes(raw)
                commands, revisions = [], iter(['old', 'new', 'new', 'new'])
                def invoke(*args):
                    commands.append(args)
                    if args[:3] == ('git', 'rev-parse', 'origin/develop'):
                        return next(revisions)
                    if args[:3] == ('gh', 'pr', 'list'):
                        return '[{"number": 1}]'
                    if args[:3] == ('git', 'diff', '--name-only'):
                        return '\n'.join(proposer.PATHS)
                    if args[:4] == ('git', 'diff', '--cached', '--name-only'):
                        return proposer.PATHS[0]
                    return ''
                result = proposer.propose('v1.8.0', invoke, lambda _: published(), prepare)
                self.assertIn('catalog-pending', result)
                self.assertEqual(sum(c[:3] == ('gh', 'pr', 'list') for c in commands), 2)
                self.assertTrue(any(c[:2] == ('git', 'merge') for c in commands))
                self.assertTrue(any(c[:2] == ('git', 'push') for c in commands))
                self.assertFalse(any('merge' in c and c[0] == 'gh' for c in commands))
                self.assertFalse(any('--force' in c for c in commands))
            finally:
                os.chdir(old)


class HostingTests(unittest.TestCase):
    def test_workflow_only_promotion_preserves_ga_source_for_push_and_schedule(self):
        import sys
        with tempfile.TemporaryDirectory() as directory:
            repository = Path(directory)
            def git(*args):
                return subprocess.check_output(['git', '-C', str(repository), *args], text=True).strip()
            git('init', '-q')
            git('config', 'user.name', 'Test')
            git('config', 'user.email', 'test@example.invalid')
            git('config', 'commit.gpgsign', 'false')
            (repository / 'index.html').write_text('approved GA')
            git('add', '.')
            git('commit', '-qm', 'GA')
            published = git('rev-parse', 'HEAD')
            workflows = repository / '.github/workflows'
            workflows.mkdir(parents=True)
            (workflows / 'pages.yml').write_text('workflow-only handoff')
            git('add', '.')
            git('commit', '-qm', 'Promote workflows')
            promoted = git('rev-parse', 'HEAD')
            cli = ROOT / 'scripts/compose-catalog-site.py'
            command = [sys.executable, str(cli), 'source-mode', '--repository', str(repository),
                       '--revision', published, '--current', promoted]
            self.assertEqual(subprocess.check_output(command, text=True).strip(), 'catalog')
            self.assertFalse(site.ga_source_changed(repository, published, promoted))
            # A later actual GA content change still rebuilds.
            (repository / 'index.html').write_text('new approved GA')
            git('add', '.')
            git('commit', '-qm', 'GA content')
            command[-1] = git('rev-parse', 'HEAD')
            self.assertEqual(subprocess.check_output(command, text=True).strip(), 'ga')
            self.assertTrue(site.ga_source_changed(repository, published, command[-1]))
            with self.assertRaises(ValueError):
                site.ga_source_changed(repository, '', promoted)
        workflow = (ROOT / '.github/workflows/pages.yml').read_text()
        self.assertEqual(workflow.count('compose-catalog-site.py source-mode'), 2)
        self.assertIn('BEFORE: ${{ github.event.before }}', workflow)

    def setup_site(self, directory):
        root = Path(directory)
        website, catalogs = root / 'website', root / 'catalogs'
        website.mkdir()
        (website / 'index.html').write_bytes(b'GA version 1.8.0')
        (website / 'asset.css').write_bytes(b'GA CSS\x00bytes')
        for f, raw in feeds().items():
            path = catalogs / f'updates/{f}/catalog.json'
            path.parent.mkdir(parents=True)
            path.write_bytes(raw)
        return root, website, catalogs

    def test_catalog_only_composition_retains_all_ga_bytes_and_new_site_overlays_latest_catalogs(self):
        with tempfile.TemporaryDirectory() as directory:
            root, website, catalogs = self.setup_site(directory)
            storage = root / 'storage'
            reference = site.retain(website, storage, 'ga-source')
            site.restore(storage, reference, root / 'restored')
            pair = prepare(feeds(), published('1.9.0', 'Beta'))
            for f, raw in pair.items():
                (catalogs / f'updates/{f}/catalog.json').write_bytes(raw)
            site.compose(root / 'restored', catalogs, root / 'composed')
            self.assertEqual(site.manifest(site.files(website), True), site.manifest(site.files(root / 'composed'), True))
            (website / 'index.html').write_bytes(b'New approved GA website')
            stale = website / site.CATALOGS[1]
            stale.parent.mkdir(parents=True)
            stale.write_bytes(feeds()['v2'])
            site.compose(website, catalogs, root / 'composed')
            self.assertEqual((root / 'composed' / site.CATALOGS[1]).read_bytes(), pair['v2'])
            self.assertEqual((root / 'composed' / 'index.html').read_bytes(), b'New approved GA website')

    def test_missing_or_corrupt_retained_bundle_and_stale_composition_fail_closed(self):
        with tempfile.TemporaryDirectory() as directory:
            root, website, catalogs = self.setup_site(directory)
            with self.assertRaises(ValueError):
                site.files(root / 'missing')
            reference = site.retain(website, root / 'storage', 'ga')
            reference['files']['index.html'] = 'bad'
            with self.assertRaises(ValueError):
                site.restore(root / 'storage', reference, root / 'restored')
            site.compose(website, catalogs, root / 'composed')
            (catalogs / site.CATALOGS[1]).write_bytes(prepare(feeds(), published('2.0.0'))['v2'])
            with self.assertRaises(ValueError):
                site.guard(root / 'composed', catalogs)
            self.assertEqual((website / 'index.html').read_bytes(), b'GA version 1.8.0')

    def test_served_verification_detects_stale_cache_wrong_valid_feed_and_network_failure(self):
        with tempfile.TemporaryDirectory() as directory:
            root, website, catalogs = self.setup_site(directory)
            site.compose(website, catalogs, root / 'composed')
            calls = []
            def read(url, refresh):
                calls.append(refresh)
                name = url.split('https://example.com/')[1]
                return (root / 'composed' / name).read_bytes(), {'Cache-Control': 'max-age=60'}
            site.verify_served(root / 'composed', 'https://example.com', read, all_files=True)
            self.assertIn(False, calls)
            self.assertIn(True, calls)
            for bad in [lambda *_: (b'{}', {}), lambda *_: (prepare(feeds(), published())['v1'], {'Cache-Control': 'max-age=60'}), lambda *_: (feeds()['v1'], {})]:
                with self.assertRaises(ValueError):
                    site.verify_served(root / 'composed', 'https://example.com', bad, attempts=1)
            def unavailable(*_):
                raise OSError('network unavailable')
            with self.assertRaisesRegex(ValueError, 'unverified'):
                site.verify_served(root / 'composed', 'https://example.com', unavailable, attempts=1)

    def test_production_cli_blocks_catalog_restore_until_candidate_verified(self):
        import sys
        with tempfile.TemporaryDirectory() as directory:
            root, website, catalogs = self.setup_site(directory)
            cli = ROOT / 'scripts/compose-catalog-site.py'
            storage = root / 'storage'
            subprocess.run([sys.executable, str(cli), 'retain', '--website', str(website), '--storage', str(storage), '--revision', 'ga-source'], check=True)
            result = subprocess.run([sys.executable, str(cli), 'restore', '--storage', str(storage), '--destination', str(root / 'restored')], capture_output=True)
            self.assertNotEqual(result.returncode, 0)
            self.assertIn(b'unverified GA candidate', result.stderr)
            state = json.loads((storage / 'state.json').read_text())
            state['published'] = state.pop('candidate')
            (storage / 'state.json').write_text(json.dumps(state))
            subprocess.run([sys.executable, str(cli), 'restore', '--storage', str(storage), '--destination', str(root / 'restored')], check=True)
            subprocess.run([sys.executable, str(cli), 'compose', '--website', str(root / 'restored'), '--catalogs', str(catalogs), '--destination', str(root / 'composed')], check=True)
            self.assertEqual((root / 'composed/index.html').read_bytes(), (website / 'index.html').read_bytes())

    def test_bootstrap_rejects_archive_traversal_and_links(self):
        import io
        import tarfile
        bootstrap = load('bootstrap-catalog-site')
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            for name in ('../escape', '/absolute'):
                archive = root / 'bad.tar'
                with tarfile.open(archive, 'w') as output:
                    item = tarfile.TarInfo(name)
                    item.size = 1
                    output.addfile(item, io.BytesIO(b'x'))
                with self.assertRaises(ValueError):
                    bootstrap.extract(archive, root / 'out')


class WorkflowRegressionTests(unittest.TestCase):
    def test_missing_catalog_credentials_are_reported_before_scheduling_proposal(self):
        import os
        text = (ROOT / '.github/workflows/update-catalog.yml').read_text()
        self.assertIn('  credentials:', text)
        contract = text.split('    secrets:', 1)[1].split('  schedule:', 1)[0]
        self.assertNotIn('required: true', contract)
        job = text.split('  credentials:', 1)[1].split('  propose:', 1)[0]
        script = job.split('        run: |\n', 1)[1]
        script = '\n'.join(line[10:] for line in script.splitlines())
        for app_id, private_key in [('', ''), ('test-app', ''), ('', 'test-private-key'), ('test-app', 'test-private-key')]:
            with tempfile.TemporaryDirectory() as directory:
                output = Path(directory) / 'output'
                summary = Path(directory) / 'summary'
                result = subprocess.run(['bash', '-eu', '-c', script], capture_output=True, text=True,
                    env=dict(os.environ, CATALOG_APP_ID=app_id, CATALOG_APP_PRIVATE_KEY=private_key,
                             GITHUB_OUTPUT=str(output), GITHUB_STEP_SUMMARY=str(summary)))
                self.assertEqual(result.returncode, 0, result.stderr)
                configured = bool(app_id and private_key)
                self.assertIn('configured=' + str(configured).lower(), output.read_text())
                if not configured:
                    self.assertIn('catalog-pending', summary.read_text())
                self.assertNotIn('test-private-key', result.stdout + result.stderr)
        proposal = text.split('  propose:', 1)[1]
        self.assertIn('needs: credentials', proposal)
        self.assertIn("needs.credentials.outputs.configured == 'true'", proposal)

    def test_release_success_invokes_shared_proposer_independent_of_ga_and_store(self):
        workflow = (ROOT / '.github/workflows/release-candidate.yml').read_text()
        self.assertIn('  propose-catalog:', workflow)
        job = workflow.split('  propose-catalog:', 1)[1].split('\n  propose-main-promotion:', 1)[0]
        self.assertIn("needs.publish-release.result == 'success'", job)
        self.assertIn('!inputs.draft_release', job)
        self.assertNotIn("inputs.channel == 'GA'", job)
        self.assertIn('uses: ./.github/workflows/update-catalog.yml', job)
        proposal = (ROOT / '.github/workflows/update-catalog.yml').read_text()
        self.assertIn('workflow_call:', proposal)
        self.assertIn('workflow_dispatch:', proposal)
        self.assertIn('actions/create-github-app-token@', proposal)
        self.assertIn('scripts/propose-release-catalog.py', proposal)
        self.assertNotIn('gh pr merge', proposal)

    def test_develop_merge_delivers_without_main_promotion_preserving_ga_snapshots(self):
        workflow = (ROOT / '.github/workflows/pages.yml').read_text()
        self.assertIn('branches: [main, develop]', workflow)
        self.assertIn('group: github-pages', workflow)
        self.assertIn('compose-catalog-site.py restore', workflow)
        self.assertIn('compose-catalog-site.py compose', workflow)
        self.assertIn('compose-catalog-site.py guard', workflow)
        self.assertIn('compose-catalog-site.py verify', workflow)
        self.assertIn('catalog-hosting-state', workflow)
        self.assertIn('Reconcile superseded GA events', workflow)
        self.assertLess(workflow.index('Reject stale composition'), workflow.index('uses: actions/deploy-pages@'))
        self.assertNotIn('source: ./tools/pages', workflow)

    def test_workflow_promotion_requires_approved_definitions_and_production_tests(self):
        workflow = (ROOT / '.github/workflows/catalog-promotion-validation.yml').read_text()
        self.assertIn('branches: [main]', workflow)
        self.assertIn('git diff --exit-code origin/develop HEAD', workflow)
        self.assertIn('test_*catalog*.py', workflow)
        self.assertIn('git worktree add --detach', workflow)
        self.assertNotIn('gh pr merge', workflow)
        self.assertNotIn('pull_request_target:', workflow)

    def test_normal_ci_runs_shared_orchestration_regressions(self):
        for name in ('ci.yml', 'website-validation.yml'):
            text = (ROOT / '.github/workflows' / name).read_text()
            self.assertIn('test_catalog_delivery.py', text)
            for path in ('release-catalog.py', 'propose-release-catalog.py', 'compose-catalog-site.py', 'bootstrap-catalog-site.py'):
                self.assertIn(path, text)


if __name__ == '__main__':
    unittest.main()
