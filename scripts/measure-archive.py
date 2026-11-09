#!/usr/bin/env python3
"""Opt-in synthetic archive measurement. Exports only bounded numeric diagnostics.

No external endpoints, native security changes, production configuration changes,
or timing gates in normal CI. Build/test output remains in a disposable directory.
"""
import argparse
import json
import os
from pathlib import Path
import platform
import re
import signal
import subprocess
import sys
import tempfile
import time

ROOT = Path(__file__).resolve().parents[1]
WINDOWS = os.name == 'nt'


def terminate_and_reap(child):
    """Bounded cleanup of this owned process tree; return cleanup uncertainty."""
    uncertain = False
    if WINDOWS:
        try:
            killed = subprocess.run(['taskkill', '/PID', str(child.pid), '/T', '/F'],
                                    stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
                                    timeout=10, check=False)
            uncertain = killed.returncode != 0
        except (OSError, subprocess.TimeoutExpired):
            uncertain = True  # Direct kill cannot confirm descendant cleanup.
    else:
        try:
            os.killpg(child.pid, signal.SIGKILL)
        except ProcessLookupError:
            pass
        except OSError:
            uncertain = True
    try:
        if child.poll() is None:
            child.kill()
    except ProcessLookupError:
        pass
    except OSError:
        uncertain = True
    try:
        child.wait(timeout=10)
    except (OSError, subprocess.TimeoutExpired):
        uncertain = True
    return uncertain


def run_bounded(command, cwd, env, timeout):
    """Own the child tree until normal completion, deadline or parent exception."""
    kwargs = {'start_new_session': True} if os.name != 'nt' else {'creationflags': subprocess.CREATE_NEW_PROCESS_GROUP}
    with tempfile.TemporaryFile() as output:
        child = None
        timed_out = False
        cleanup_uncertain = False
        try:
            child = subprocess.Popen(command, cwd=cwd, env=env, stdout=output, stderr=subprocess.STDOUT, **kwargs)
            try:
                child.wait(timeout=timeout)
            except subprocess.TimeoutExpired:
                timed_out = True
                cleanup_uncertain = terminate_and_reap(child)
            output.seek(0)
            # Cargo JSON is bounded here; do not export compiler output or local paths.
            captured = output.read(16 * 1024 * 1024).decode('utf-8', errors='replace')
        except BaseException as error:
            if child is not None:
                try:
                    uncertain = terminate_and_reap(child)
                except BaseException:
                    uncertain = True
                if uncertain:
                    error.add_note('Process-tree termination or reaping could not be fully confirmed.')
            raise
    return {'status': 'TIMEOUT' if timed_out else ('PASS' if child.returncode == 0 else 'FAIL'),
            'returncode': child.returncode, 'output': captured, 'cleanup_uncertain': cleanup_uncertain}


def validate_report(report, profile, workers, samples):
    """Accept only the explicit, bounded synthetic report schema; never raw data."""
    def reject():
        raise ValueError('invalid synthetic diagnostic schema')

    def obj(value, keys):
        if type(value) is not dict or set(value) != set(keys.split()):
            reject()
        return value

    def integer(value, low=0, high=(1 << 64) - 1):
        if type(value) is not int or not low <= value <= high:
            reject()
        return value

    def boolean(value):
        if type(value) is not bool:
            reject()
        return value

    def fixed(value, expected):
        if type(value) is not type(expected) or value != expected:
            reject()

    def choice(value, allowed):
        if type(value) is not str or value not in allowed:
            reject()

    def array(value, cap):
        if type(value) is not list or len(value) > cap:
            reject()
        return value

    def probes(value):
        obj(value, 'timer network period_ms cap_per_probe window_start_ns window_end_ns')
        fixed(value['period_ms'], 10)
        fixed(value['cap_per_probe'], 8192)
        start = integer(value['window_start_ns'])
        end = integer(value['window_end_ns'], start)
        for name in ('timer', 'network'):
            series = obj(value[name], 'elapsed_ns samples failures dropped missed_periods in_window_samples window_status')
            for key in ('failures', 'dropped', 'missed_periods', 'in_window_samples'):
                integer(series[key])
            elapsed = array(series['elapsed_ns'], 8192)
            for number in elapsed:
                integer(number)
            observations = array(series['samples'], 8192)
            for observation in observations:
                obj(observation, 'scheduled_ns start_ns end_ns elapsed_ns success in_window')
                began = integer(observation['start_ns'])
                ended = integer(observation['end_ns'], began)
                measured = integer(observation['elapsed_ns'])
                boolean(observation['success'])
                boolean(observation['in_window'])
                if name == 'timer':
                    scheduled = integer(observation['scheduled_ns'], high=began)
                    expected_window = end > start and start <= scheduled < end
                    fixed(measured, began - scheduled)
                    fixed(ended, began)
                    fixed(observation['success'], True)
                else:
                    fixed(observation['scheduled_ns'], None)
                    expected_window = end > start and began < end and ended > start
                    fixed(measured, ended - began)
                fixed(observation['in_window'], expected_window)
            if elapsed != [s['elapsed_ns'] for s in observations if s['success']]:
                reject()
            fixed(series['in_window_samples'], sum(s['in_window'] for s in observations))
            expected_status = 'INVALID' if series['dropped'] else ('PASS' if series['in_window_samples'] else 'NOTRUN')
            fixed(series['window_status'], expected_status)
            recorded_failures = sum(not s['success'] for s in observations)
            if not recorded_failures <= series['failures'] <= recorded_failures + series['dropped']:
                reject()
        return value

    # Options are emitted by the pinned bundled SQLite. Permit platform/compiler
    # differences, never arbitrary option names, prose, filesystem paths or URLs.
    option_names = set('''ATOMIC_INTRINSICS DEFAULT_AUTOVACUUM DEFAULT_CACHE_SIZE DEFAULT_FILE_FORMAT
        DEFAULT_FOREIGN_KEYS DEFAULT_JOURNAL_SIZE_LIMIT DEFAULT_MMAP_SIZE DEFAULT_PAGE_SIZE
        DEFAULT_PCACHE_INITSZ DEFAULT_RECURSIVE_TRIGGERS DEFAULT_SECTOR_SIZE DEFAULT_SYNCHRONOUS
        DEFAULT_WAL_AUTOCHECKPOINT DEFAULT_WAL_SYNCHRONOUS DEFAULT_WORKER_THREADS DIRECT_OVERFLOW_READ
        ENABLE_API_ARMOR ENABLE_COLUMN_METADATA ENABLE_DBSTAT_VTAB ENABLE_FTS3 ENABLE_FTS3_PARENTHESIS
        ENABLE_FTS3_TOKENIZER ENABLE_FTS4 ENABLE_FTS5 ENABLE_LOAD_EXTENSION ENABLE_MATH_FUNCTIONS
        ENABLE_MEMORY_MANAGEMENT ENABLE_RTREE ENABLE_STAT4 HAVE_ISNAN HAVE_LOCALTIME_R HAVE_LOCALTIME_S
        HAVE_MALLOC_USABLE_SIZE HAVE_USLEEP MALLOC_SOFT_LIMIT MAX_ATTACHED MAX_COLUMN MAX_COMPOUND_SELECT
        MAX_DEFAULT_PAGE_SIZE MAX_EXPR_DEPTH MAX_FUNCTION_ARG MAX_LENGTH MAX_LIKE_PATTERN_LENGTH
        MAX_MMAP_SIZE MAX_PAGE_COUNT MAX_PAGE_SIZE MAX_SQL_LENGTH MAX_TRIGGER_DEPTH MAX_VARIABLE_NUMBER
        MAX_VDBE_OP MAX_WORKER_THREADS MUTEX_NOOP MUTEX_PTHREADS MUTEX_W32 SOUNDEX SYSTEM_MALLOC
        TEMP_STORE THREADSAFE USE_URI'''.split())

    def sqlite(value):
        obj(value, 'version journal_mode synchronous busy_timeout_ms page_size cache_size wal_autocheckpoint compile_options')
        if type(value['version']) is not str or not re.fullmatch(r'3\.\d{1,3}\.\d{1,3}', value['version']):
            reject()
        fixed(value['journal_mode'], 'wal')
        integer(value['synchronous'], 0, 3)
        for key in ('busy_timeout_ms', 'page_size', 'wal_autocheckpoint'):
            integer(value[key])
        integer(value['cache_size'], -(1 << 63), (1 << 63)-1)
        options = array(value['compile_options'], 128)
        for option in options:
            if type(option) is not str or len(option) > 128:
                reject()
            if option.startswith('COMPILER='):
                if not re.fullmatch(r'COMPILER=(?:(?:clang|gcc)-[0-9][0-9.]{0,30}|msvc-[0-9]{1,8})', option):
                    reject()
            else:
                match = re.fullmatch(r'([A-Z][A-Z0-9_]*)(?:=(-?(?:[0-9]{1,20}|0x[0-9a-fA-F]{1,16})))?', option)
                if not match or match[1] not in option_names:
                    reject()
        if len(set(options)) != len(options):
            reject()

    if profile not in ('debug', 'release') or type(workers) is not int or workers not in (0, 2) or type(samples) is not int or not 1 <= samples <= 5:
        reject()
    obj(report, 'schema status profile workers runtime cases timer_network_positive_control not_run')
    fixed(report['schema'], 1)
    fixed(report['profile'], profile)
    fixed(report['workers'], workers)
    fixed(report['runtime'], 'current_thread' if workers == 0 else 'multi_thread')
    choice(report['status'], ('PASS', 'FAIL'))
    cases = array(report['cases'], 10)
    fixed(len(cases), 2 * samples)
    seen = set()
    evidence_keys = ('evidence_valid', 'reopened_evidence_valid', 'evidence_corruption_control_detected',
                     'evidence_loss_control_detected', 'control_baseline_restored')
    for case in cases:
        obj(case, '''status sample transport clients joined successful client_elapsed_ns batch_elapsed_ns
            shutdown_elapsed_ns cooperative_batch_timeout wall_budget_exceeded client_timeout_s batch_timeout_s
            persisted_rows evidence_valid reopened_evidence_valid revision_delta evidence_corruption_control_detected
            evidence_loss_control_detected control_baseline_restored save_cap saves probes sqlite client_configuration storage''')
        choice(case['status'], ('PASS', 'FAIL'))
        choice(case['transport'], ('HTTP', 'TLS'))
        integer(case['sample'], 0, samples-1)
        identity = (case['transport'], case['sample'])
        if identity in seen:
            reject()
        seen.add(identity)
        for key, expected in [('clients', 32), ('client_timeout_s', 30), ('batch_timeout_s', 60), ('save_cap', 256)]:
            fixed(case[key], expected)
        for key in ('joined', 'successful'):
            integer(case[key], 0, 32)
        integer(case['persisted_rows'], 0, 33)
        integer(case['revision_delta'], -(1 << 63), (1 << 63)-1)
        for key in ('batch_elapsed_ns', 'shutdown_elapsed_ns'):
            integer(case[key])
        for key in evidence_keys + ('cooperative_batch_timeout', 'wall_budget_exceeded'):
            boolean(case[key])
        for number in array(case['client_elapsed_ns'], 32):
            integer(number)
        saves = obj(case['saves'], 'dropped samples')
        integer(saves['dropped'])
        for sample in array(saves['samples'], 256):
            obj(sample, 'serialization_ns mutex_wait_ns statement_ns serialized_bytes success')
            for key in ('serialization_ns', 'mutex_wait_ns', 'statement_ns', 'serialized_bytes'):
                integer(sample[key])
            boolean(sample['success'])
        probes(case['probes'])
        sqlite(case['sqlite'])
        configuration = obj(case['client_configuration'], 'http pool tls barrier')
        for key, expected in [('http', '1.1 only'), ('pool', 'shared client clones'),
                ('tls', 'fresh archive and client per batch; interception CA trusted; no external upstream'), ('barrier', 32)]:
            fixed(configuration[key], expected)
        fixed(case['storage'], 'fresh temporary file-backed archive; no PRAGMA overrides')
        if case['status'] == 'PASS':
            if not (case['joined'] == case['successful'] == case['persisted_rows'] == len(case['client_elapsed_ns']) == 32
                    and all(case[key] for key in evidence_keys) and case['revision_delta'] == 192
                    and not case['cooperative_batch_timeout'] and not case['wall_budget_exceeded']
                    and case['batch_elapsed_ns'] <= 60_000_000_000
                    and all(n <= 30_000_000_000 for n in case['client_elapsed_ns'])
                    and saves['dropped'] == 0 and len(saves['samples']) == 192
                    and all(s['success'] for s in saves['samples'])
                    and all(case['probes'][name]['dropped'] == 0 for name in ('timer', 'network'))):
                reject()
    control = report['timer_network_positive_control']
    if workers == 0:
        obj(control, 'status injected_block_ms detector_threshold_ms probes')
        choice(control['status'], ('PASS', 'FAIL'))
        fixed(control['injected_block_ms'], 300)
        fixed(control['detector_threshold_ms'], 100)
        observed = probes(control['probes'])
        if control['status'] == 'PASS' and not (
                any(s['in_window'] and s['elapsed_ns'] > 100_000_000 for s in observed['timer']['samples'])
                and any(s['in_window'] and (not s['success'] or s['elapsed_ns'] > 100_000_000) for s in observed['network']['samples'])
                and observed['timer']['dropped'] == observed['network']['dropped'] == 0):
            reject()
    else:
        obj(control, 'status reason')
        fixed(control['status'], 'NOTRUN')
        fixed(control['reason'], 'intentional blocking control requires current-thread runtime')
    not_run = obj(report['not_run'], 'queue_wait disk_flush_count sqlite_busy_wait_separate production_performance gui_android')
    for key, expected in [('queue_wait', 'no writer queue exists'), ('disk_flush_count', 'not observed'),
            ('sqlite_busy_wait_separate', 'included in statement elapsed'),
            ('production_performance', 'synthetic test-only harness'), ('gui_android', 'outside harness')]:
        fixed(not_run[key], expected)
    if report['status'] == 'PASS' and not (all(case['status'] == 'PASS' for case in cases) and (workers == 2 or control['status'] == 'PASS')):
        reject()

def resource_limits():
    values = {'logical_cpus': os.cpu_count(), 'affinity_cpus': None,
              'cgroup_memory_max_bytes': None, 'cgroup_cpu_quota_us': None, 'cgroup_cpu_period_us': None,
              'peak_child_rss': 'NOTRUN', 'filesystem_type': 'NOTRUN', 'io_quota': 'NOTRUN'}
    if hasattr(os, 'sched_getaffinity'):
        values['affinity_cpus'] = len(os.sched_getaffinity(0))
    for field, file in [('cgroup_memory_max_bytes', '/sys/fs/cgroup/memory.max')]:
        try:
            text = Path(file).read_text().strip()
            if text.isdigit():
                values[field] = int(text)
        except OSError:
            pass
    try:
        quota, period = Path('/sys/fs/cgroup/cpu.max').read_text().split()
        values['cgroup_cpu_quota_us'] = int(quota) if quota.isdigit() else None
        values['cgroup_cpu_period_us'] = int(period)
    except (OSError, ValueError):
        pass
    return values


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--profiles', nargs='+', choices=['debug', 'release'], default=['debug', 'release'])
    parser.add_argument('--samples', type=int, choices=range(1, 6), default=3)
    parser.add_argument('--workers', type=int, nargs='+', choices=[0, 2], default=[0, 2])
    parser.add_argument('--output-dir', type=Path, required=True)
    args = parser.parse_args()
    output_dir = args.output_dir.resolve()
    output_dir.mkdir(parents=True, exist_ok=True)
    env = os.environ.copy()
    # Comparisons must use the repository's declared profiles, not hidden tuning.
    if any(key.startswith('CARGO_PROFILE_') or key.endswith('_RUSTFLAGS') for key in env) or any(env.get(key) for key in ('RUSTFLAGS', 'RUSTC_WRAPPER', 'RUSTC_WORKSPACE_WRAPPER')):
        parser.error('remove profile/Rust flag overrides before measurement')
    env['CARGO_BUILD_JOBS'] = '2'
    revision = subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=ROOT, text=True).strip()
    if not re.fullmatch(r'[0-9a-f]{40}', revision):
        parser.error('invalid source revision')
    dirty = bool(subprocess.check_output(['git', 'status', '--porcelain'], cwd=ROOT))
    compiler = subprocess.check_output(['rustc', '--version'], text=True).strip()
    if not re.fullmatch(r'rustc 1\.99\.1 \([a-z0-9]+ [0-9-]+\)', compiler):
        parser.error('use the pinned Rust 1.99.1 compiler')
    hosted = {}
    for name in ('GITHUB_RUN_ID', 'GITHUB_RUN_ATTEMPT', 'ImageOS', 'ImageVersion'):
        value = env.get(name)
        hosted[name] = value if value and re.fullmatch(r'[a-zA-Z0-9._-]{1,80}', value) else None
    metadata = {'source_commit': revision, 'source_dirty': dirty, 'compiler': compiler,
                'os': platform.system(), 'architecture': platform.machine(), 'python': platform.python_version(),
                'hosted': hosted, 'resources': resource_limits(), 'build_jobs': 2,
                'profiles': args.profiles, 'workers': args.workers, 'samples_per_transport': args.samples,
                'build_hard_timeout_s': 600, 'child_hard_timeout_s': 420, 'overall_budget_s': 1800,
                'optimization': {'debug': 'repository profile: opt-level=0,debug=1', 'release': 'repository profile: opt-level=3,lto=thin,strip=true'},
                'profile_limits': 'environment flag/profile/wrapper overrides rejected; user Cargo configuration is not inspected; hosted jobs use clean checkout and toolchain',
                'storage': 'OS temporary directory; fresh file-backed SQLite archive per case; path omitted',
                'unavailable_resource_fields': 'null means NOTRUN or not exposed by this platform'}
    results = []
    overall_deadline = time.monotonic() + 1800
    cleanup_uncertain = False
    def bounded(command, child_env, maximum):
        nonlocal cleanup_uncertain
        if cleanup_uncertain:
            return {'status': 'NOTRUN', 'returncode': None, 'output': '', 'cleanup_uncertain': True}
        remaining = overall_deadline - time.monotonic() - 30
        if remaining <= 0:
            return {'status': 'NOTRUN', 'returncode': None, 'output': '', 'cleanup_uncertain': False}
        result = run_bounded(command, ROOT, child_env, min(maximum, remaining))
        cleanup_uncertain = result['cleanup_uncertain']
        return result
    for profile in dict.fromkeys(args.profiles):
        command = ['cargo', 'test', '--locked', '-p', 'scope-core', '--lib', '--no-run', '--message-format=json']
        if profile == 'release':
            command.append('--release')
        built = bounded(command, env, 600)
        binary = None
        if built['status'] == 'PASS':
            for line in built['output'].splitlines():
                try:
                    event = json.loads(line)
                except ValueError:
                    continue
                if event.get('reason') == 'compiler-artifact' and event.get('target', {}).get('name') == 'scope_core' and event.get('profile', {}).get('test') and event.get('executable'):
                    binary = event['executable']
        if not binary:
            results.append({'profile': profile, 'status': built['status'] if built['status'] != 'PASS' else 'FAIL', 'reason': 'cleanup_uncertain' if built['cleanup_uncertain'] else 'build_or_artifact_failure', 'cleanup_uncertain': built['cleanup_uncertain']})
            continue
        for workers in dict.fromkeys(args.workers):
            with tempfile.TemporaryDirectory(prefix='scope-measure-') as temp:
                report_path = Path(temp) / 'report.json'
                child_env = dict(env, SCOPE_MEASURE_OUTPUT=str(report_path), SCOPE_MEASURE_WORKERS=str(workers), SCOPE_MEASURE_SAMPLES=str(args.samples))
                result = bounded([binary, '--ignored', '--exact', 'diagnostics::archive_responsiveness', '--nocapture'], child_env, 420)
                record = {'profile': profile, 'workers': workers, 'status': result['status'], 'reason': 'cleanup_uncertain' if result['cleanup_uncertain'] else None, 'report': None, 'cleanup_uncertain': result['cleanup_uncertain']}
                if report_path.exists() and report_path.stat().st_size <= 8 * 1024 * 1024:
                    try:
                        report = json.loads(report_path.read_text(encoding='utf-8'))
                        validate_report(report, profile, workers, args.samples)
                        record['report'] = report
                        if report['status'] != 'PASS':
                            record['status'] = 'FAIL'
                    except (ValueError, TypeError, KeyError):
                        record['status'], record['reason'] = 'FAIL', 'invalid_report'
                else:
                    if record['reason'] is None:
                        record['reason'] = 'report_unavailable'
                    if record['status'] == 'PASS':
                        record['status'] = 'FAIL'
                results.append(record)
            print(f'{profile} workers={workers}: {record["status"]}', flush=True)
    summary = {'schema': 1, 'status': 'PASS' if results and all(r['status'] == 'PASS' for r in results) else 'FAIL',
               'metadata': metadata, 'results': results, 'cleanup_uncertain': cleanup_uncertain}
    (output_dir / 'measurement.json').write_text(json.dumps(summary, indent=2) + '\n', encoding='utf-8')
    return 0 if summary['status'] == 'PASS' else 1


if __name__ == '__main__':
    raise SystemExit(main())
