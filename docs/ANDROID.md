# Android interception

Scope's Android adapter uses its own localhost proxy, public CA and experimental Java interception engine on Frida. The retained Java bridge has separate third-party provenance. [Engine coverage and evidence](ANDROID_ENGINE.md).

## What is implemented

- Real ADB device enumeration, including offline/unauthorized states, model and a read-only root check.
- Installed packages and running state. If an existing Frida server is available, app display labels come from Frida; otherwise the exact package ID is shown as the name.
- **System proxy:** Android's global HTTP proxy and a matching ADB reverse mapping. The selected application is restarted. This mode is **device-wide**, so traffic from other apps may appear. HTTPS requires manually configured application trust; Scope does not install a system CA.
- **Frida:** a separate Python worker spawns and attaches to the selected application. Original Scope Java hooks configure supported proxy seams, add trust for chains validated against Scope's CA, and conditionally bypass supported OkHttp3 pins for those chains. Hostname verification remains active.
- **Frida Stop ends the exact instrumented app process**, unloads/detaches and restores recorded owned mappings/server state. System proxy Stop restores recorded proxy/reverse state. Older recovery records retain cleanup of their Scope-owned CA.
- A saved recovery file allows Stop to retry after a disconnected device or interrupted session. Starting another session is refused until that cleanup succeeds.
- The 1.0 **Diagnostics** workspace and `device_diagnostics` MCP tool provide read-only authorization/root/ABI/runtime/mapping/recovery preflight with actionable fixes. See [diagnostic boundaries and troubleshooting](DIAGNOSTICS.md).

No device proxy, certificate or Frida server changes occur during device/app listing. Start interception is the action that changes the selected device. A root check can cause the device's superuser manager to show its normal authorization prompt.

Diagnostics never starts an emulator, app or server, uploads a binary, downloads a runtime, changes trust/proxy/mappings, or performs cleanup. Local server versions are not guessed from filenames; the report distinguishes unknown values from verified binary versions. A ready result is basic preflight, not proof of app/OS compatibility.

## ADB

Install Android SDK platform-tools and enable USB debugging on the device.

Discovery order:

1. `SCOPE_ADB` executable override.
2. `ANDROID_HOME/platform-tools`, then `ANDROID_SDK_ROOT/platform-tools`.
3. Common macOS, Linux and Windows Android SDK locations.
4. `adb` / `adb.exe` on PATH.

Authorize the computer on the device and use Refresh. A rooted emulator is useful for a local lab. Scope does not create, root or rewrite emulator images.

A non-rooted device can use system proxy mode for HTTP. HTTPS then depends on manually configured app trust. Modern Android apps commonly ignore user-installed CAs, and pinning is not bypassed by proxy mode.

## Frida runtime

The Rust adapter and resources are designed for multiple hosts; the worker is isolated from the desktop process. **Public packages, including 1.0.0, do not include a standalone Frida worker or Android server.** Install the optional runtime locally before using Frida mode. The integration source and JavaScript profiles are included; ordinary proxy capture does not require Python or Frida. Windows/Linux have CI build/headless-protocol coverage; interactive desktop and device behavior on those hosts remain unverified. See [verification boundaries](VERIFICATION.md).

A task-local macOS arm64 bundle with a standalone worker/server was exercised during development. That verifies the optional integration, not the contents of the public download. Worker and device server must use the **same Frida version**.

### Recommended user-side setup

Install Python **3.9+** with `venv` support. Find the Android ABI with `adb -s YOUR_SERIAL shell getprop ro.product.cpu.abi`, then run the included helper. For the public macOS app installed in Applications:

```sh
python3 /Applications/Scope.app/Contents/Resources/resources/device/install-runtime.py --abi arm64-v8a
```

From a source checkout:

```sh
python3 resources/device/install-runtime.py --abi arm64-v8a
```

Replace `arm64-v8a` with the ABI reported by the device (`armeabi-v7a`, `x86` and `x86_64` are also accepted). On Windows, use `py -3` instead of `python3`. Scope also reports a command with the actual resource/runtime paths when no runtime is found.

The helper creates a private virtualenv under Scope's `device/managed-frida` runtime directory, installs pinned Frida 17.9.1 from PyPI and downloads the matching official Android server with a pinned SHA-256 check. It does not use system-wide pip or change the Android device. After it succeeds, retry interception; no environment variables are needed. Server push/start happens only when you start interception. `--runtime-dir` supports a custom Scope device-runtime directory.

Worker precedence is an explicit `SCOPE_FRIDA_WORKER` executable, an explicit `SCOPE_PYTHON` interpreter, an optional privately packaged worker, the managed virtualenv, then `python3`/`python` on PATH. Server precedence is explicit local/device overrides, an optional privately packaged ABI match, the managed ABI match, then an existing default-port USB Frida server. Packaged resources are resolved through `SCOPE_RESOURCES`; source builds also find the repository's resources directory.

### Manual environment overrides

For a separately managed Python environment, from the repository root:

```sh
python3 -m venv .venv-frida
.venv-frida/bin/python -m pip install -r resources/device/requirements-frida.txt
export SCOPE_PYTHON="$PWD/.venv-frida/bin/python"
```

On Windows, use `.venv-frida\Scripts\python.exe` and set `SCOPE_PYTHON` to its absolute path before launching Scope.

With no server override, Scope checks the selected device's ABI and uses a matching optional packaged or managed `frida-server-17.9.1-android-*` resource when available. No native server is included in public packages; the recommended helper installs it locally. If none is found, Scope connects to an existing default-port USB Frida server and does not stop a server it did not start.

To have Scope launch a dedicated server, download a matching official binary locally first. This helper **only downloads** and does not alter the device:

```sh
# Choose --abi from: adb -s YOUR_SERIAL shell getprop ro.product.cpu.abi
.venv-frida/bin/python resources/device/prepare_frida.py \
  --abi arm64-v8a --output .runtime/frida-server
export SCOPE_FRIDA_SERVER_LOCAL="$PWD/.runtime/frida-server"
```

Start interception then copies that binary to `/data/local/tmp/scope-frida-server`, checks its version and starts it on device loopback port `24073`. A dynamically allocated local ADB forward connects the Python worker. Only the recorded server PID is stopped.

Before upload, Start checks the local ELF architecture against the selected Android ABI. It refuses unsupported/unknown ABIs, exact worker/server version mismatches and an occupied dedicated listener port. A failed server startup reports its local Android log. A worker exit/detach clears the running indicator and preserves recovery state for Stop.

Alternatively, `SCOPE_FRIDA_SERVER=/absolute/device/path/frida-server` selects an existing executable on the Android device and starts a dedicated instance. The path is validated before entering a remote shell command.

### Experimental local-only standalone packaging

Public packaging excludes optional native Frida binaries from `resources/bin`. To experiment locally with a standalone worker for the current desktop OS/architecture, use a redistributable Python runtime and a dedicated virtualenv. This is not a public redistribution recipe:

```sh
/path/to/redistributable/python3 -m venv .venv-frida-bundle
.venv-frida-bundle/bin/python -m pip install -r resources/device/requirements-bundle-frida.txt
.venv-frida-bundle/bin/python scripts/bundle-frida.py --local-only \
  --work-dir .runtime/frida-package \
  --server /absolute/path/to/matching/android/frida-server --server-arch arm64
```

The helper runs PyInstaller, checks the resulting worker's version and copies the optional device server. It does not install dependencies or touch a device. The selected server is still pushed/started only when interception starts. `SCOPE_FRIDA_WORKER` can point to the result when packaging resources separately.

The locally tested optional worker used CPython 3.12.13 from python-build-standalone, Frida 17.9.1 and PyInstaller 6.22.3. It was checked with an empty environment containing only `/usr/bin:/bin` on PATH and without a system Python. It is macOS arm64 only and is not part of the public release. Windows, Linux and Intel macOS need their own worker builds. Before redistributing any optional native runtime, review the artifact-specific dependency notice requirements and corresponding-source references in [THIRD_PARTY_NOTICES.md](../THIRD_PARTY_NOTICES.md).

### Hook profiles

| ID | Supported behavior |
| --- | --- |
| `proxy` | Subsequent Java URL HTTP(S) connections and available OkHttp3 builders use the Scope listener. |
| `certificate` | Subsequent SSLContext initialization wraps original X509 trust managers; separate Java PKIX validation admits only valid chains to the supplied Scope CA. |
| `unpinning` | Ordinary Java OkHttp3 CertificatePinner List and Certificate[] overloads may accept a Scope-validated chain after an original rejection; acceptance target 3.14.9. Requires `certificate`. Lazy Kotlin `check$okhttp` configurations fail before readiness. |

An omitted/empty API or CLI profile array selects `proxy`, `certificate`, `unpinning` for compatibility. Explicit selections remain exact. The UI disables Start for an empty selection; deselecting certificate also deselects/disables unpinning. `root`, `system-ca` and unknown profiles fail before device probes. No root hiding is provided.

Readiness requires the exact engine, owned PID, requested profiles and installed capability receipts. An unavailable supported OkHttp pinner causes default-all startup to fail; choose explicit proxy/certificate profiles for the supported available seams. A callback alone cannot establish readiness or network coverage.

### Compatibility boundaries

[Actual acceptance on 9 November 2026](ANDROID_ACCEPTANCE.md) passed 75/75 direct-worker outcomes and 2/2 Stop/restoration gates on AOSP Android 16/API 36 ARM64 with Frida 17.9.1 and OkHttp 3.14.9. A genuine OkHttp 4.9.3/Kotlin 1.4.10 APK was rejected for its unsupported lazy pinner before readiness and cleaned up successfully; this is rejection coverage, not Kotlin unpinning support. Other API/ABI combinations and physical devices remain unverified. Native desktop Stop/restart and recovery are separate gates.

The source repository recorded Android 16 ART/GC crashes with Frida 17.9.1. This remains experimental; app/OS combinations must be tested. Hook crashes are contained in the external worker/app process and do not erase saved sessions.

The bundled profiles target **Java** networking. Native TLS implementations, QUIC/HTTP3, child processes, anti-instrumentation and app-specific pinning may need additional work. Do not infer complete capture from an empty request list.

The Frida version pin is the existing integration baseline, not a claim that it supports every current Android release. Python/server upgrades should be tested together with the Java bridge bundle.

## Cleanup and recovery

Scope writes `device-recovery.json` under its runtime directory before changing the device. It contains the selected serial/package, previous proxy/reverse state, legacy temporary CA hash, owned forwarding/server identifiers and pending spawned-process cleanup/PID. The spawned PID is persisted before readiness; an attempted spawn without a returned PID remains explicitly uncertain.

Press Stop while the device is connected. If cleanup fails, the file remains and the log explains that reconnection is needed. Reconnect the **same device**, then press Stop again. Do not delete the recovery file as a substitute for restoring device settings.

Stop verifies that current proxy and ADB mappings still match Scope's recorded values before restoring them. External changes are preserved and logged. Invalid saved fields are rejected before cleanup commands; failed inspection leaves recovery available for a retry.

New sessions never inject system CAs. For older recovery records only, Stop still removes the recorded Scope-owned certificate from shared stores and the selected app. Existing temporary mount overlays disappear on reboot; Scope does not unmount shared trust stores underneath other apps.

Frida Stop terminates the owned app process because created client/TLS objects can outlive method unhooking. The worker has 12 seconds for bounded cleanup, followed by stdout drain. A successful worker exit alone cannot prove termination: Rust requires a matching successful acknowledgment. Failed/timed-out workers and missing acknowledgments retain recovery. After a restart, only observed absence of the recorded PID in a readable root procfs view allows automatic reconciliation. A present or inaccessible PID remains an error and is never killed from saved state. Unknown attempted-spawn completion requires manual process/device reconciliation; Scope cannot safely discover it by package name. Preserve the recovery file until that reconciliation and owned setting/mapping restoration are complete. A server binary downloaded/copied for Scope remains for the next session; only its owned process is stopped.

## Local Android demo

`examples/android-demo` contains a small Java Activity using `HttpURLConnection`. It talks only to Scope's local demo API at `http://127.0.0.1:17891`.

The build script uses an installed SDK and JDK directly; it requires no Gradle or Maven download:

```sh
bash examples/android-demo/build.sh
adb -s YOUR_SERIAL install -r examples/android-demo/build/scope-demo.apk
adb -s YOUR_SERIAL reverse tcp:17891 tcp:17891
```

The default paths target macOS Android Studio. Override `ANDROID_HOME`, `JAVA_HOME`, `SCOPE_ANDROID_BUILD_TOOLS` and `SCOPE_ANDROID_JAR` for another SDK layout. The demo app itself works on Android API 26+; the offline shell build is currently verified on macOS.

Start Scope's demo server, choose `com.scope.demo` in the application picker and start interception. The app buttons send real GET/POST requests, show the received HTTP status/body and allow enough read timeout for an interactive breakpoint.

The package uses cleartext for the local fixture server. It is a test target, not an example of production TLS configuration. Its login request contains only an explicit local-demo credential.

For low-level adapter diagnosis:

```sh
cargo run -p scope-device --example device -- devices
cargo run -p scope-device --example device -- diagnostics YOUR_SERIAL
cargo run -p scope-device --example device -- apps YOUR_SERIAL
cargo run -p scope-device --example device -- start YOUR_SERIAL com.scope.demo proxy 17890 /absolute/path/scope-ca.pem
# Enter stops and restores this smoke-test session.
cargo run -p scope-device --example device -- stop
```

Set `SCOPE_DEVICE_RUNTIME` to an absolute directory to keep the smoke-test recovery state at a chosen location. The smoke test is separate from the desktop runtime.

## Source and licenses

The runtime uses original `scope-engine.js` plus the unchanged Frida Java bridge. Six imported HTTP Toolkit hook/config files and the CA installation template were removed; source history and retained license/provenance notices remain. Runtime data is JSON-serialized. This migration changes neither the project AGPL license nor the existing Rust proxy/CA/upstream verification. See [THIRD_PARTY_NOTICES.md](../THIRD_PARTY_NOTICES.md).

## Recorded local smoke test

**Historical backend receipt:** the following 2026-11-02 results used the removed hook backend and CA-injection path. They do not validate the current Scope-owned engine or its process-ending Stop contract. Current direct-worker TLS/pinning/hostname and Stop/restoration acceptance is recorded separately for one API/ABI in [the 9 November receipt](ANDROID_ACCEPTANCE.md).

On 2026-11-02, the macOS build was checked against an existing rooted Android 16 arm64 emulator using only the included `com.scope.demo` application:

- The APK built offline, signed, installed and launched successfully.
- Rust ADB discovery reported the actual device and verified root.
- Proxy mode injected Scope's own CA and verified its contents inside the app's mount namespace. Four tapped GET/POST requests reached the Scope archive with the real `Dalvik` user agent and `X-Scope-Source: android-demo` header.
- Stop restored the previous `:0` global proxy, removed only the proxy ADB reverse mapping, preserved the separate demo-server mapping and removed Scope's CA from system/APEX stores. The Rust subject hash matched `openssl x509 -subject_hash_old`.
- Frida 17.9.1 Python and server then instrumented the selected process. With the global Android proxy still `:0`, three additional tapped requests reached the same archive through the app's Java proxy hooks.
- Frida Stop detached the worker, removed its ADB forward/reverse mappings and stopped the owned server process. The demo app remained running.
- A second Frida smoke used the standalone worker and automatically selected packaged Android arm64 server, with all Python/Frida override variables unset. Two further real APK requests reached the native app's archive; Stop again restored forwarding and left no owned server process.

This is a short functional smoke test of one app/OS/device combination. It does not establish stability under long sessions or support for other network stacks. Windows/Linux packaging and physical-device behavior were not tested in this run.

The 1.0 diagnostics/lifecycle updates add fixture-based authorization, ABI/version, recovery ownership and worker-exit tests. No emulator was booted and no new physical-device/interception smoke test was performed for those updates.
