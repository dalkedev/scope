#!/usr/bin/env python3
"""Explicit synthetic loopback capture, native HAR export, and offline CLI proof.
Requires a separately built native_capture.rs binary. Downloads nothing.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import selectors
import shutil
import signal
import subprocess
import tempfile
import time
from urllib.parse import urlsplit
import xml.etree.ElementTree as ET

ROOT = Path(__file__).resolve().parents[3]
CLI = ROOT / 'tools/har-compare/cli.mjs'
MARKERS = ('NATIVE_SYNTHETIC_QUERY_VALUE', 'NATIVE_SYNTHETIC_HEADER_VALUE', 'NATIVE_SYNTHETIC_BODY_VALUE')


def require(condition, message):
    if not condition:
        raise AssertionError(message)


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def read_event(process, timeout=30, max_bytes=65536):
    """Read one bounded JSON line; retain any subsequent event bytes."""
    require(timeout > 0 and max_bytes > 0, 'Producer event bounds must be positive')
    pending = getattr(process, '_scope_event_pending', None)
    if pending is None:
        pending = bytearray()
        process._scope_event_pending = pending
    deadline = time.monotonic() + timeout
    descriptor = process.stdout.fileno()
    os.set_blocking(descriptor, False)
    selector = selectors.DefaultSelector()
    try:
        selector.register(descriptor, selectors.EVENT_READ)
        while True:
            newline = pending.find(b'\n')
            if newline >= 0:
                require(newline <= max_bytes, 'Producer event exceeds byte limit')
                line = bytes(pending[:newline])
                del pending[:newline + 1]
                try:
                    return json.loads(line.decode('utf-8'))
                except (UnicodeDecodeError, json.JSONDecodeError):
                    raise AssertionError('Producer event is not valid JSON') from None
            require(len(pending) <= max_bytes, 'Producer event exceeds byte limit')
            remaining = deadline - time.monotonic()
            require(remaining > 0 and selector.select(remaining), 'Producer event timed out')
            try:
                chunk = os.read(descriptor, min(4096, max_bytes + 1 - len(pending)))
            except BlockingIOError:
                continue
            require(chunk, 'Producer exited before its complete event')
            pending.extend(chunk)
    finally:
        selector.close()


def main(event_timeout=30):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', required=True, type=Path)
    parser.add_argument('--node', required=True, type=Path)
    parser.add_argument('--output-parent', required=True, type=Path)
    args = parser.parse_args()
    binary, node = args.binary.resolve(), args.node.resolve()
    for executable in (binary, node):
        require(executable.is_file() and os.access(executable, os.X_OK), f'Unavailable executable: {executable}')
    require(args.output_parent.is_dir(), 'Output parent must already exist')
    run = Path(tempfile.mkdtemp(prefix='native-capture-', dir=args.output_parent.resolve()))
    run.chmod(0o700)
    archive, exports = run / 'private-archive', run / 'exports'
    environment = {key: os.environ[key] for key in ('PATH', 'HOME', 'TMPDIR', 'LANG', 'LC_ALL') if key in os.environ}
    lock_before = digest(ROOT / 'Cargo.lock')
    receipt = {'schemaVersion': 1, 'ok': False, 'syntheticOnly': True, 'producerSourceSha256': digest(Path(__file__).with_suffix('.rs')),
               'binarySha256': digest(binary), 'cliSha256': digest(CLI), 'rootLockBefore': lock_before,
               'network': 'Producer uses 127.0.0.1 HTTP only; CLI is repeated after producer shutdown.', 'checks': []}
    process = None
    try:
        with (run / 'producer.stderr.log').open('w') as errors:
            process = subprocess.Popen([str(binary), str(archive), str(exports)], stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                                       stderr=errors, text=True, env=environment, start_new_session=True)
            ready = read_event(process, timeout=event_timeout)
            require(ready['status'] == 'ready' and ready['upstreamRequestsBeforeAnalysis'] == 12, 'Producer was not ready')
            origin = ready['origin']
            parsed_origin = urlsplit(origin)
            require(parsed_origin.scheme == 'http' and parsed_origin.hostname == '127.0.0.1' and parsed_origin.port > 0, 'Producer must bind loopback ephemerally')
            receipt['producer'] = ready
            hars, hashes = {}, {}
            for session in ready['sessions']:
                name = session['name']
                require(name in ('baseline', 'missing-field', 'http-failure', 'timestamp-noise'), 'Unexpected export name')
                path = exports / f'{name}.har'
                har = json.loads(path.read_text())
                entries = har['log']['entries']
                require(len(entries) == 3, 'Expected three native entries')
                for entry in entries:
                    scope = entry['_scope']
                    require(scope['completed'] is True and scope['error'] is None, 'Incomplete Snurpo evidence')
                    require(scope['provenance'] == 'upstream' and scope['edited'] is False and scope['appliedRules'] == [], 'Unexpected native provenance')
                    require(scope['originalRequest'] is None and scope['originalResponse'] is None, 'Unexpected edit history')
                    require(entry['request']['url'].startswith(origin + '/'), 'Non-fixture native URL')
                    body = entry['response']['content']
                    require(body['encoding'] is None, 'This fixture expects UTF-8 native bodies')
                    require(body['size'] == len(body['text'].encode()), 'Native body size mismatch')
                hars[name], hashes[name] = har, digest(path)
            require(set(hars) == {'baseline', 'missing-field', 'http-failure', 'timestamp-noise'}, 'Missing session')
            receipt['checks'].append('12 real proxy captures; complete upstream Snurpo extensions and decoded sizes')
            strict = run / 'strict.json'
            ignored = run / 'ignored.json'
            strict.write_text(json.dumps({'mode': 'strict'}) + '\n')
            ignored.write_text(json.dumps({'mode': 'strict', 'scopedIgnores': [{'method': 'GET', 'origin': origin, 'path': '/health', 'pointers': ['/timestamp']}]}) + '\n')
            cases = [
                ('missing-field', 'missing-field', None, 1, 'field-removed', '/users', '/user/role'),
                ('http-failure', 'http-failure', None, 1, 'http-failure', '/failure', None),
                ('timestamp-ignored', 'timestamp-noise', ignored, 0, None, None, None),
                ('timestamp-control', 'timestamp-noise', strict, 1, 'value-changed', '/health', '/timestamp'),
            ]
            def invoke(command, output, expected):
                result = subprocess.run([str(node), str(CLI), *command], capture_output=True, text=True, env=environment, timeout=15)
                require(result.returncode == expected and not result.stderr, f'CLI failed: {result.stdout} {result.stderr}')
                require(output.read_text() == result.stdout, 'JSON stdout/file differ')
                report = json.loads(result.stdout)
                return report

            def analyze(label):
                folder = run / label
                folder.mkdir()
                contents = {}
                for name in hars:
                    output = folder / f'inspect-{name}.json'
                    report = invoke(['inspect', '--input', str(exports / f'{name}.har'), '--json', str(output)], output, 0)
                    require(report['analysisPerformed'] is False and report['summary'] == {'entries': 3, 'operations': 3}, 'Inventory counts differ')
                    operations = {operation['selector']['path']: operation for operation in report['operations']}
                    require(set(operations) == {'/users', '/failure', '/health'}, 'Inventory operations differ')
                    for operation in operations.values():
                        require(operation['selector']['origin'] == origin and operation['jsonResponses'] == 1, 'Inventory JSON/origin differs')
                    contents[output.name] = output.read_bytes()
                for name, candidate, config, exit_code, kind, path, pointer in cases:
                    output = folder / f'{name}.json'
                    html, xml = folder / f'{name}.html', folder / f'{name}.xml'
                    arguments = ['compare', '--baseline', str(exports / 'baseline.har'), '--candidate', str(exports / f'{candidate}.har'), '--json', str(output), '--html', str(html), '--junit', str(xml)]
                    if config:
                        arguments += ['--config', str(config)]
                    report = invoke(arguments, output, exit_code)
                    require(report['summary']['pairedEntries'] == 3 and report['summary']['operations'] == 3, 'Native pairs lost')
                    require(report['summary']['regressions'] == exit_code and report['summary']['informational'] == 0, 'Literal finding counts differ')
                    require(len(report['findings']) == exit_code, 'Unexpected findings')
                    for operation in report['operations']:
                        for side in ('baseline', 'candidate'):
                            for evidence in operation[side]:
                                require(evidence['provenance'] == 'upstream' and evidence['edited'] is False, 'Report lost native provenance')
                    if kind:
                        finding = report['findings'][0]
                        require(finding['kind'] == kind and finding['severity'] == 'regression', 'Finding kind differs')
                        require(finding.get('path') == pointer and finding['occurrence'] == 1, 'Finding pointer/occurrence differs')
                        operation = next(item for item in report['operations'] if item['id'] == finding['operation'])
                        require(operation['path'] == path, 'Finding operation differs')
                        for side, session in (('baseline', 'baseline'), ('candidate', candidate)):
                            index = finding['evidence'][side + 'Entry']
                            entry = hars[session]['log']['entries'][index - 1]
                            require(urlsplit(entry['request']['url']).path == path, 'Evidence does not identify original HAR entry')
                        if kind == 'http-failure':
                            require(finding['status'] == {'before': 200, 'after': 500}, 'Status evidence differs')
                    else:
                        require(report['summary']['ignoredNodes'] == 1 and report['summary']['scopedIgnoredNodes'] == 1, 'Scoped ignore was not exercised')
                    tree = ET.parse(xml)
                    require(tree.getroot().tag == 'testsuites', 'Invalid JUnit root')
                    require(len(tree.findall('.//failure')) == exit_code, 'JUnit failures differ')
                    for file in (output, html, xml):
                        value = file.read_bytes()
                        require(all(marker.encode() not in value for marker in MARKERS), 'Fixture values leaked into report')
                        contents[file.name] = value
                for file in folder.iterdir():
                    require(all(marker.encode() not in file.read_bytes() for marker in MARKERS), 'Fixture marker leaked')
                return contents

            online = analyze('while-fixture-running')
            receipt['checks'].append('4 inventories plus literal missing-field/status/noise/strict-control findings, Snurpo evidence, privacy, and parsed JUnit')
            process.stdin.write('finish\n')
            process.stdin.flush()
            finished = read_event(process, timeout=event_timeout)
            require(process.wait(timeout=10) == 0 and finished['upstreamRequestsAfterAnalysis'] == 12, 'Producer/no-replay check failed')
            receipt['finished'] = finished
            receipt['checks'].append('Fixture request counter stayed exactly 12 throughout CLI analysis')
            shutil.rmtree(archive)
            offline = analyze('after-fixture-shutdown')
            require(online == offline, 'Reports changed after shutting down producer')
            require(hashes == {name: digest(exports / f'{name}.har') for name in hars}, 'HAR inputs changed during analysis')
            receipt['checks'].append('All 16 JSON/HTML/JUnit outputs repeated byte for byte after server/proxy/archive shutdown; HAR bytes unchanged')
            receipt['exportsSha256'] = hashes
            receipt['cases'] = [{'name': case[0], 'exitCode': case[3]} for case in cases]
            receipt['ok'] = True
    except Exception as error:
        receipt['error'] = str(error)
    finally:
        if process and process.poll() is None:
            os.killpg(process.pid, signal.SIGKILL)
            process.wait(timeout=10)
        if archive.exists():
            shutil.rmtree(archive)
        receipt['privateArchiveRemoved'] = not archive.exists()
        receipt['retainedPrivateKeysOrDatabases'] = [str(path.relative_to(run)) for path in run.rglob('*') if path.suffix in ('.pem', '.sqlite', '.sqlite3', '.db')]
        receipt['rootLockAfter'] = digest(ROOT / 'Cargo.lock')
        receipt['rootLockUnchanged'] = receipt['rootLockAfter'] == lock_before
        receipt['ok'] = receipt['ok'] and receipt['privateArchiveRemoved'] and not receipt['retainedPrivateKeysOrDatabases'] and receipt['rootLockUnchanged']
        receipt['limitations'] = ['Synthetic HTTP capture on loopback; no real traffic, HTTPS, device, desktop or browser coverage.', 'An unchanged upstream counter proves this fixture received no replay; it does not instrument arbitrary network syscalls.', 'Offline repeat means producer/proxy stopped and archive deleted; operating-system network isolation is not claimed.']
        (run / 'receipt.json').write_text(json.dumps(receipt, indent=2) + '\n')
    print(json.dumps({'ok': receipt['ok'], 'receipt': str(run / 'receipt.json'), 'error': receipt.get('error'), 'checks': receipt['checks']}, indent=2))
    return 0 if receipt['ok'] else 2


if __name__ == '__main__':
    raise SystemExit(main())
