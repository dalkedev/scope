#!/usr/bin/env python3
"""Inspect local tools; explicitly opt into offline checks. Downloads nothing."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import signal
import subprocess
import sys
import time

ROOT = Path(__file__).resolve().parents[3]
PROTOTYPE = ROOT / 'tools' / 'har-compare'
NODE_MINIMUM = (26, 13, 1)


def child_environment():
    # No task/application credentials are needed by these offline checks.
    allowed = ('PATH', 'HOME', 'TMPDIR', 'TEMP', 'TMP', 'LANG', 'LC_ALL',
               'SYSTEMROOT', 'COMSPEC', 'PATHEXT', 'USERPROFILE')
    environment = {name: os.environ[name] for name in allowed if name in os.environ}
    environment['PYTHONDONTWRITEBYTECODE'] = '1'
    return environment


def run(command, environment, timeout=30):
    started = time.monotonic()
    process = subprocess.Popen(command, cwd=ROOT, env=environment,
                               stdout=subprocess.PIPE, stderr=subprocess.STDOUT,
                               start_new_session=os.name == 'posix')
    try:
        output, _ = process.communicate(timeout=timeout)
        code = process.returncode
    except subprocess.TimeoutExpired:
        if os.name == 'posix':
            os.killpg(process.pid, signal.SIGKILL)
        else:
            process.kill()
        output, _ = process.communicate()
        code = 124
    result = {'command': [str(part) for part in command], 'exitCode': code,
              'seconds': round(time.monotonic() - started, 3),
              'output': output.decode('utf-8', errors='replace')}
    return result


def version_tuple(value):
    found = re.search(r'(\d+)\.(\d+)\.(\d+)', value)
    return tuple(map(int, found.groups())) if found else None


def require_executable(path):
    if not path.is_file() or not os.access(path, os.X_OK):
        raise ValueError(f'Executable is unavailable: {path}')
    return str(path.resolve())


def lock_hash():
    return hashlib.sha256((ROOT / 'Cargo.lock').read_bytes()).hexdigest()


def inspect(args):
    report = {
        'schemaVersion': 1, 'mode': 'verify' if args.verify else 'doctor',
        'network': 'No downloader; Cargo checks use --locked --offline.',
        'repository': str(ROOT), 'checks': [], 'selectedChecks': ['node-version'],
        'toolAvailability': {}, 'missingPrerequisites': [],
        'rust': {'status': 'not requested; pass --artifact-prefix for headless Rust checks'},
        'limitations': [
            'Success applies only to the named selected checks.',
            'The artifact-only bridge-module tests and MCP smoke are separate from the selected headless Rust workspace tests.',
            'Desktop GTK, devices, live archives, lab/browser checks and root smoke are excluded.',
        ],
    }
    if args.artifact_prefix:
        report['selectedChecks'].extend(['rustc-version', 'cargo-version', 'zig-version'])
    if args.verify:
        report['selectedChecks'].extend(['har-product-tests', 'har-quality-tests',
                                         'original-static-demos', 'sprint-static-demos'])
        if args.artifact_prefix:
            report['selectedChecks'].extend(['rust-format', 'rust-headless-tests'])
    try:
        return execute_checks(args, report)
    except (ValueError, OSError) as error:
        report['ok'] = False
        report['error'] = str(error)
        report['missingPrerequisites'].append(str(error))
        report['completedChecks'] = [check['name'] for check in report['checks']]
        report['notRunChecks'] = [name for name in report['selectedChecks'] if name not in report['completedChecks']]
        return report


def execute_checks(args, report):
    environment = child_environment()
    node = args.node or shutil.which('node', path=environment.get('PATH', ''))
    report['toolAvailability']['node'] = {'available': False, 'requested': args.node}
    if not node:
        raise ValueError('Node is unavailable; pass --node /path/to/node (>=26.13.1).')
    node = require_executable(Path(node))
    report['toolAvailability']['node']['available'] = True
    node_check = run([node, '--version'], environment)
    node_check['name'] = 'node-version'
    report['checks'].append(node_check)
    actual = version_tuple(node_check['output'])
    if node_check['exitCode'] != 0 or actual is None or actual < NODE_MINIMUM:
        raise ValueError('Node >=26.13.1 is required; selected executable failed the version check.')
    report['node'] = node
    report['toolAvailability']['node'].update({'available': True, 'version': node_check['output'].strip()})
    checks = report['checks']
    cargo = None
    if args.artifact_prefix:
        prefix = Path(args.artifact_prefix).resolve()
        report['toolAvailability']['rustPrefix'] = {'available': False, 'requested': str(prefix)}
        pin_match = re.search(r'^channel\s*=\s*"([^"]+)"',
                              (ROOT / 'rust-toolchain.toml').read_text(), re.MULTILINE)
        if not pin_match:
            raise ValueError('Cannot read exact channel from rust-toolchain.toml.')
        pin = pin_match.group(1)
        rust = prefix / f'rust-{pin}-x86_64-unknown-linux-gnu' / 'bin'
        zig = prefix / 'zig-x86_64-linux-0.17.1' / 'zig'
        wrappers = Path(__file__).resolve().parent
        cargo = require_executable(rust / 'cargo')
        overrides = {
            'RUSTC': require_executable(rust / 'rustc'),
            'RUSTDOC': require_executable(rust / 'rustdoc'),
            'RUSTFMT': require_executable(rust / 'rustfmt'),
            'CC': require_executable(wrappers / 'zig-cc'),
            'CXX': require_executable(wrappers / 'zig-cxx'),
            'AR': require_executable(wrappers / 'zig-ar'),
            'CARGO_HOME': str(prefix / 'cargo-home'),
            'RUSTUP_HOME': str(prefix / 'rustup-home'),
            'CARGO_TARGET_DIR': str(prefix / 'cargo-target'),
            'ZIG_GLOBAL_CACHE_DIR': str(prefix / 'zig-global-cache'),
            'ZIG_LOCAL_CACHE_DIR': str(prefix / 'zig-local-cache'),
            'SCOPE_TOOLCHAIN_ZIG': str(zig),
            'CARGO_BUILD_JOBS': '2', 'CARGO_TERM_COLOR': 'never',
            'CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER': str(wrappers / 'zig-cc'),
        }
        require_executable(zig)
        environment.update(overrides)
        environment['PATH'] = str(rust) + os.pathsep + environment.get('PATH', '')
        for name, executable, arguments, expected in [
            ('rustc-version', overrides['RUSTC'], ['--version'], pin),
            ('cargo-version', cargo, ['--version'], pin),
            ('zig-version', str(zig), ['version'], '0.17.1'),
        ]:
            check = run([executable, *arguments], environment)
            check['name'] = name
            checks.append(check)
            report['toolAvailability'][name.removesuffix('-version')] = {'available': True, 'version': check['output'].strip()}
            if check['exitCode'] or version_tuple(check['output']) != version_tuple(expected):
                raise ValueError(f'Selected tool does not match version {expected}: {executable}')
        report['rust'] = {'status': 'local versions inspected', 'channel': pin,
                          'artifactPrefix': str(prefix), 'childOverrides': overrides}
        report['toolAvailability']['rustPrefix']['available'] = True
    if args.verify:
        # Keep the scope aligned with the prototype package's current test globs.
        tests = sorted(str(path) for folder in ('test', 'contracts', 'json', 'formats')
                       for path in (PROTOTYPE / folder).glob('*.test.mjs'))
        if not tests:
            raise ValueError('No prototype tests found.')
        quality_tests = sorted(str(path) for path in (PROTOTYPE / 'quality').glob('*.test.mjs'))
        if not quality_tests:
            raise ValueError('No quality harness tests found.')
        commands = [
            ('har-product-tests', [node, '--test', *tests], 300),
            ('har-quality-tests', [node, '--test', *quality_tests], 300),
            ('original-static-demos', [node, str(PROTOTYPE / 'examples' / 'run-demo.mjs')], 120),
            ('sprint-static-demos', [node, str(PROTOTYPE / 'examples' / 'sprint' / 'run.mjs')], 120),
        ]
        if cargo:
            commands.extend([
                ('rust-format', [cargo, 'fmt', '--all', '--', '--check'], 120),
                ('rust-headless-tests', [cargo, 'test', '--workspace', '--exclude', 'scope-desktop',
                  '--locked', '--offline'], 1200),
            ])
        report['cargoLockBefore'] = lock_hash()
        protected = [path for path in ('Cargo.lock', 'package-lock.json') if (ROOT / path).is_file()]
        report['protectedLocksBefore'] = {path: hashlib.sha256((ROOT / path).read_bytes()).hexdigest() for path in protected}
        for name, command, timeout in commands:
            print(name + ': ' + ' '.join(command[:5]) + (' …' if len(command) > 5 else ''),
                  file=sys.stderr, flush=True)
            check = run(command, environment, timeout)
            check['name'] = name
            if name.endswith('-tests'):
                count = re.search(r'^# tests (\d+)$', check['output'], re.MULTILINE)
                skipped = re.search(r'^# skipped (\d+)$', check['output'], re.MULTILINE)
                if count:
                    check['tests'] = int(count.group(1))
                    check['skipped'] = int(skipped.group(1)) if skipped else None
                elif name == 'rust-headless-tests':
                    check['tests'] = sum(int(value) for value in re.findall(r'test result: ok\. (\d+) passed;', check['output']))
            checks.append(check)
            if check['exitCode']:
                if name == 'rust-headless-tests' and re.search(r'offline|failed to download|no matching package', check['output'], re.IGNORECASE):
                    report['missingPrerequisites'].append('Prepared Cargo dependency cache is incomplete for offline verification; inspect the failed named check.')
                break
        report['cargoLockAfter'] = lock_hash()
        report['protectedLocksAfter'] = {path: hashlib.sha256((ROOT / path).read_bytes()).hexdigest() if (ROOT / path).is_file() else None for path in protected}
    report['ok'] = all(check['exitCode'] == 0 for check in checks)
    if args.verify and report['protectedLocksBefore'] != report['protectedLocksAfter']:
        report['ok'] = False
        report['error'] = 'A protected dependency lock changed during verification.'
    report['completedChecks'] = [check['name'] for check in checks]
    report['notRunChecks'] = [name for name in report['selectedChecks'] if name not in report['completedChecks']]
    return report


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--node', help='Explicit existing Node executable; otherwise resolve PATH.')
    parser.add_argument('--artifact-prefix', help='Existing toolchains directory; enables optional Rust/Zig checks.')
    parser.add_argument('--verify', action='store_true', help='Run named HAR product/quality tests, original/sprint demos and optional headless Rust checks offline.')
    parser.add_argument('--output', type=Path, help='Also write JSON to a new file; refuses existing paths.')
    args = parser.parse_args()
    # Fail before checks rather than running a long verification with an unusable receipt path.
    if args.output and (args.output.exists() or not args.output.parent.is_dir()):
        parser.error('--output must name a new file in an existing directory.')
    try:
        report = inspect(args)
    except (ValueError, OSError) as error:
        report = {'schemaVersion': 1, 'ok': False, 'error': str(error)}
    encoded = json.dumps(report, indent=2) + '\n'
    if args.output:
        with args.output.open('x', encoding='utf-8') as destination:
            destination.write(encoded)
    sys.stdout.write(encoded)
    return 0 if report['ok'] else 2


if __name__ == '__main__':
    raise SystemExit(main())
