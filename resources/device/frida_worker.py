#!/usr/bin/env python3
"""Scope's isolated Frida worker. Stdout is JSON Lines; stdin EOF requests Stop.

The worker owns one spawned application process. Stop ends that process because
unloading methods cannot revoke already-created proxy or TLS objects.
"""
import argparse
from collections import deque
from contextlib import nullcontext
import json
from pathlib import Path
import re
import signal
import ssl
import sys
import threading
import time

PROFILES = ("proxy", "certificate", "unpinning")
CAPABILITIES = {"proxy": "java-url", "certificate": "sslcontext-scope-ca", "unpinning": "okhttp3-certificate-pinner"}


def emit(kind, **fields):
    if "message" in fields:
        fields["message"] = re.sub(r"(https?://[^\s?]+)\?[^\s]+", r"\1?[query omitted]", str(fields["message"]))[:4000]
    print(json.dumps({"kind": kind, **fields}), flush=True)


def normalize_profiles(profiles):
    if not isinstance(profiles, (list, tuple)) or any(not isinstance(p, str) or p not in PROFILES for p in profiles):
        raise ValueError("Unsupported hook profile; supported: proxy, certificate, unpinning")
    result = [p for p in PROFILES if p in profiles] if profiles else list(PROFILES)
    if "unpinning" in result and "certificate" not in result:
        raise ValueError("unpinning profile requires certificate")
    return result


def build_script(root, ca, port, profiles):
    profiles = normalize_profiles(profiles)
    if type(port) is not int or not 1 <= port <= 65535:
        raise ValueError("A valid proxy port is required")
    try:
        # Bound the read itself, including files that grow after opening.
        with ca.open("rb") as stream:
            data = stream.read(65537)
        if len(data) > 65536:
            raise ValueError("oversized CA")
        # Preserve universal-newline behavior of the former text-mode read.
        cert = data.decode("ascii").replace("\r\n", "\n").replace("\r", "\n").strip()
        if not re.fullmatch(r"-----BEGIN CERTIFICATE-----\s+[A-Za-z0-9+/=\s]+-----END CERTIFICATE-----", cert, flags=re.ASCII):
            raise ValueError("invalid PEM")
        der = ssl.PEM_cert_to_DER_cert(cert)
        # Fresh context with no system roots: enumerate only this public CA.
        # PEM decoding alone also accepts relabelled keys and trailing DER.
        context = ssl.SSLContext(ssl.PROTOCOL_TLS_CLIENT)
        context.load_verify_locations(cadata=cert)
        anchors = context.get_ca_certs(binary_form=True)
        if len(anchors) != 1 or anchors[0] != der:
            raise ValueError("incomplete or non-CA certificate")
    except (OSError, UnicodeError, ValueError, ssl.SSLError):
        raise ValueError("Scope CA must be one valid complete public PEM CA certificate, at most 64 KiB") from None
    config = {"caPem": cert, "proxyHost": "127.0.0.1", "proxyPort": port, "profiles": profiles}
    return "\n".join([
        (root / "frida-java-bridge.js").read_text(encoding="utf-8"),
        (root / "scope-engine.js").read_text(encoding="utf-8"),
        "globalThis.ScopeJavaEngine.start(" + json.dumps(config, ensure_ascii=True) + ");\n",
    ])


def validate_ready(payload, profiles):
    selected = normalize_profiles(profiles)
    if not isinstance(payload, dict) or payload.get("kind") != "scope-hooks-ready" or payload.get("engine") != "scope-java-v1" or payload.get("profiles") != selected:
        raise ValueError("Invalid Scope engine readiness identity or profiles")
    capabilities = payload.get("capabilities")
    if not isinstance(capabilities, dict) or set(capabilities) != set(selected):
        raise ValueError("Missing requested profile capabilities")
    for profile in selected:
        installed = capabilities[profile]
        if not isinstance(installed, list) or any(not isinstance(c, str) for c in installed) or CAPABILITIES[profile] not in installed:
            raise ValueError("Missing installed capability for profile " + profile)
    return payload


def bounded_call(label, operation, timeout):
    done = threading.Event()
    outcome = []

    def run():
        try:
            outcome.append((True, operation()))
        except BaseException as error:
            outcome.append((False, error))
        finally:
            done.set()

    threading.Thread(target=run, daemon=True).start()
    if not done.wait(timeout):
        raise RuntimeError(label + " timed out; completion is unknown")
    success, value = outcome[0]
    if not success:
        raise RuntimeError(label + ": " + str(value)) from value
    return value


class ProcessOwnership:
    """Current local evidence about one spawn, not an atomic remote PID handle."""

    def __init__(self):
        self.exited = threading.Event()
        self.replaced = threading.Event()
        self.lock = threading.Lock()
        self.pending_kill = None
        self.kill_dispatched = False
        self.kill_uncertain = False

    def observe(self, reason):
        pending = None
        with self.lock:
            if reason == "process-terminated":
                self.exited.set()
            elif reason == "process-replaced":
                self.replaced.set()
                if self.kill_dispatched:
                    self.kill_uncertain = True
                    pending = self.pending_kill
        if pending is not None:
            pending.cancel()

    def kill_current(self, operation, cancellable):
        try:
            with cancellable if cancellable is not None else nullcontext():
                with self.lock:
                    if self.replaced.is_set():
                        raise RuntimeError("Process ownership lost: original image was replaced; refusing kill")
                    if self.exited.is_set():
                        return
                    self.pending_kill = cancellable
                    self.kill_dispatched = True
                # Check inside the destructive operation's worker thread, rather
                # than relying on the caller's earlier ownership snapshot.
                if self.replaced.is_set():
                    raise RuntimeError("Process ownership lost before kill dispatch; refusing kill")
                if self.exited.is_set():
                    return
                operation()
        finally:
            with self.lock:
                self.pending_kill = None
                self.kill_dispatched = False
                uncertain = self.kill_uncertain
            if uncertain:
                raise RuntimeError("Kill completion is unknown after in-flight process replacement; cancellation cannot prove remote outcome")


def cleanup_owned(device, pid, script, session, timeout=3, process_not_found=(), ownership=None, cancellable_factory=None):
    errors = []
    ownership = ownership if ownership is not None else ProcessOwnership()

    def kill_pid():
        try:
            device.kill(pid)
        except process_not_found:
            ownership.observe("process-terminated")

    def kill():
        cancellable = cancellable_factory() if cancellable_factory else None
        ownership.kill_current(kill_pid, cancellable)

    def unload():
        if script.is_destroyed is not True:
            try:
                script.unload()
            except Exception:
                if script.is_destroyed is not True:
                    raise

    def detach():
        if session.is_detached is not True:
            try:
                session.detach()
            except Exception:
                if session.is_detached is not True:
                    raise

    operations = []
    if pid is not None:
        operations.append(("kill owned application", kill))
    if script is not None:
        operations.append(("unload hooks", unload))
    if session is not None:
        operations.append(("detach session", detach))
    for label, operation in operations:
        try:
            bounded_call(label, operation, timeout)
        except Exception as error:
            errors.append(str(error))
    if errors:
        raise RuntimeError("Cleanup failed: " + "; ".join(errors))


def run_session(device, package, source, profiles, version, stopped, ready_timeout=30, cleanup_timeout=3, process_not_found=(), cancellable_factory=None):
    profiles = normalize_profiles(profiles)
    ready = threading.Event()
    ownership = ProcessOwnership()
    failure = deque(maxlen=2)
    receipt = []
    replaced = ownership.replaced
    session = script = pid = None

    def receive(message, _data):
        payload = message.get("payload")
        if message.get("type") == "send" and isinstance(payload, dict):
            if payload.get("kind") == "scope-hooks-ready":
                if failure or stopped.is_set():
                    return
                try:
                    receipt[:] = [validate_ready(payload, profiles)]
                    ready.set()
                except ValueError as error:
                    failure.append(str(error))
                    stopped.set()
                return
            if payload.get("kind") == "scope-hooks-error":
                failure.append(str(payload.get("message", "Hook installation failed"))[:2000])
                stopped.set()
                return
        if message.get("type") == "error":
            failure.append(str(message.get("stack") or message.get("description") or "Hook error")[:2000])
            stopped.set()
        emit("log", message=message.get("stack") or payload or message.get("description") or message)

    def detached(reason, crash=None):
        ownership.observe(reason)
        if reason == "process-replaced":
            failure.append("Owned process image was replaced; termination cannot be confirmed safely")
        emit("detached", message=reason, crash=bool(crash))
        stopped.set()

    primary_error = None
    startup_complete = threading.Event()
    deadline = time.monotonic() + ready_timeout
    cancellable = cancellable_factory() if cancellable_factory else None

    def cancel_startup():
        while not startup_complete.wait(0.025):
            if stopped.is_set():
                cancellable.cancel()
                return
            if time.monotonic() >= deadline:
                failure.append("Startup deadline expired; pending operation completion may be unknown")
                stopped.set()
                cancellable.cancel()
                return

    if cancellable is not None:
        threading.Thread(target=cancel_startup, daemon=True).start()
    attempted_spawn = False
    try:
        if stopped.is_set():
            return
        with cancellable if cancellable is not None else nullcontext():
            attempted_spawn = True
            pid = device.spawn([package])
            emit("spawned", engine="scope-java-v1", pid=pid)
            session = device.attach(pid)
            session.on("detached", detached)
            script = session.create_script(source)
            script.on("message", receive)
            script.load()
            if not stopped.is_set():
                device.resume(pid)
        startup_complete.set()
        while not ready.is_set() and not stopped.is_set() and time.monotonic() < deadline:
            ready.wait(min(0.05, max(0, deadline - time.monotonic())))
        if failure:
            raise RuntimeError("Hook startup failed: " + "; ".join(failure))
        if stopped.is_set():
            if failure:
                raise RuntimeError("Hook startup failed: " + "; ".join(failure))
            return
        if not ready.is_set():
            raise RuntimeError("Java hooks did not become ready within the startup deadline")
        emit("ready", pid=pid, version=version, engine=receipt[0]["engine"], profiles=profiles, capabilities=receipt[0]["capabilities"])
        stopped.wait()
        if failure:
            raise RuntimeError("Hook runtime failed: " + "; ".join(failure))
    except BaseException as error:
        primary_error = RuntimeError("Spawn did not return an owned PID; completion is unknown: " + str(error)) if attempted_spawn and pid is None else error
    finally:
        startup_complete.set()
        try:
            cleanup_owned(device, pid, script, session, cleanup_timeout, process_not_found, ownership, cancellable_factory)
        except Exception as error:
            if primary_error:
                raise RuntimeError(str(primary_error) + "; " + str(error)) from primary_error
            raise
        if replaced.is_set():
            raise RuntimeError("Owned process image was replaced; cleanup outcome is uncertain") from primary_error
        if not attempted_spawn:
            emit("stopped", engine="scope-java-v1", pid=None, spawnAttempted=False, cleanupComplete=True, processTerminated=False)
        elif pid is not None:
            emit("stopped", engine="scope-java-v1", pid=pid, spawnAttempted=True, cleanupComplete=True, processTerminated=True, message="Owned application process ended; hooks unloaded and session detached")
    if primary_error:
        raise primary_error


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--serial")
    parser.add_argument("--version", action="store_true", help="Report bundled Frida version without contacting a device")
    parser.add_argument("--package")
    parser.add_argument("--address")
    parser.add_argument("--ca", type=Path)
    parser.add_argument("--scripts", type=Path)
    parser.add_argument("--port", type=int, default=17890)
    parser.add_argument("--profile", action="append", default=[])
    parser.add_argument("--list-apps", action="store_true")
    args = parser.parse_args()
    profiles = normalize_profiles(args.profile)  # Reject stale profiles before any device operation.
    source = None
    if not args.version and not args.list_apps:
        if not args.package or not args.ca or not args.scripts:
            raise ValueError("Package, public CA and scripts are required")
        source = build_script(args.scripts, args.ca, args.port, profiles)
    try:
        import frida
    except ImportError as error:
        raise RuntimeError("Frida Python is missing. Create a virtualenv, install resources/device/requirements-frida.txt and set SCOPE_PYTHON to its Python executable.") from error
    if args.version:
        emit("version", version=frida.__version__)
        return
    if not args.serial:
        parser.error("--serial is required for device operations")
    manager = frida.get_device_manager()
    device = manager.add_remote_device(args.address) if args.address else manager.get_device(args.serial, timeout=5)
    if args.list_apps:
        emit("apps", apps=[{"package": app.identifier, "name": app.name, "running": bool(app.pid)} for app in device.enumerate_applications()])
        return
    stopped = threading.Event()

    def wait_for_stop():
        for line in sys.stdin:
            if line.strip() == "stop":
                break
        stopped.set()

    signal.signal(signal.SIGINT, lambda *_: stopped.set())
    signal.signal(signal.SIGTERM, lambda *_: stopped.set())
    threading.Thread(target=wait_for_stop, daemon=True).start()
    emit("log", message="Experimental Scope Java hooks; Stop ends the spawned app. Native TLS/QUIC and custom pinners are not covered.")
    run_session(device, args.package, source, profiles, frida.__version__, stopped, process_not_found=(frida.ProcessNotFoundError,), cancellable_factory=frida.Cancellable)


if __name__ == "__main__":
    try:
        main()
    except Exception as error:
        emit("error", message=str(error))
        sys.exit(1)
