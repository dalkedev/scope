"""Worker contract tests; fake Frida calls are not Android coverage."""
import builtins
import importlib.util
import json
from pathlib import Path
import ssl
import subprocess
import tempfile
import time
import unittest
from unittest import mock

SOURCE = Path(__file__).resolve().parents[1] / "frida_worker.py"
SPEC = importlib.util.spec_from_file_location("scope_frida_worker", SOURCE)
WORKER = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(WORKER)
FIXTURES = Path(__file__).resolve().parent / "fixtures"
# Public synthetic certificates only; their disposable signing keys are not retained.
PEM = (FIXTURES / "ca.pem").read_text(encoding="ascii").strip()


class WorkerTests(unittest.TestCase):
    def test_profiles_are_shared_normalized_and_unsupported_fail(self):
        self.assertTrue(hasattr(WORKER, "normalize_profiles"), "worker provides normalized profile contract")
        self.assertEqual(WORKER.normalize_profiles([]), ["proxy", "certificate", "unpinning"])
        self.assertEqual(WORKER.normalize_profiles(["certificate", "proxy", "certificate"]), ["proxy", "certificate"])
        for profiles in [["root"], ["system-ca"], ["unpinning"], ["unknown"], "proxy", [None]]:
            with self.assertRaises(ValueError):
                WORKER.normalize_profiles(profiles)

    def test_builder_serializes_data_and_loads_only_owned_engine_and_bridge(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            ca = root / "ca.pem"
            ca.write_text(PEM)
            (root / "frida-java-bridge.js").write_text("// bridge\n")
            (root / "scope-engine.js").write_text("globalThis.ScopeJavaEngine = {start(c) { globalThis.observed = c; }};\n")
            source = WORKER.build_script(root, ca, 17890, ["proxy"])
            result = subprocess.run(["node", "-e", source + "\nconsole.log(JSON.stringify(globalThis.observed))"], capture_output=True, text=True, check=True)
            self.assertEqual(json.loads(result.stdout), {"caPem": PEM, "proxyHost": "127.0.0.1", "proxyPort": 17890, "profiles": ["proxy"]})
            ca.write_text(PEM + "`;globalThis.pwned=true;//")
            with self.assertRaises(ValueError):
                WORKER.build_script(root, ca, 17890, ["proxy"])
            ca.write_text(PEM)
            for port in [0, 65536, True, "17890;evil()"]:
                with self.assertRaises(ValueError):
                    WORKER.build_script(root, ca, port, ["proxy"])

    def test_builder_serializes_public_ca_with_canonical_lf_for_all_newlines(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            ca = root / "ca.pem"
            (root / "frida-java-bridge.js").write_text("// bridge\n")
            (root / "scope-engine.js").write_text("globalThis.ScopeJavaEngine = {start(c) { globalThis.observed = c; }};\n")
            for newline in [b"\n", b"\r\n", b"\r"]:
                with self.subTest(newline=newline):
                    # Explicit bytes reproduce every platform independently of text I/O.
                    ca.write_bytes(PEM.encode("ascii").replace(b"\n", newline))
                    try:
                        source = WORKER.build_script(root, ca, 17890, ["proxy"])
                    except ValueError as error:
                        self.fail("Valid public CA newline form was rejected: " + str(error))
                    result = subprocess.run(["node", "-e", source + "\nconsole.log(JSON.stringify(globalThis.observed))"], capture_output=True, text=True, check=True)
                    self.assertEqual(json.loads(result.stdout)["caPem"], PEM)

    def test_builder_requires_one_complete_public_ca(self):
        # Missing DER/CA validation would serialize these inputs to the device.
        der = ssl.PEM_cert_to_DER_cert(PEM)
        # Complete PKCS#1 RSAPrivateKey ASN.1 with tiny known integers (n=15).
        # This unusable synthetic key needs no crypto tool or secret material.
        key_der = b"\x30\x1b" + b"".join(
            b"\x02\x01" + bytes([value]) for value in (0, 15, 3, 3, 3, 5, 1, 3, 2)
        )
        cases = {
            "malformed-der": ssl.DER_cert_to_PEM_cert(b"abc"),
            "relabelled-key": ssl.DER_cert_to_PEM_cert(key_der),
            "trailing-der": ssl.DER_cert_to_PEM_cert(der + b"abc"),
            "leaf": (FIXTURES / "leaf.pem").read_text(encoding="ascii"),
            "multiple-certificates": PEM + "\n" + PEM,
            "private-label": "-----BEGIN PRIVATE KEY-----\nYWJj\n-----END PRIVATE KEY-----",
            "oversized": " " * 65537,
            "non-ascii": "\N{SNOWMAN}",
        }
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            ca = root / "input.pem"
            (root / "frida-java-bridge.js").write_text("// bridge")
            (root / "scope-engine.js").write_text("// engine")
            for name, content in cases.items():
                with self.subTest(case=name):
                    ca.write_text(content, encoding="utf-8")
                    with self.assertRaisesRegex(ValueError, "Scope CA"):
                        WORKER.build_script(root, ca, 17890, ["proxy"])

    def test_main_rejects_invalid_ca_before_frida_import_or_device_work(self):
        # An import guard catches regression before any real Frida/device action.
        original_import = builtins.__import__
        def guarded_import(name, *args, **kwargs):
            if name == "frida":
                self.fail("Frida import reached before public CA validation")
            return original_import(name, *args, **kwargs)
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            ca = root / "input.pem"
            ca.write_text("-----BEGIN CERTIFICATE-----\nYWJj\n-----END CERTIFICATE-----")
            (root / "frida-java-bridge.js").write_text("// bridge")
            (root / "scope-engine.js").write_text("// engine")
            argv = ["frida_worker", "--serial", "synthetic", "--package", "app.test",
                    "--ca", str(ca), "--scripts", str(root), "--profile", "proxy"]
            with mock.patch.object(WORKER.sys, "argv", argv), mock.patch("builtins.__import__", side_effect=guarded_import):
                with self.assertRaisesRegex(ValueError, "Scope CA"):
                    WORKER.main()

    def test_exact_ca_size_boundary_and_new_contents_are_revalidated(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "frida-java-bridge.js").write_text("// bridge")
            (root / "scope-engine.js").write_text("// engine")
            ca = root / "ca.pem"
            public = PEM.encode("ascii")
            ca.write_bytes(public + b" " * (65536 - len(public)))
            self.assertIn("ScopeJavaEngine.start", WORKER.build_script(root, ca, 17890, ["proxy"]))
            ca.write_bytes(ca.read_bytes() + b" ")
            with self.assertRaisesRegex(ValueError, "64 KiB"):
                WORKER.build_script(root, ca, 17890, ["proxy"])
            ca.write_bytes(b"not a certificate")
            with self.assertRaisesRegex(ValueError, "Scope CA"):
                WORKER.build_script(root, ca, 17890, ["proxy"])

    def test_ready_gate_requires_engine_exact_profiles_and_real_capabilities(self):
        self.assertTrue(hasattr(WORKER, "validate_ready"), "worker validates installed capabilities")
        good = {"kind": "scope-hooks-ready", "engine": "scope-java-v1", "profiles": ["proxy"], "capabilities": {"proxy": ["java-url"]}}
        self.assertEqual(WORKER.validate_ready(good, ["proxy"]), good)
        for changes in [{"engine": "other"}, {"profiles": []}, {"capabilities": {}}, {"capabilities": {"proxy": []}}, {"capabilities": {"proxy": ["not-implemented"]}}]:
            with self.assertRaises(ValueError):
                WORKER.validate_ready({**good, **changes}, ["proxy"])

    def test_owned_cleanup_kills_exact_pid_before_unload_and_detach(self):
        self.assertTrue(hasattr(WORKER, "cleanup_owned"), "worker terminates its instrumented PID")
        calls = []
        device = mock.Mock()
        device.kill.side_effect = lambda pid: calls.append(("kill", pid))
        script = mock.Mock()
        script.unload.side_effect = lambda: calls.append(("unload",))
        session = mock.Mock()
        session.detach.side_effect = lambda: calls.append(("detach",))
        WORKER.cleanup_owned(device, 427, script, session, timeout=0.2)
        self.assertEqual(calls, [("kill", 427), ("unload",), ("detach",)])
        calls.clear()
        WORKER.cleanup_owned(device, None, None, None, timeout=0.2)
        self.assertEqual(calls, [])

    def test_cleanup_errors_and_timeouts_are_visible_and_bounded(self):
        self.assertTrue(hasattr(WORKER, "cleanup_owned"))
        device, script, session = mock.Mock(), mock.Mock(), mock.Mock()
        device.kill.side_effect = RuntimeError("permission denied")
        with self.assertRaisesRegex(RuntimeError, "kill.*permission denied"):
            WORKER.cleanup_owned(device, 427, script, session, timeout=0.05)
        script.unload.assert_called_once()
        session.detach.assert_called_once()
        device.kill.side_effect = lambda _pid: time.sleep(0.3)
        start = time.monotonic()
        with self.assertRaisesRegex(RuntimeError, "kill.*timed out"):
            WORKER.cleanup_owned(device, 427, None, None, timeout=0.02)
        self.assertLess(time.monotonic() - start, 0.2)

    def test_cli_rejects_unsupported_profile_before_import_or_device_contact(self):
        with mock.patch.object(WORKER.sys, "argv", ["frida_worker", "--profile", "root"]):
            with self.assertRaisesRegex(ValueError, "profile"):
                WORKER.main()

class SessionTests(unittest.TestCase):
    def fixture(self, behavior):
        calls = []
        device, session, script = mock.Mock(), mock.Mock(), mock.Mock()
        device.spawn.side_effect = lambda argv: calls.append(("spawn", argv)) or 427
        device.attach.return_value = session
        session.create_script.return_value = script
        callback = []
        script.on.side_effect = lambda _name, handler: callback.append(handler)
        script.load.side_effect = lambda: behavior(callback[0])
        device.kill.side_effect = lambda pid: calls.append(("kill", pid))
        script.unload.side_effect = lambda: calls.append(("unload",))
        session.detach.side_effect = lambda: calls.append(("detach",))
        return device, session, script, calls

    def test_stop_before_ready_cleans_owned_suspended_process_without_ready(self):
        stopped = WORKER.threading.Event()
        device, session, script, calls = self.fixture(lambda _cb: stopped.set())
        output = []
        with mock.patch.object(WORKER, "emit", side_effect=lambda kind, **fields: output.append({"kind": kind, **fields})):
            start = time.monotonic()
            WORKER.run_session(device, "app.test", "source", ["proxy"], "17.9.1", stopped, ready_timeout=30, cleanup_timeout=0.1)
        self.assertLess(time.monotonic() - start, 0.5)
        self.assertEqual(calls, [("spawn", ["app.test"]), ("kill", 427), ("unload",), ("detach",)])
        device.resume.assert_not_called()
        self.assertFalse(any(row["kind"] == "ready" for row in output))
        self.assertEqual(output[0], {"kind": "spawned", "engine": "scope-java-v1", "pid": 427})
        self.assertEqual(output[-1]["processTerminated"], True)
        self.assertEqual(output[-1]["pid"], 427)

    def test_late_ready_cannot_revive_hook_error_and_startup_failure_kills_pid(self):
        stopped = WORKER.threading.Event()
        def behavior(callback):
            callback({"type": "send", "payload": {"kind": "scope-hooks-error", "message": "no pinner"}}, None)
            callback({"type": "send", "payload": {"kind": "scope-hooks-ready", "engine": "scope-java-v1", "profiles": ["proxy"], "capabilities": {"proxy": ["java-url"]}}}, None)
        device, session, script, calls = self.fixture(behavior)
        output = []
        with mock.patch.object(WORKER, "emit", side_effect=lambda kind, **fields: output.append({"kind": kind, **fields})):
            with self.assertRaisesRegex(RuntimeError, "no pinner"):
                WORKER.run_session(device, "app.test", "source", ["proxy"], "17.9.1", stopped, cleanup_timeout=0.1)
        self.assertIn(("kill", 427), calls)
        self.assertFalse(any(row["kind"] == "ready" for row in output))

    def test_failed_cleanup_never_emits_clean_stopped(self):
        stopped = WORKER.threading.Event()
        device, session, script, calls = self.fixture(lambda _cb: stopped.set())
        device.kill.side_effect = RuntimeError("cannot kill")
        output = []
        with mock.patch.object(WORKER, "emit", side_effect=lambda kind, **fields: output.append({"kind": kind, **fields})):
            with self.assertRaisesRegex(RuntimeError, "cannot kill"):
                WORKER.run_session(device, "app.test", "source", ["proxy"], "17.9.1", stopped, cleanup_timeout=0.1)
        self.assertFalse(any(row["kind"] == "stopped" for row in output))

class DestroyedCleanupTests(unittest.TestCase):
    def test_already_destroyed_script_and_detached_session_need_no_second_cleanup(self):
        device, script, session = mock.Mock(), mock.Mock(), mock.Mock()
        script.is_destroyed = True
        session.is_detached = True
        script.unload.side_effect = RuntimeError("script has been destroyed")
        session.detach.side_effect = RuntimeError("already detached")
        WORKER.cleanup_owned(device, 427, script, session, timeout=0.1)
        device.kill.assert_called_once_with(427)
        script.unload.assert_not_called()
        session.detach.assert_not_called()

class StartupCancellationTests(unittest.TestCase):
    fixture = SessionTests.fixture
    def test_stop_cancels_inflight_script_load_then_kills_only_owned_pid(self):
        stopped = WORKER.threading.Event()
        cancelled = WORKER.threading.Event()
        class Cancellable:
            def __enter__(self):
                return self
            def __exit__(self, *_args):
                pass
            def cancel(self):
                cancelled.set()
        device, session, script, calls = self.fixture(lambda _cb: None)
        def blocked_load():
            stopped.set()
            if not cancelled.wait(0.5):
                raise RuntimeError("startup was not cancelled")
            raise RuntimeError("operation cancelled")
        script.load.side_effect = blocked_load
        with mock.patch.object(WORKER, "emit"):
            with self.assertRaisesRegex(RuntimeError, "operation cancelled"):
                WORKER.run_session(device, "app.test", "source", ["proxy"], "17.9.1", stopped, cleanup_timeout=0.1, cancellable_factory=Cancellable)
        self.assertTrue(cancelled.is_set())
        self.assertIn(("kill", 427), calls)

class ReviewedLifecycleTests(unittest.TestCase):
    fixture = SessionTests.fixture

    def test_pre_spawn_stop_has_explicit_safe_no_spawn_receipt(self):
        stopped = WORKER.threading.Event()
        stopped.set()
        device = mock.Mock()
        output = []
        with mock.patch.object(WORKER, "emit", side_effect=lambda kind, **fields: output.append({"kind": kind, **fields})):
            WORKER.run_session(device, "app.test", "source", ["proxy"], "17.9.1", stopped)
        device.spawn.assert_not_called()
        self.assertEqual(output, [{"kind": "stopped", "engine": "scope-java-v1", "pid": None, "spawnAttempted": False, "cleanupComplete": True, "processTerminated": False}])

    def test_replacement_before_kill_dispatch_never_kills_new_image(self):
        stopped = WORKER.threading.Event()
        device, session, script, calls = self.fixture(lambda _cb: stopped.set())
        detached = []
        session.on.side_effect = lambda _name, handler: detached.append(handler)
        bounded = WORKER.bounded_call
        def interleave(label, operation, timeout):
            if label == "kill owned application":
                detached[0]("process-replaced")
            return bounded(label, operation, timeout)
        output = []
        with mock.patch.object(WORKER, "bounded_call", side_effect=interleave), mock.patch.object(WORKER, "emit", side_effect=lambda kind, **fields: output.append({"kind": kind, **fields})):
            with self.assertRaisesRegex(RuntimeError, "replaced|ownership"):
                WORKER.run_session(device, "app.test", "source", ["proxy"], "17.9.1", stopped, cleanup_timeout=0.1)
        device.kill.assert_not_called()
        self.assertFalse(any(row["kind"] == "stopped" for row in output))

    def test_replacement_cancels_dispatched_kill_and_retains_uncertainty(self):
        stopped = WORKER.threading.Event()
        cancelled = WORKER.threading.Event()
        class Cancellable:
            def __enter__(self): return self
            def __exit__(self, *_args): pass
            def cancel(self): cancelled.set()
        device, session, script, calls = self.fixture(lambda _cb: stopped.set())
        detached = []
        session.on.side_effect = lambda _name, handler: detached.append(handler)
        def kill(_pid):
            detached[0]("process-replaced")
            if not cancelled.wait(0.2):
                raise RuntimeError("pending kill was not cancelled")
        device.kill.side_effect = kill
        with mock.patch.object(WORKER, "emit") as output:
            with self.assertRaisesRegex(RuntimeError, "unknown|uncertain"):
                WORKER.run_session(device, "app.test", "source", ["proxy"], "17.9.1", stopped, cleanup_timeout=0.3, cancellable_factory=Cancellable)
        self.assertTrue(cancelled.is_set())
        self.assertFalse(any(call.args[0] == "stopped" for call in output.call_args_list))


class OwnershipEvidenceTests(unittest.TestCase):
    fixture = SessionTests.fixture

    def test_attempted_spawn_without_returned_pid_never_claims_no_spawn_cleanup(self):
        stopped = WORKER.threading.Event()
        device = mock.Mock()
        device.spawn.side_effect = RuntimeError("connection lost during spawn")
        with mock.patch.object(WORKER, "emit") as output:
            with self.assertRaisesRegex(RuntimeError, "completion is unknown"):
                WORKER.run_session(device, "app.test", "source", ["proxy"], "17.9.1", stopped)
        device.kill.assert_not_called()
        self.assertFalse(any(call.args[0] == "stopped" for call in output.call_args_list))

    def test_observed_exit_before_kill_dispatch_skips_pid_and_acknowledges_termination(self):
        stopped = WORKER.threading.Event()
        device, session, script, calls = self.fixture(lambda _cb: stopped.set())
        detached = []
        session.on.side_effect = lambda _name, handler: detached.append(handler)
        bounded = WORKER.bounded_call
        def interleave(label, operation, timeout):
            if label == "kill owned application":
                detached[0]("process-terminated")
            return bounded(label, operation, timeout)
        with mock.patch.object(WORKER, "bounded_call", side_effect=interleave), mock.patch.object(WORKER, "emit") as output:
            WORKER.run_session(device, "app.test", "source", ["proxy"], "17.9.1", stopped, cleanup_timeout=0.1)
        device.kill.assert_not_called()
        receipts = [call.kwargs for call in output.call_args_list if call.args[0] == "stopped"]
        self.assertEqual(len(receipts), 1)
        self.assertIs(receipts[0]["processTerminated"], True)
        self.assertIs(receipts[0]["spawnAttempted"], True)
        self.assertIs(receipts[0]["cleanupComplete"], True)


if __name__ == "__main__":
    unittest.main()
