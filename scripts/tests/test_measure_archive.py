"""Behavioral tests for the bounded measurement process and report trust boundary."""
import copy
import json
import importlib.util
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import patch
import subprocess

SPEC = importlib.util.spec_from_file_location('measure_archive', Path(__file__).parents[1] / 'measure-archive.py')
measure = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(measure)


def valid_report():
    def probes(control=False):
        start, end = (0, 300_000_000) if control else (100, 200)
        timer_start, timer_end = (10_000_000, 300_000_000) if control else (150, 160)
        timer = {'scheduled_ns': timer_start, 'start_ns': timer_end, 'end_ns': timer_end,
                 'elapsed_ns': timer_end-timer_start, 'success': True, 'in_window': True}
        network = dict(timer, scheduled_ns=None, start_ns=timer_start)
        def values(sample):
            return {'elapsed_ns':[sample['elapsed_ns']], 'samples':[sample], 'failures':0,
                    'dropped':0, 'missed_periods':0, 'in_window_samples':1, 'window_status':'PASS'}
        return {'timer':values(timer), 'network':values(network), 'period_ms':10,
                'cap_per_probe':8192, 'window_start_ns':start, 'window_end_ns':end}
    case = {'status':'PASS', 'sample':0, 'transport':'HTTP', 'clients':32, 'joined':32,
            'successful':32, 'client_elapsed_ns':[50]*32, 'batch_elapsed_ns':100,
            'shutdown_elapsed_ns':1, 'cooperative_batch_timeout':False, 'wall_budget_exceeded':False,
            'client_timeout_s':30, 'batch_timeout_s':60, 'persisted_rows':32, 'evidence_valid':True,
            'reopened_evidence_valid':True, 'revision_delta':192, 'evidence_corruption_control_detected':True,
            'evidence_loss_control_detected':True, 'control_baseline_restored':True, 'save_cap':256,
            'saves':{'dropped':0,'samples':[{'serialization_ns':1,'mutex_wait_ns':1,'statement_ns':1,
                     'serialized_bytes':100,'success':True} for _ in range(192)]},
            'probes':probes(), 'sqlite':{'version':'3.51.1','journal_mode':'wal','synchronous':2,
            'busy_timeout_ms':5000,'page_size':4096,'cache_size':-2000,'wal_autocheckpoint':1000,
            'compile_options':['COMPILER=clang-22.1.8','THREADSAFE=1','MUTEX_PTHREADS']},
            'client_configuration':{'http':'1.1 only','pool':'shared client clones',
            'tls':'fresh archive and client per batch; interception CA trusted; no external upstream','barrier':32},
            'storage':'fresh temporary file-backed archive; no PRAGMA overrides'}
    tls = copy.deepcopy(case); tls['transport'] = 'TLS'
    return {'schema':1,'status':'PASS','workers':0,'profile':'debug','runtime':'current_thread',
            'cases':[case,tls], 'timer_network_positive_control':{'status':'PASS','injected_block_ms':300,
            'detector_threshold_ms':100,'probes':probes(True)}, 'not_run':{'queue_wait':'no writer queue exists',
            'disk_flush_count':'not observed','sqlite_busy_wait_separate':'included in statement elapsed',
            'production_performance':'synthetic test-only harness','gui_android':'outside harness'}}


class MeasurementRunnerTests(unittest.TestCase):
    def test_normal_completion_preserves_output_and_exit_status(self):
        with tempfile.TemporaryDirectory() as directory:
            for code, status in [(0, 'PASS'), (3, 'FAIL')]:
                result = measure.run_bounded([sys.executable, '-c',
                    f"import sys; print('synthetic output'); sys.exit({code})"],
                    Path(directory), {}, 5)
                self.assertEqual(result['status'], status)
                self.assertEqual(result['returncode'], code)
                self.assertEqual(result['output'].strip(), 'synthetic output')
                self.assertFalse(result['cleanup_uncertain'])

    def test_hard_deadline_kills_and_reaps_stalled_child(self):
        with tempfile.TemporaryDirectory() as directory:
            result = measure.run_bounded([sys.executable, '-c', 'import time; time.sleep(10)'], Path(directory), {}, .1)
            self.assertEqual(result['status'], 'TIMEOUT')
            self.assertIsNotNone(result['returncode'])

    def test_taskkill_failure_still_kills_and_reaps_child(self):
        with tempfile.TemporaryDirectory() as directory:
            with patch.object(measure, 'WINDOWS', True), patch.object(measure.subprocess, 'run', side_effect=subprocess.TimeoutExpired('taskkill', 10)):
                result = measure.run_bounded([sys.executable, '-c', 'import time; time.sleep(10)'], Path(directory), {}, .1)
            self.assertEqual(result['status'], 'TIMEOUT')
            self.assertIsNotNone(result['returncode'])
            self.assertTrue(result['cleanup_uncertain'])

    def test_report_accepts_complete_numeric_fixture(self):
        measure.validate_report(valid_report(), 'debug', 0, 1)

    def test_report_rejects_unknown_fields_at_every_boundary(self):
        fixture = valid_report()
        containers = [fixture, fixture['cases'][0], fixture['cases'][0]['saves'],
                      fixture['cases'][0]['saves']['samples'][0], fixture['cases'][0]['probes'],
                      fixture['cases'][0]['probes']['timer'], fixture['cases'][0]['probes']['timer']['samples'][0],
                      fixture['cases'][0]['sqlite'], fixture['cases'][0]['client_configuration'],
                      fixture['not_run'], fixture['timer_network_positive_control']]
        for container in containers:
            with self.subTest(keys=list(container)):
                container['unexpected_secret'] = 'fixture-token'
                with self.assertRaises(ValueError):
                    measure.validate_report(fixture, 'debug', 0, 1)
                del container['unexpected_secret']

    def test_report_rejects_injected_strings_missing_fields_and_wrong_types(self):
        mutations = [lambda r: r['cases'][0].update(storage='fixture-token'),
                     lambda r: r['not_run'].update(queue_wait='fixture-token'),
                     lambda r: r['cases'][0]['sqlite'].update(compile_options=['SECRET=fixture-token']),
                     lambda r: r['cases'][0].pop('evidence_valid'),
                     lambda r: r['cases'][0].update(clients=True),
                     lambda r: r['cases'][0].update(batch_elapsed_ns=1.2),
                     lambda r: r['cases'][0].update(evidence_valid=1),
                     lambda r: r['cases'][0]['probes']['timer']['samples'][0].update(elapsed_ns=-1)]
        for mutate in mutations:
            fixture = valid_report(); mutate(fixture)
            with self.assertRaises(ValueError):
                measure.validate_report(fixture, 'debug', 0, 1)

    def test_taskkill_nonzero_result_marks_descendant_cleanup_uncertain(self):
        with tempfile.TemporaryDirectory() as directory:
            failed = subprocess.CompletedProcess(['taskkill'], 1)
            with patch.object(measure, 'WINDOWS', True), patch.object(measure.subprocess, 'run', return_value=failed):
                result = measure.run_bounded([sys.executable, '-c', 'import time; time.sleep(10)'], Path(directory), {}, .1)
            self.assertEqual(result['status'], 'TIMEOUT')
            self.assertTrue(result['cleanup_uncertain'])
            self.assertIsNotNone(result['returncode'])

    def test_uncertain_cleanup_stops_all_subsequent_child_launches(self):
        with tempfile.TemporaryDirectory() as directory:
            args = ['measure-archive.py', '--profiles', 'debug', 'release', '--workers', '0', '2',
                    '--samples', '1', '--output-dir', directory]
            uncertain = {'status':'TIMEOUT', 'returncode':-9, 'output':'', 'cleanup_uncertain':True}
            with patch.object(sys, 'argv', args), patch.dict(measure.os.environ, {}, clear=True), \
                    patch.object(measure.subprocess, 'check_output', side_effect=['a'*40, b'', 'rustc 1.99.1 (abcdef 2026-10-01)']), \
                    patch.object(measure.platform, 'system', return_value='SyntheticOS'), \
                    patch.object(measure.platform, 'machine', return_value='synthetic-arch'), \
                    patch.object(measure.platform, 'python_version', return_value='3.12.10'), \
                    patch.object(measure, 'resource_limits', return_value={}), \
                    patch.object(measure, 'run_bounded', return_value=uncertain) as launch:
                self.assertEqual(measure.main(), 1)
            self.assertEqual(launch.call_count, 1)
            summary = json.loads((Path(directory)/'measurement.json').read_text())
            self.assertTrue(summary['cleanup_uncertain'])
            self.assertEqual(summary['status'], 'FAIL')
            self.assertEqual([r['status'] for r in summary['results']], ['TIMEOUT', 'NOTRUN'])
            self.assertTrue(all(r['reason'] == 'cleanup_uncertain' for r in summary['results']))

    def test_report_accepts_release_two_worker_and_failed_diagnostics(self):
        fixture = valid_report()
        fixture.update(profile='release', workers=2, runtime='multi_thread',
                       timer_network_positive_control={'status':'NOTRUN',
                       'reason':'intentional blocking control requires current-thread runtime'})
        measure.validate_report(fixture, 'release', 2, 1)
        fixture['status'] = fixture['cases'][0]['status'] = 'FAIL'
        fixture['cases'][0]['evidence_valid'] = False
        measure.validate_report(fixture, 'release', 2, 1)

    def test_report_rejects_bounded_array_and_integer_overflow(self):
        mutations = [lambda r: r['cases'][0].update(client_elapsed_ns=[1]*33),
                     lambda r: r['cases'][0]['saves'].update(samples=r['cases'][0]['saves']['samples']*2),
                     lambda r: r['cases'][0]['probes']['timer'].update(samples=[{}]*8193),
                     lambda r: r['cases'][0]['sqlite'].update(compile_options=['THREADSAFE=1']*129),
                     lambda r: r['cases'][0].update(batch_elapsed_ns=2**64),
                     lambda r: r['cases'][0].update(batch_elapsed_ns=float('nan'))]
        for mutate in mutations:
            fixture = valid_report(); mutate(fixture)
            with self.assertRaises(ValueError):
                measure.validate_report(fixture, 'debug', 0, 1)

    def test_report_rejects_pass_with_missing_evidence_or_false_window_coverage(self):
        mutations = [lambda r: r['cases'][0].update(evidence_valid=False),
                     lambda r: r['cases'][0]['probes']['timer'].update(in_window_samples=0),
                     lambda r: r['cases'][0]['probes']['network']['samples'][0].update(in_window=False),
                     lambda r: r['cases'][0]['saves']['samples'][0].update(success=False)]
        for mutate in mutations:
            fixture = valid_report(); mutate(fixture)
            with self.assertRaises(ValueError):
                measure.validate_report(fixture, 'debug', 0, 1)

    def test_report_rejects_profile_mismatch(self):
        with self.assertRaises(ValueError):
            measure.validate_report({'schema':1,'status':'PASS','profile':'release','workers':0,'cases':[]}, 'debug', 0, 1)

    def test_report_rejects_missing_and_unbounded_cases(self):
        for cases in ([], [{}]*11):
            with self.assertRaises(ValueError):
                measure.validate_report({'schema':1,'status':'PASS','profile':'debug','workers':0,'cases':cases}, 'debug', 0, 1)


if __name__ == '__main__':
    unittest.main()
