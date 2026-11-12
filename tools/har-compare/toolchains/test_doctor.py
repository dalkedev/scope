"""Process-coordinator checks using disposable repositories and controlled tools.

The tiny executables are real child processes, not Rust/Node implementations.
They expose missing prerequisites, command flags and lock preservation failures
without downloads, application credentials or access to the real root locks.
"""
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import unittest

SOURCE = Path(__file__).resolve().parent


class DoctorProcessTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(prefix='scope-doctor-tests-')
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.tools = self.root / 'tools/har-compare/toolchains'
        self.tools.mkdir(parents=True)
        for name in ('doctor.py', 'zig_driver.py', 'zig-cc', 'zig-cxx', 'zig-ar'):
            shutil.copy2(SOURCE / name, self.tools / name)
        (self.root / 'Cargo.lock').write_text('synthetic-root-lock\n')
        (self.root / 'package-lock.json').write_text('{"synthetic":true}\n')
        (self.root / 'rust-toolchain.toml').write_text('[toolchain]\nchannel = "1.99.1"\n')
        for folder in ('test', 'quality'):
            path = self.root / 'tools/har-compare' / folder
            path.mkdir()
            (path / 'fixture.test.mjs').write_text('// synthetic coordinator fixture\n')
        self.node = self.root / 'node'
        self.write_executable(self.node, "import sys\nprint('v26.13.1' if '--version' in sys.argv else '# tests 1\\n# skipped 0')\n")
        self.prefix = self.root / 'toolchains'
        binaries = self.prefix / 'rust-1.99.1-x86_64-unknown-linux-gnu/bin'
        binaries.mkdir(parents=True)
        self.write_executable(binaries / 'rustc', "print('rustc 1.99.1')\n")
        self.write_executable(binaries / 'rustdoc', "print('rustdoc 1.99.1')\n")
        self.write_executable(binaries / 'rustfmt', "print('rustfmt 1.99.1')\n")
        self.write_executable(binaries / 'cargo', "import sys\nprint('cargo 1.99.1' if '--version' in sys.argv else 'test result: ok. 1 passed;')\n")
        zig = self.prefix / 'zig-x86_64-linux-0.17.1/zig'
        zig.parent.mkdir()
        self.write_executable(zig, "print('0.17.1')\n")
        self.cargo = binaries / 'cargo'

    @staticmethod
    def write_executable(path, source):
        path.write_text('#!' + sys.executable + '\n' + source)
        path.chmod(0o755)

    def invoke(self, *arguments):
        return subprocess.run([sys.executable, '-B', str(self.tools / 'doctor.py'), *arguments],
                              cwd=self.root, capture_output=True, text=True, timeout=20)

    def test_missing_node_reports_prerequisite_without_creating_caches(self):
        result = self.invoke('--node', str(self.root / 'missing'))
        self.assertEqual(result.returncode, 2)
        report = json.loads(result.stdout)
        self.assertFalse(report['ok'])
        self.assertFalse(report['toolAvailability']['node']['available'])
        self.assertTrue(report['missingPrerequisites'])
        self.assertEqual(report['notRunChecks'], ['node-version'])
        self.assertFalse((self.prefix / 'cargo-home').exists())

    def test_invalid_node_version_keeps_probe_evidence(self):
        self.write_executable(self.node, "print('v26.13.0')\n")
        result = self.invoke('--node', str(self.node))
        report = json.loads(result.stdout)
        self.assertEqual(result.returncode, 2)
        self.assertEqual(report['checks'][0]['output'], 'v26.13.0\n')
        self.assertTrue(report['toolAvailability']['node']['available'])
        self.assertTrue(report['missingPrerequisites'])

    def test_receipt_collision_is_rejected_before_a_tool_runs(self):
        marker = self.root / 'ran-node'
        self.write_executable(self.node, f"from pathlib import Path\nPath({str(marker)!r}).touch()\nprint('v26.13.1')\n")
        output = self.root / 'receipt.json'
        output.write_text('preserved synthetic receipt\n')
        result = self.invoke('--node', str(self.node), '--output', str(output))
        self.assertEqual(result.returncode, 2)
        self.assertEqual(output.read_text(), 'preserved synthetic receipt\n')
        self.assertFalse(marker.exists())

    def test_default_doctor_preserves_locks_and_creates_no_caches(self):
        before = {name: (self.root / name).read_bytes() for name in ('Cargo.lock', 'package-lock.json')}
        result = self.invoke('--node', str(self.node), '--artifact-prefix', str(self.prefix))
        report = json.loads(result.stdout)
        self.assertEqual(result.returncode, 0, result.stdout)
        self.assertEqual(len(report['checks']), 4)
        self.assertEqual(report['selectedChecks'], report['completedChecks'])
        self.assertEqual(report['notRunChecks'], [])
        for name, content in before.items():
            self.assertEqual((self.root / name).read_bytes(), content)
        self.assertFalse((self.prefix / 'cargo-home').exists())
        self.assertFalse((self.prefix / 'cargo-target').exists())

    def test_verification_names_product_quality_and_sprint_separately(self):
        result = self.invoke('--node', str(self.node), '--verify')
        report = json.loads(result.stdout)
        self.assertEqual(result.returncode, 0, result.stdout)
        self.assertEqual(report['completedChecks'], ['node-version', 'har-product-tests', 'har-quality-tests', 'original-static-demos', 'sprint-static-demos'])
        self.assertEqual(report['checks'][1]['tests'], 1)
        self.assertEqual(report['checks'][2]['tests'], 1)
        self.assertEqual(report['protectedLocksBefore'], report['protectedLocksAfter'])

    def test_changed_protected_lock_fails_even_when_child_claims_success(self):
        self.write_executable(self.cargo, "import sys\nfrom pathlib import Path\nif '--version' in sys.argv: print('cargo 1.99.1')\nelif 'test' in sys.argv: Path('package-lock.json').write_text('changed synthetic lock')\n")
        result = self.invoke('--node', str(self.node), '--artifact-prefix', str(self.prefix), '--verify')
        report = json.loads(result.stdout)
        self.assertEqual(result.returncode, 2)
        self.assertFalse(report['ok'])
        self.assertNotEqual(report['protectedLocksBefore'], report['protectedLocksAfter'])
        self.assertIn('protected dependency lock changed', report['error'])

    def test_offline_dependency_failure_has_failed_named_check_and_prerequisite(self):
        self.write_executable(self.cargo, "import sys\nif '--version' in sys.argv: print('cargo 1.99.1')\nelif 'test' in sys.argv:\n assert '--locked' in sys.argv and '--offline' in sys.argv\n print('failed to download synthetic dependency: --offline was specified')\n raise SystemExit(101)\n")
        result = self.invoke('--node', str(self.node), '--artifact-prefix', str(self.prefix), '--verify')
        report = json.loads(result.stdout)
        self.assertEqual(result.returncode, 2)
        self.assertFalse(report['ok'])
        self.assertEqual(report['checks'][-1]['name'], 'rust-headless-tests')
        self.assertEqual(report['checks'][-1]['exitCode'], 101)
        self.assertTrue(report['missingPrerequisites'])
        self.assertEqual(report['protectedLocksBefore'], report['protectedLocksAfter'])
        self.assertFalse((self.prefix / 'cargo-home').exists())


if __name__ == '__main__':
    unittest.main(verbosity=2)
