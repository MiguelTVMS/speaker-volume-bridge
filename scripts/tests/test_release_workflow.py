"""Release dependency graph checks (no GitHub credentials or network required)."""
import re
import os
import subprocess
import tempfile
import unittest
from pathlib import Path

WORKFLOW = Path(__file__).resolve().parents[2] / '.github/workflows/release-candidate.yml'


def jobs(text):
    return dict(re.findall(r'^  ([\w-]+):\n(.*?)(?=^  [\w-]+:|\Z)', text, re.M | re.S))


def dependencies(body):
    match = re.search(r'^    needs:\s*(\[[^\]]*\]|[\w-]+)', body, re.M)
    return re.findall(r'[\w-]+', match[1]) if match else []


class ReleaseWorkflowTests(unittest.TestCase):
    def test_normalized_release_input_order_defaults_and_store_gates(self):
        text = WORKFLOW.read_text()
        inputs = text.split('    inputs:\n', 1)[1].split('\npermissions:', 1)[0]
        blocks = dict(re.findall(r'^      (\w+):\n(.*?)(?=^      \w+:|\Z)', inputs, re.M | re.S))
        self.assertEqual(list(blocks), ['version', 'stable', 'sign_apple_pack', 'push_apple_store', 'push_ms_store'])
        for name, default in [('stable', 'false'), ('sign_apple_pack', 'true'), ('push_apple_store', 'false'), ('push_ms_store', 'false')]:
            self.assertIn('default: ' + default, blocks[name])
            self.assertIn('type: boolean', blocks[name])
        self.assertIn('options: [Major, Minor, Fix]', blocks['version'])
        graph = jobs(text)
        for job, option in [('macos-app-store', 'push_apple_store'), ('publish-apple-store', 'push_apple_store'), ('publish-microsoft-store', 'push_ms_store')]:
            expression = re.search(r'^    if: \$\{\{ (.*?) \}\}', graph[job], re.M)[1]
            for stable in (False, True):
                for push in (False, True):
                    value = expression.replace('cancelled()', 'False').replace('inputs.stable', str(stable)).replace('inputs.' + option, str(push))
                    value = re.sub(r'needs\.[\w-]+\.result', repr('success'), value)
                    value = value.replace('&&', ' and ').replace('!', ' not ')
                    self.assertEqual(eval(value.strip(), {'__builtins__': {}}, {}), stable and push)

    def test_full_uninstall_cleans_native_toast_registration_but_updates_preserve_it(self):
        root = WORKFLOW.parents[2]
        template = (root / 'src-tauri/windows/installer.nsi').read_text(encoding='utf-8')
        uninstall = template.split('Section Uninstall', 1)[1]
        cleanup = uninstall.split('${If} $UpdateMode <> 1', 1)[1].split('${EndIf}', 1)[0]
        for key in ('AppUserModelId\\${BUNDLEID}', 'CLSID\\{a607018c-48b4-45c8-b0b2-46c243fde206}'):
            command = 'DeleteRegKey HKCU "Software\\Classes\\' + key + '"'
            self.assertIn(command, cleanup)
            self.assertEqual(template.count(command), 1)

    def test_all_platform_builds_gate_signing_and_store_submission(self):
        graph = jobs(WORKFLOW.read_text(encoding='utf-8'))
        builds = {'macos-app', 'linux-deb', 'windows-app', 'windows-nsis', 'windows-msix', 'windows-store-upload'}
        self.assertEqual(set(dependencies(graph['all-platform-builds'])), builds)
        self.assertIsNone(re.search(r'^    if:', graph['all-platform-builds'], re.M))
        for job in ('macos-direct', 'macos-app-store', 'publish-microsoft-store'):
            self.assertIn('all-platform-builds', dependencies(graph[job]), job)
            self.assertNotIn('always()', graph[job].split('    steps:')[0])

        def visit(job, pending):
            self.assertNotIn(job, pending, 'Release workflow has a dependency cycle')
            for dependency in dependencies(graph[job]):
                self.assertIn(dependency, graph)
                visit(dependency, pending | {job})
        for job in graph:
            visit(job, set())

    def test_downstream_checkouts_pin_a_commit_from_trusted_history(self):
        graph = jobs(WORKFLOW.read_text())
        for name, body in graph.items():
            if name == 'prepare-version':
                continue
            for checkout in body.split('uses: actions/checkout@')[1:]:
                settings, following = checkout.split('      - ', 1)
                self.assertIn('ref: develop', settings, name)
                self.assertIn('fetch-depth: 0', settings, name)
                self.assertIn('persist-credentials: false', settings, name)
                pin = following.split('      - ', 1)[0]
                self.assertIn('name: Pin the trusted release commit', pin, name)
                self.assertIn('RELEASE_COMMIT: ${{ needs.prepare-version.outputs.commit }}', pin)
                self.assertIn('[[ "$RELEASE_COMMIT" =~ ^[0-9a-f]{40}$ ]]', pin)
                ancestry = pin.index('git merge-base --is-ancestor "$RELEASE_COMMIT" origin/develop')
                detach = pin.index('git checkout --detach "$RELEASE_COMMIT"')
                self.assertLess(ancestry, detach)
                self.assertIn('test "$(git rev-parse HEAD)" = "$RELEASE_COMMIT"', pin)

    def test_release_pin_survives_branch_advance_and_rejects_untrusted_commits(self):
        body = jobs(WORKFLOW.read_text())['macos-app']
        step = body.split('name: Pin the trusted release commit', 1)[1].split('      - ', 1)[0]
        script = '\n'.join(line[10:] for line in step.split('        run: |\n', 1)[1].splitlines())
        with tempfile.TemporaryDirectory() as directory:
            def git(*args):
                return subprocess.check_output(['git', *args], cwd=directory, text=True, stderr=subprocess.DEVNULL).strip()
            git('init', '-b', 'develop')
            git('config', 'user.name', 'Test')
            git('config', 'commit.gpgsign', 'false')
            git('config', 'user.email', 'test@example.invalid')
            git('commit', '--allow-empty', '-m', 'release')
            release = git('rev-parse', 'HEAD')
            git('commit', '--allow-empty', '-m', 'later approved change')
            tip = git('rev-parse', 'HEAD')
            git('update-ref', 'refs/remotes/origin/develop', tip)
            git('checkout', '--orphan', 'unrelated')
            git('commit', '--allow-empty', '-m', 'unrelated')
            unrelated = git('rev-parse', 'HEAD')
            for commit, accepted in ((release, True), (unrelated, False), ('--help', False)):
                git('checkout', 'develop')
                result = subprocess.run(['bash', '-e', '-o', 'pipefail', '-c', script], cwd=directory,
                                        env=dict(os.environ, RELEASE_COMMIT=commit), capture_output=True)
                self.assertEqual(result.returncode == 0, accepted, result.stderr)
                self.assertEqual(git('rev-parse', 'HEAD'), release if accepted else tip)

    def test_combined_store_upload_is_built_before_publication_and_reused(self):
        graph = jobs(WORKFLOW.read_text())
        self.assertIn('windows-store-upload', dependencies(graph['all-platform-builds']))
        package = graph['windows-store-upload']
        self.assertIn('windows-msix', dependencies(package))
        self.assertIn('./scripts/build-msix-upload.ps1', package)
        self.assertNotIn('environment:', package)
        self.assertNotIn('    if:', package)
        submission = graph['publish-microsoft-store']
        self.assertIn('uses: ./.github/workflows/microsoft-store-publish.yml', submission)
        self.assertIn('release_tag: ${{ needs.prepare-version.outputs.tag }}', submission)
        self.assertIn('actions: read', submission)
        self.assertIn('dry_run: false', submission)
        self.assertNotIn('steps:', submission)
        self.assertNotIn('MakeAppx', submission)
        self.assertNotIn('Compress-Archive', submission)
        ci = (WORKFLOW.parent / 'ci.yml').read_text()
        self.assertIn('./scripts/build-msix.ps1', ci)
        self.assertIn('./scripts/tests/test-msix-upload.ps1', ci)
        manual = (WORKFLOW.parent / 'microsoft-store-package.yml').read_text()
        self.assertIn('./scripts/build-msix-upload.ps1', manual)

    def test_publication_waits_for_every_requested_variant(self):
        body = jobs(WORKFLOW.read_text())['publish-release']
        required = {'prepare-version', 'all-platform-builds', 'macos-direct'}
        self.assertEqual(set(dependencies(body)), required | {'macos-app-store'})
        expression = re.search(r'    if: >-\n(.*?)    runs-on:', body, re.S)[1]
        expression = expression.strip().removeprefix('${{').removesuffix('}}').strip()

        def permitted(results, selected, cancelled=False):
            value = expression.replace('cancelled()', str(cancelled))
            value = value.replace('(inputs.stable && inputs.push_apple_store)', str(selected)).replace('inputs.sign_apple_pack', 'True')
            value = re.sub(r'needs\.([\w-]+)\.result', lambda match: repr(results[match[1]]), value)
            value = value.replace('&&', ' and ').replace('||', ' or ').replace('!', ' not ')
            return eval(' '.join(value.split()), {'__builtins__': {}}, {})

        for selected in (False, True):
            for apple in ('success', 'failure', 'cancelled', 'skipped'):
                results = dict.fromkeys(required, 'success') | {'macos-app-store': apple}
                expected = apple == 'success' or (not selected and apple == 'skipped')
                self.assertEqual(permitted(results, selected), expected, (selected, apple))
                self.assertFalse(permitted(results, selected, cancelled=True))
                for job in required:
                    for failure in ('failure', 'cancelled', 'skipped'):
                        self.assertFalse(permitted(results | {job: failure}, selected))

    def test_every_store_publication_requires_successful_github_publication(self):
        graph = jobs(WORKFLOW.read_text())
        for job in ('publish-apple-store', 'publish-microsoft-store'):
            self.assertIn('publish-release', dependencies(graph[job]))
            self.assertIn("needs.publish-release.result == 'success'", graph[job])
            self.assertNotIn('always()', graph[job].split('    steps:')[0])

    def test_publication_allows_unsigned_only_when_signing_was_disabled(self):
        body = jobs(WORKFLOW.read_text())['publish-release']
        expression = re.search(r'    if: >-\n(.*?)    runs-on:', body, re.S)[1].strip().removeprefix('${{').removesuffix('}}').strip()
        for sign in (False, True):
            for result in ('success', 'failure', 'cancelled', 'skipped'):
                value = expression.replace('cancelled()', 'False').replace('inputs.sign_apple_pack', str(sign)).replace('(inputs.stable && inputs.push_apple_store)', 'False')
                results = {'prepare-version': 'success', 'all-platform-builds': 'success', 'macos-direct': result, 'macos-app-store': 'skipped'}
                value = re.sub(r'needs\.([\w-]+)\.result', lambda match: repr(results[match[1]]), value)
                value = value.replace('&&', ' and ').replace('||', ' or ').replace('!', ' not ')
                self.assertEqual(eval(' '.join(value.split()), {'__builtins__': {}}, {}), result == 'success' or (not sign and result == 'skipped'))
        graph = jobs(WORKFLOW.read_text())
        self.assertIn('if: ${{ !inputs.sign_apple_pack }}', graph['macos-app'])
        self.assertIn('--no-sign', graph['macos-app'])
        self.assertIn('$apple_signing_message', body)

    def test_apple_upload_validates_before_upload_and_cleans_key_on_failure(self):
        body = jobs(WORKFLOW.read_text())['publish-apple-store']
        script = body.split('        run: |\n', 1)[1]
        script = '\n'.join(line[10:] for line in script.splitlines())
        for rejected in (False, True):
            with tempfile.TemporaryDirectory() as directory:
                workspace = Path(directory)
                (workspace / 'apple-store-upload').mkdir()
                (workspace / 'apple-store-upload/test.pkg').write_text('fixture')
                tools = workspace / 'bin'
                tools.mkdir()
                for name, code in [('pkgutil', '#!/bin/sh\nexit 0\n'), ('xcrun', '#!/bin/sh\nprintf "%s\n" "$*" >> calls\ncase "$*" in *--validate-app*) test "$REJECT_VALIDATION" != true;; esac\n')]:
                    tool = tools / name
                    tool.write_text(code)
                    tool.chmod(0o755)
                result = subprocess.run(['bash', '-e', '-c', script], cwd=workspace, capture_output=True,
                    env=dict(os.environ, PATH=str(tools) + os.pathsep + os.environ['PATH'], RUNNER_TEMP=directory, GITHUB_STEP_SUMMARY=str(workspace / 'summary'), APPLE_API_KEY='fixture', APPLE_API_ISSUER='fixture', APPLE_API_PRIVATE_KEY='fixture', REJECT_VALIDATION=str(rejected).lower()))
                self.assertEqual(result.returncode == 0, not rejected)
                calls = (workspace / 'calls').read_text()
                self.assertIn('--validate-app', calls)
                self.assertEqual('--upload-app' in calls, not rejected)
                if not rejected:
                    self.assertIn('Manually release this version', (workspace / 'summary').read_text())
                self.assertFalse((workspace / 'apple-store-private-keys').exists())

    def test_draft_release_flags_and_downstream_publication_gates(self):
        graph = jobs(WORKFLOW.read_text())
        body = graph['publish-release']
        script = body.split('          release_flags=()', 1)[1].split('          release_intro=', 1)[0]
        script = 'release_flags=()\n' + '\n'.join(line[10:] for line in script.splitlines())
        script += '\nprintf "%s\\n" "${release_flags[@]}"'
        for channel in ('GA', 'Beta', 'Alpha'):
            for draft in (False, True):
                result = subprocess.check_output(['bash', '-e', '-c', script], text=True,
                    env=dict(os.environ, RELEASE_CHANNEL=channel, DRAFT_RELEASE=str(draft).lower()))
                self.assertEqual('--draft' in result.splitlines(), draft)
                self.assertEqual('--prerelease' in result.splitlines(), channel != 'GA')
        for job in ('propose-main-promotion', 'publish-microsoft-store'):
            expression = re.search(r'    if: \$\{\{ (.*?) \}\}', graph[job])[1]
            for draft in (False, True):
                value = expression.replace('cancelled()', 'False')
                value = value.replace('inputs.draft_release', str(draft))
                value = value.replace('inputs.stable', 'True')
                value = value.replace('inputs.push_ms_store', 'True')
                value = re.sub(r'needs\.[\w-]+\.result', repr('success'), value)
                value = value.replace('&&', ' and ').replace('!', ' not ')
                self.assertTrue(eval(value.strip(), {'__builtins__': {}}, {}), job)

    def test_draft_run_cannot_overwrite_a_public_release(self):
        body = jobs(WORKFLOW.read_text())['publish-release']
        guard = body.split('            if [ "$DRAFT_RELEASE"', 1)[1].split('            gh release upload', 1)[0]
        script = 'gh() { echo "$EXISTING_DRAFT"; }\nif [ "$DRAFT_RELEASE"' + guard
        for existing, expected in (('true', 0), ('false', 1)):
            result = subprocess.run(['bash', '-eu', '-c', script], capture_output=True,
                env=dict(os.environ, DRAFT_RELEASE='true', EXISTING_DRAFT=existing, RELEASE_TAG='v1.6.0'))
            self.assertEqual(result.returncode, expected, result.stderr)

    def test_updater_validation_stays_private_and_failure_propagates(self):
        graph = jobs(WORKFLOW.read_text())
        direct = graph['macos-direct']
        self.assertNotIn('macos-updater', graph['macos-app-store'])
        self.assertLess(direct.index('xcrun stapler validate "$image"'), direct.index('Prepare updater validation artifacts'))
        step = direct.split('name: Prepare updater validation artifacts', 1)[1].split('      - uses:', 1)[0]
        script = '\n'.join(line[10:] for line in step.split('        run: |\n', 1)[1].splitlines())
        for private, public, accepted in [('', '', True), ('fixture', '', False), ('', 'fixture', False), ('fixture', 'fixture', False)]:
            with tempfile.TemporaryDirectory() as directory:
                env = dict(os.environ, RUNNER_TEMP=directory, GITHUB_STEP_SUMMARY=directory + '/summary',
                    TAURI_SIGNING_PRIVATE_KEY=private, UPDATER_PUBLIC_KEY=public, RELEASE_VERSION='2.0.0')
                # A configured verifier failure must fail this step, never become optional success.
                result = subprocess.run(['bash', '-eu', '-c', 'cargo() { return 1; }\n' + script], env=env, capture_output=True)
                self.assertEqual(result.returncode == 0, accepted)
                self.assertFalse((Path(directory) / 'updater.pub').exists())
        self.assertIn('name: macos-updater-validation', direct)
        self.assertNotIn('name: macos-updater-release', direct)
        self.assertIn('pattern: "*-release"', graph['publish-release'])

    def test_future_release_title_is_only_the_version_tag(self):
        body = jobs(WORKFLOW.read_text())['publish-release']
        self.assertIn('--title "$RELEASE_TAG"', body)
        self.assertNotIn('--title "Speaker Volume Bridge', body)
        command = re.search(r'gh release create .*?--generate-notes', body, re.S)[0]
        script = 'release_assets=(fixture); release_flags=(--prerelease); gh() { printf "%s\\n" "$@"; }\n' + command
        result = subprocess.check_output(['bash', '-eu', '-c', script], text=True,
            env=dict(os.environ, RELEASE_TAG='v2.0.0', release_intro='release'))
        args = result.splitlines()
        self.assertEqual(args[args.index('--title') + 1], 'v2.0.0')

    def test_macos_asset_name_contract_is_guarded_before_build(self):
        body = jobs(WORKFLOW.read_text())['macos-app']
        self.assertIn('run: test "$(uname -m)" = arm64', body)
        self.assertLess(body.index('Verify the official macOS asset architecture contract'), body.index('cargo tauri build --no-bundle'))

    def test_macos_publishes_only_the_verified_dmg(self):
        body = jobs(WORKFLOW.read_text())['macos-direct']
        self.assertNotIn('macos.zip', body)
        self.assertNotIn('Archive the notarized app bundle', body)
        self.assertIn('speaker-volume-bridge-*-macos.dmg', body)
        self.assertIn('xcrun stapler validate "$image"', body)
        self.assertIn('hdiutil verify "$image"', body)

    def test_windows_artifacts_are_separate_for_both_native_architectures(self):
        graph = jobs(WORKFLOW.read_text(encoding='utf-8'))
        for job in ('windows-app', 'windows-nsis', 'windows-msix'):
            self.assertIn('os: windows-latest', graph[job])
            self.assertIn('arch: x64', graph[job])
            self.assertIn('os: windows-11-arm', graph[job])
            self.assertIn('arch: arm64', graph[job])
            uploads = graph[job].split('uses: actions/upload-artifact@')[1:]
            self.assertTrue(uploads)
            for upload in uploads:
                self.assertRegex(upload, r'name: [^\n]*\$\{\{ matrix.arch \}\}')


if __name__ == '__main__':
    unittest.main()
