"""Runner safety and evidence semantics, not a substitute for signed native execution."""
import importlib.util
import json
import os
from pathlib import Path
import queue
import subprocess
import tempfile
import unittest
from unittest.mock import patch

ROOT=Path(__file__).resolve().parents[2]
spec=importlib.util.spec_from_file_location('native_runner',ROOT/'scripts/run-macos-updater-acceptance.py')
runner=importlib.util.module_from_spec(spec)
spec.loader.exec_module(runner)


class Server:
    def __init__(self, directory):
        self.results=queue.Queue()
        self.directory=directory
    def serve_forever(self): pass
    def shutdown(self): pass
    def server_close(self): pass


class NativeAcceptanceTests(unittest.TestCase):
    def test_native_failure_preserves_operation_and_os_error_without_private_detail(self):
        with tempfile.TemporaryDirectory() as directory:
            root=Path(directory)
            replies=[dict(stage='baseline',version='1.8.2',settingsPreserved=True),
                dict(stage='failed',version='1.8.2',settingsPreserved=True,error={
                    'message':'private installation path and diagnostics',
                    'operation':'install','kind':'io','osCode':1,'ioKind':'PermissionDenied'})]
            with patch.dict(os.environ,SVB_DISPOSABLE_NATIVE_ACCOUNT='1'), patch.object(runner,'FixtureServer',Server), patch.object(runner,'launch'), patch.object(runner,'wait_result',side_effect=replies), patch.object(runner.time,'sleep'), patch.object(runner.subprocess,'run',return_value=subprocess.CompletedProcess([],0)):
                self.assertFalse(runner.run(root/'old.app',root,root/'Applications'/runner.APP,root/'result'))
            private=json.loads((root/'result/native-result.json').read_text())
            summary=json.loads((root/'result/native-summary.json').read_text())
            self.assertIn('private installation path',private[-1]['error']['message'])
            self.assertEqual(summary[-1]['failure'],dict(operation='install',kind='io',osCode=1,ioKind='PermissionDenied'))
            self.assertNotIn('private installation path',json.dumps(summary))

    def test_script_code_survives_native_runner_without_private_message(self):
        error=dict(operation='install',kind='authentication',scriptCode=-1743,
                   scriptStage='execute',message='private AppleScript diagnostic')
        result=runner.public_report(dict(stage='failed',error=error))
        self.assertEqual(result['failure']['scriptCode'],-1743)
        self.assertEqual(result['failure']['scriptStage'],'execute')
        self.assertNotIn('private',json.dumps(result))
        error.update(scriptCode=True,scriptStage='private injected stage')
        result=runner.public_report(dict(stage='failed',error=error))
        self.assertNotIn('scriptCode',result['failure'])
        self.assertNotIn('scriptStage',result['failure'])

    def test_native_replacement_error_is_retained_without_private_details(self):
        error=dict(operation='install',kind='authentication',nativeCode=3072,
                   nativeStage='authorize',reason='native_replacement_failed',
                   message='private domain and diagnostic')
        result=runner.public_report(dict(stage='failed',error=error))
        self.assertEqual(result['failure']['nativeCode'],3072)
        self.assertEqual(result['failure']['nativeStage'],'authorize')
        self.assertEqual(result['failure']['reason'],'native_replacement_failed')
        error['nativeStage']='recovery'
        self.assertEqual(runner.public_report(dict(stage='failed',error=error))['failure']['nativeStage'],'recovery')
        self.assertNotIn('private',json.dumps(result))
        error.update(nativeCode=True,nativeStage='injected stage')
        result=runner.public_report(dict(stage='failed',error=error))
        self.assertNotIn('nativeCode',result['failure'])
        self.assertNotIn('nativeStage',result['failure'])

    def test_runner_refuses_existing_app_and_requires_disposable_account(self):
        with tempfile.TemporaryDirectory() as directory:
            root=Path(directory)
            target=root/'Applications'/runner.APP
            with patch.dict(os.environ,SVB_DISPOSABLE_NATIVE_ACCOUNT=''):
                with self.assertRaises(ValueError): runner.run(root,root,target,root/'result')
            target.mkdir(parents=True)
            with patch.dict(os.environ,SVB_DISPOSABLE_NATIVE_ACCOUNT='1'):
                with self.assertRaises(ValueError): runner.run(root,root,target,root/'result')

    def test_installed_event_does_not_pass_without_relaunch_version_and_settings(self):
        for final,passed in [('snapshot',True),('failed',False),('wrong_version',False),('lost_settings',False)]:
            with tempfile.TemporaryDirectory() as directory:
                root=Path(directory); candidate=root/'candidate';candidate.mkdir()
                (candidate/'manifest.json').write_text(json.dumps(dict(version='1.8.3')))
                replies=[dict(stage='baseline',version='1.8.2',settingsPreserved=True),dict(stage='installed',version='1.8.2',settingsPreserved=True),
                    dict(stage='failed' if final=='failed' else 'snapshot',version='1.8.2' if final=='wrong_version' else '1.8.3',settingsPreserved=final!='lost_settings',updatePreferencesPreserved=True)]
                with patch.dict(os.environ,SVB_DISPOSABLE_NATIVE_ACCOUNT='1'), patch.object(runner,'FixtureServer',Server), patch.object(runner,'launch'), patch.object(runner,'wait_result',side_effect=replies), patch.object(runner.time,'sleep'), patch.object(runner.subprocess,'run',return_value=subprocess.CompletedProcess([],0)):
                    actual=runner.run(root/'old.app',candidate,root/'Applications'/runner.APP,root/'result')
                self.assertEqual(actual,passed)
                self.assertEqual(len(json.loads((root/'result/native-result.json').read_text())),3)

    def test_no_native_result_is_unavailable_not_success(self):
        with tempfile.TemporaryDirectory() as directory:
            root=Path(directory)
            with patch.dict(os.environ,SVB_DISPOSABLE_NATIVE_ACCOUNT='1'), patch.object(runner,'FixtureServer',Server), patch.object(runner,'launch'), patch.object(runner,'wait_result',side_effect=queue.Empty), patch.object(runner.subprocess,'run',return_value=subprocess.CompletedProcess([],0)):
                self.assertFalse(runner.run(root/'old.app',root,root/'Applications'/runner.APP,root/'result'))
            self.assertEqual(json.loads((root/'result/native-result.json').read_text())[0]['stage'],'timed_out')

    def test_signed_workflow_only_runs_probe_when_explicitly_requested(self):
        workflow=(ROOT/'.github/workflows/verify-macos-signing.yml').read_text()
        self.assertIn('default: false',workflow)
        self.assertIn('if: ${{ inputs.updater_acceptance }}',workflow)
        self.assertIn('SVB_DISPOSABLE_NATIVE_ACCOUNT',workflow)
        self.assertNotIn('gh release',workflow)
        self.assertNotIn('git push',workflow)
        builder=(ROOT/'scripts/build-macos-updater-acceptance.sh').read_text()
        self.assertIn('native-updater-acceptance',builder)
        self.assertIn('temporary-updater.key',builder)
        self.assertNotIn('--no-sign',builder)
        self.assertNotIn('codesign --remove-signature',builder)
        self.assertIn('test ! -f "$output/temporary-updater.key"',workflow)


if __name__=='__main__': unittest.main()
