"""Host helper checks. These do not execute Android or Scope hooks."""
import importlib.util
from pathlib import Path
import ssl
import tempfile
import unittest
import urllib.request

ROOT = Path(__file__).resolve().parents[1]


def load(name):
    spec = importlib.util.spec_from_file_location(name, ROOT / (name + '.py'))
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


class BuildTests(unittest.TestCase):
    def test_corrupt_cached_jar_is_rejected_before_any_build(self):
        build = load('build')
        with tempfile.TemporaryDirectory() as temp:
            jar = Path(temp) / 'bad.jar'
            jar.write_bytes(b'corrupt')
            with self.assertRaisesRegex(ValueError, 'SHA-256'):
                build.verify_jar(jar, '0' * 64)

    def test_corrupt_build_exits_before_needing_sdk_or_jdk(self):
        import subprocess
        import sys
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            # This intentionally owned build directory reaches dependency validation.
            (root / '.scope-engine-fixture-build').write_text('scope-engine-fixture-v1\n')
            (root / 'okhttp-3.14.9.jar').write_bytes(b'corrupt')
            completed = subprocess.run([sys.executable, str(ROOT / 'build.py'), '--sdk', str(root / 'missing-sdk'),
                                        '--jdk', str(root / 'missing-jdk'), '--output', str(root), '--offline'],
                                       capture_output=True, text=True, timeout=5)
            self.assertNotEqual(completed.returncode, 0)
            self.assertIn('SHA-256 mismatch', completed.stderr)
            self.assertNotIn('Required supplied tool missing', completed.stderr)

    def test_output_cannot_write_generated_keys_into_source(self):
        build = load('build')
        with self.assertRaisesRegex(ValueError, 'outside'):
            build.validate_output(ROOT / 'build')
        with tempfile.TemporaryDirectory() as temp:
            self.assertEqual(build.validate_output(Path(temp)), Path(temp).resolve())

    def test_public_ca_input_rejects_private_key_and_oversized_input(self):
        build = load('build')
        self.assertTrue(callable(getattr(build, 'read_public_ca', None)), 'public CA input validation is implemented')
        with tempfile.TemporaryDirectory() as temp:
            path = Path(temp) / 'input.pem'
            for content in [b'-----BEGIN PRIVATE KEY-----\nsecret\n-----END PRIVATE KEY-----', b'X' * 65537]:
                path.write_bytes(content)
                with self.assertRaisesRegex(ValueError, 'public PEM'):
                    build.read_public_ca(path)

    def test_public_ca_requires_real_complete_ca_der(self):
        import subprocess
        build, serve = load('build'), load('serve')
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            serve.generate_fixtures(root)
            genuine = (root / 'scope-ca.pem').read_text()
            self.assertEqual(build.read_public_ca(root / 'scope-ca.pem'), genuine)
            genuine_der = ssl.PEM_cert_to_DER_cert(genuine)
            private_der = subprocess.run(['openssl', 'genpkey', '-algorithm', 'RSA', '-pkeyopt', 'rsa_keygen_bits:1024', '-outform', 'DER'],
                                         capture_output=True, check=True, timeout=10).stdout
            for case, der in [('malformed', b'ABC'), ('relabelled-private-key', private_der), ('trailing-bytes', genuine_der + b'ABC')]:
                with self.subTest(case=case):
                    path = root / 'invalid.pem'
                    path.write_text(ssl.DER_cert_to_PEM_cert(der))
                    with self.assertRaisesRegex(ValueError, 'public PEM'):
                        build.read_public_ca(path)
            with self.assertRaisesRegex(ValueError, 'public PEM'):
                build.read_public_ca(root / 'scope.pem')  # A genuine server leaf is not a CA.

    def test_owned_output_can_be_prepared_again_without_losing_files(self):
        build = load('build')
        with tempfile.TemporaryDirectory() as temp:
            root = build.prepare_output(Path(temp) / 'new-build')
            (root / 'keep.txt').write_text('owned output')
            self.assertEqual(build.prepare_output(root), root)
            self.assertEqual((root / 'keep.txt').read_text(), 'owned output')

    def test_unowned_output_is_preserved_before_permissions_or_cleanup(self):
        import os
        import stat
        import subprocess
        import sys
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            for name in ['classes', 'dex', 'assets']:
                (root / name).mkdir()
                (root / name / 'keep.txt').write_text('unrelated')
            os.chmod(root, 0o755)
            completed = subprocess.run([sys.executable, str(ROOT / 'build.py'), '--sdk', str(root / 'missing-sdk'),
                                        '--jdk', str(root / 'missing-jdk'), '--output', str(root), '--offline'],
                                       capture_output=True, text=True, timeout=5)
            self.assertNotEqual(completed.returncode, 0)
            self.assertIn('unowned', completed.stderr)
            self.assertEqual(stat.S_IMODE(root.stat().st_mode), 0o755)
            self.assertFalse((root / '.scope-engine-fixture-build').exists())
            for name in ['classes', 'dex', 'assets']:
                self.assertEqual((root / name / 'keep.txt').read_text(), 'unrelated')


class FixtureTests(unittest.TestCase):
    def test_real_tls_scope_foreign_wrong_host_and_expired_controls(self):
        serve = load('serve')
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            serve.generate_fixtures(root)
            context = ssl.create_default_context(cafile=str(root / 'scope-ca.pem'))
            with serve.running_servers(root, 0, 0, seconds=15, max_connections=12) as urls:
                with urllib.request.urlopen(urls['http'], timeout=3) as response:
                    self.assertEqual(response.read(), b'scope-engine-fixture\n')
                with urllib.request.urlopen(urls['scope'], context=context, timeout=3) as response:
                    self.assertEqual(response.status, 200)
                for case in ['foreign', 'wrong-host', 'expired']:
                    with self.subTest(case=case):
                        with self.assertRaises(urllib.error.URLError) as caught:
                            urllib.request.urlopen(urls[case], context=context, timeout=3)
                        self.assertIsInstance(caught.exception.reason, ssl.SSLCertVerificationError)
                        expected = {'foreign': 'unable to get local issuer', 'wrong-host': 'IP address mismatch', 'expired': 'expired'}[case]
                        self.assertIn(expected, str(caught.exception.reason))

    def test_connection_limit_counts_requests_rather_than_idle_polling(self):
        serve = load('serve')
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            serve.generate_fixtures(root)
            with serve.running_servers(root, 0, 0, seconds=3, max_connections=1) as urls:
                import time
                time.sleep(0.5)
                with urllib.request.urlopen(urls['http'], timeout=1) as response:
                    self.assertEqual(response.status, 200)

    def test_servers_have_a_finite_deadline_and_release_ports(self):
        serve = load('serve')
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            serve.generate_fixtures(root)
            with serve.running_servers(root, 0, 0, seconds=0.3, max_connections=1) as urls:
                import time
                time.sleep(0.7)
                with self.assertRaises(urllib.error.URLError):
                    urllib.request.urlopen(urls['http'], timeout=1)


if __name__ == '__main__':
    unittest.main()
