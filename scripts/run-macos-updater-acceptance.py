#!/usr/bin/env python3
"""Run the actual signed sandboxed probe in a disposable macOS runner/account.

Never replace an existing installation or use a container with existing settings.
Reports failures as evidence, not as successful upgrades. Does not publish anything.
"""
import argparse
import http.server
import json
import os
from pathlib import Path
import queue
import re
import subprocess
import threading
import time

APP = 'Speaker Volume Bridge.app'


class FixtureServer(http.server.ThreadingHTTPServer):
    def __init__(self, directory):
        self.directory = directory
        self.results = queue.Queue()
        super().__init__(('127.0.0.1', 8765), Handler)


class Handler(http.server.SimpleHTTPRequestHandler):
    def __init__(self, *args, **kwargs):
        super().__init__(*args, directory=str(args[2].directory), **kwargs)

    def log_message(self, *args):
        pass

    def do_POST(self):
        size = int(self.headers.get('Content-Length', '0'))
        if self.path != '/result' or not 0 < size <= 16384:
            self.send_error(400)
            return
        self.server.results.put(json.loads(self.rfile.read(size)))
        self.send_response(204)
        self.end_headers()


def launch(app, mode):
    subprocess.run(['open', '-n', str(app), '--args', '--native-updater-acceptance', mode], check=True)


def wait_result(server, stages, timeout=90):
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        result = server.results.get(timeout=max(1, deadline-time.monotonic()))
        if result['stage'] in stages or result['stage'] == 'failed':
            return result
    raise TimeoutError('Native acceptance did not report a result')


def public_report(report):
    sanitized={key:value for key,value in report.items() if key not in ('error','failure')}
    error=report.get('error')
    if isinstance(error,dict):
        # Only typed operation/category and numeric system codes may leave the
        # disposable account. Never retain arbitrary native messages or paths.
        operations={'precondition','build','check','download','install'}
        kinds={'probe','io','timeout','connect','http','transport','signature','format','authentication','updater'}
        if error.get('operation') in operations and error.get('kind') in kinds:
            failure={key:error[key] for key in ('operation','kind')}
            for key in ('osCode','httpStatus','scriptCode'):
                value=error.get(key)
                if type(value) is int:
                    failure[key]=value
            if error.get('scriptStage') in {'dispatch','receive','compile','execute','decode'}:
                failure['scriptStage']=error['scriptStage']
            if error.get('ioKind') in {'PermissionDenied','NotFound','AlreadyExists','InvalidData','InvalidInput','TimedOut','Interrupted','UnexpectedEof','WriteZero','Other','ReadOnlyFilesystem','CrossesDevices','StorageFull','NotADirectory','IsADirectory','DirectoryNotEmpty','Unsupported','Uncategorized'}:
                failure['ioKind']=error['ioKind']
            if error.get('reason') == 'replacement_authorization_failed':
                failure['reason']=error['reason']
            sanitized['failure']=failure
    return sanitized


def run(old_app, candidate, destination, output):
    # This runner is intentionally explicit about isolation before opening the app.
    if os.environ.get('SVB_DISPOSABLE_NATIVE_ACCOUNT') != '1':
        raise ValueError('Explicit disposable-account acknowledgement is required')
    if destination.exists():
        raise ValueError('Refusing to replace an existing installation')
    if destination.name != APP or destination.parent.name != 'Applications':
        raise ValueError('Acceptance requires an Applications installation')
    output.mkdir(parents=True, exist_ok=True)
    reports = []
    server = FixtureServer(candidate)
    thread = threading.Thread(target=server.serve_forever, daemon=True)
    thread.start()
    installed = False
    try:
        subprocess.run(['ditto', str(old_app), str(destination)], check=True)
        installed = True
        subprocess.run(['codesign', '--verify', '--deep', '--strict', str(destination)], check=True)
        launch(destination, 'seed')
        baseline = wait_result(server, ['baseline'])
        reports.append(baseline)
        if baseline['stage'] != 'baseline' or not baseline['settingsPreserved']:
            raise ValueError('Native baseline did not pass; do not attempt replacement')
        # The seed app exits itself; wait for launch services before the next instance.
        time.sleep(3)
        launch(destination, 'install')
        result = wait_result(server, ['installed', 'snapshot'])
        reports.append(result)
        if result['stage'] == 'installed':
            result = wait_result(server, ['snapshot'])
            reports.append(result)
        if result['stage'] != 'snapshot':
            return False
        expected = json.loads((candidate/'manifest.json').read_text())['version']
        passed = result['version'] == expected and result['settingsPreserved'] and result.get('updatePreferencesPreserved',False)
        subprocess.run(['codesign', '--verify', '--deep', '--strict', str(destination)], check=True)
        subprocess.run(['xcrun', 'stapler', 'validate', str(destination)], check=True)
        subprocess.run(['spctl', '--assess', '--type', 'execute', str(destination)], check=True)
        return passed
    except (queue.Empty, TimeoutError) as error:
        reports.append(dict(stage='timed_out', error=str(error)))
        return False
    finally:
        (output/'native-result.json').write_text(json.dumps(reports, indent=2)+'\n')
        sanitized=[public_report(report) for report in reports]
        (output/'native-summary.json').write_text(json.dumps(sanitized,indent=2)+'\n')
        for report in sanitized:
            if 'failure' in report:
                print('Native failure: '+json.dumps(report['failure'],sort_keys=True))
        server.shutdown()
        server.server_close()
        # The probe app must be closed before removing the disposable installation.
        # Keep all packages and the result for recovery and diagnosis on failures.
        if installed:
            executable = destination/'Contents/MacOS/speaker-volume-bridge'
            subprocess.run(['pkill', '-f', '^'+re.escape(str(executable))+'( |$)'], check=False)


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('old_app',type=Path)
    parser.add_argument('candidate',type=Path)
    parser.add_argument('destination',type=Path)
    parser.add_argument('output',type=Path)
    args=parser.parse_args()
    passed=run(args.old_app,args.candidate,args.destination,args.output)
    print('Native replacement: '+('passed' if passed else 'failed or unavailable'))
    raise SystemExit(0 if passed else 2)


if __name__=='__main__':
    main()
