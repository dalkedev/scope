# Owned Android engine acceptance fixture

This original lab APK exercises a fresh platform `SSLContext.init` and real OkHttp **3.14.9**, with a deliberately wrong SHA-256 pin for `127.0.0.1`. It keeps the original hostname verifier, certificate checks and request errors. The explicit `okhttp-scope` control uses an isolated original manager containing only an optional public fixture CA; that same manager and its socket factory reach OkHttp, so the deliberate wrong pin is tested after successful chain cleaning. `platform` and default `okhttp` retain original system trust. Only `http[s]://127.0.0.1:PORT/` inputs are accepted; credentials, queries, fragments and redirects are forbidden. One action makes one GET, with one request in flight, four-second socket timeouts and an eight-second OkHttp call deadline. No response bodies are logged. HTTP reachability is separate from TLS/pin acceptance.

**Actual direct-worker acceptance: PASS, 75/75 outcomes and 2/2 Stop/restoration gates** on AOSP Android 16/API 36 ARM64, Frida 17.9.1 and real OkHttp 3.14.9. See [the exact target, source hashes, initial failure/fix and full numeric receipt](../../docs/ANDROID_ACCEPTANCE.md). The earlier software-emulator prerequisite failure remains historical; it is superseded for this measured target. Other API/ABI combinations and full native desktop lifecycle gates remain separate. The APK is test-only and is never packaged with Scope.

## Build with existing tools

Supply Android SDK platform `android-35`, build-tools `36.0.0` and JDK 17. The helper installs no SDK, JDK, Gradle or Maven. Its only network acquisition is these two exact Maven Central JARs, checked before any build tool executes:

| Artifact | SHA-256 |
| --- | --- |
| `com.squareup.okhttp3:okhttp:3.14.9` | `2570fab55515cbf881d7a4ceef49fc515490bc027057e666776a2832465aeca0` |
| `com.squareup.okio:okio:1.17.2` | `f80ce42d2ffac47ad4c47e1d6f980d604d247ceb1a886705cf4581ab0c9fe2b8` |

Downloads reject redirects, exceed neither 2 MiB per artifact nor a bounded network operation, and use a 45-second transfer deadline with a 15-second socket timeout. Cached JARs are rechecked. Generated output must be outside the repository. Use a dedicated empty directory. The helper creates an ownership marker and refuses nonempty unowned directories before changing permissions or replacing build subdirectories; marked directories support repeat builds.

```sh
python3 examples/android-engine-fixture/build.py \
  --sdk /path/to/existing/android-sdk --jdk /path/to/existing/jdk-17 \
  --output /tmp/scope-owned-apk
# Repeat without network after the locked JARs have been acquired:
python3 examples/android-engine-fixture/build.py \
  --sdk /path/to/existing/android-sdk --jdk /path/to/existing/jdk-17 \
  --output /tmp/scope-owned-apk --offline
python3 -m unittest discover -s examples/android-engine-fixture/tests -v
```

For the genuine pin-mismatch control, start the TLS fixture below first, then pass its public CA via `--scope-ca /path/from/server/scope-ca.pem`. Only a single actual X.509 CA certificate of at most 64 KiB is accepted. Parsing must return exactly one CA whose complete DER equals the decoded input; malformed DER, relabelled private keys, server leaves and trailing data are rejected. It is embedded as an APK asset, never installed in a system trust store. Rebuild after regenerating fixture CAs.

The helper runs `javac`, `d8`, `aapt`, `zipalign`, `keytool`, and `apksigner sign` followed by actual `apksigner verify --verbose --print-certs`. The output contains the signed APK and a JSON receipt with its SHA-256, exact libraries, tool versions and signature verification. Signing uses a disposable, seven-day lab key and the public password `android`; never reuse that key for production. Delete the dedicated output directory when finished. The APK contains the Apache license and dependency notices, including OkHttp's embedded Public Suffix List notice; see [THIRD_PARTY_NOTICES.txt](THIRD_PARTY_NOTICES.txt).

## Bounded direct TLS fixture

The server uses the installed `openssl` command and Python standard library. It generates unrelated ephemeral Scope/foreign CAs, an IP-valid Scope leaf, a foreign leaf, a Scope leaf for the wrong IP and an expired Scope leaf. Listeners bind **only 127.0.0.1**. Private keys are mode 0600 inside a temporary mode-0700 directory and are deleted at exit. A server instance lasts at most 600 seconds, has two-second connection timeouts and accepts a bounded number of connections per endpoint. It does not modify any trust store or device.

```sh
python3 examples/android-engine-fixture/serve.py --seconds 600 --max-connections 100
```

The single JSON startup receipt supplies the temporary public `scope_ca` path and URLs: Scope `18443`, foreign `18444`, wrong-host `18445`, expired `18446`, and HTTP `18080`. Keep that process running during the manual test. Restarting it creates new CAs; restart the worker with the new public CA. Host helper tests use real Python SSL verification for all four certificate cases. These are host certificate-fixture checks, not Android or Scope engine proof.

## Manual acceptance on an explicitly selected owned lab device

Use only a device and application you own and have selected for this test. The commands below are instructions, not an automatic device driver. They assume an already prepared compatible Frida server and the pinned Python Frida dependencies described in [Android setup](../../docs/ANDROID.md). Do not install a CA in the device system store. Do not use a real account or traffic.

Start the server, copy its public CA path, and build with that same certificate before installing:

```sh
CA='/temporary/path/from/server/receipt/scope-ca.pem'
python3 examples/android-engine-fixture/build.py \
  --sdk /path/to/existing/android-sdk --jdk /path/to/existing/jdk-17 \
  --output /tmp/scope-owned-apk --scope-ca "$CA" --offline
```

Set the explicit serial and artifact paths, inspect the selected device's existing reverse mappings, then add only unused test mappings. If a port already belongs to another mapping, choose unused fixture ports instead. Record which entries you added; remove only those entries at cleanup.

```sh
SERIAL='YOUR_OWNED_LAB_SERIAL'
APK='/tmp/scope-owned-apk/scope-engine-fixture.apk'
CA='/temporary/path/from/server/receipt/scope-ca.pem'
adb -s "$SERIAL" get-state
adb -s "$SERIAL" reverse --list
adb -s "$SERIAL" install -r "$APK"
adb -s "$SERIAL" reverse tcp:18080 tcp:18080
adb -s "$SERIAL" reverse tcp:18443 tcp:18443
adb -s "$SERIAL" reverse tcp:18444 tcp:18444
adb -s "$SERIAL" reverse tcp:18445 tcp:18445
adb -s "$SERIAL" reverse tcp:18446 tcp:18446
```

For the baseline, start the owned app directly. The `run` intent explicitly triggers one request; omit it to open the manual UI. Repeat with `client=okhttp`, `client=okhttp-scope` and each URL. The `okhttp-scope` mode requires the public-CA build above. Inspect JSON Logcat receipts; a `response` with status 200 is success, an `error` records the original exception class/message, `busy` requires waiting for the existing request to end. A connection refusal or timeout is an inconclusive reachability failure, never a TLS-negative PASS.

```sh
adb -s "$SERIAL" shell am start -n com.scope.enginefixture/.MainActivity \
  --ez run true --es client platform --es url http://127.0.0.1:18080/
adb -s "$SERIAL" shell am start -n com.scope.enginefixture/.MainActivity \
  --ez run true --es client platform --es url https://127.0.0.1:18443/
adb -s "$SERIAL" logcat -d -t 200 ScopeEngineFixture:I '*:S'
```

For certificate-only, run this worker in a separate interactive terminal, keeping stdin open. Wait for its exact `scope-java-v1` ready receipt before triggering requests through `am start`. Do not use `am start -S`, which would kill the worker-owned process. Type `stop` into the worker terminal and require its successful termination acknowledgment before proceeding to another phase.

```sh
python3 resources/device/frida_worker.py --serial "$SERIAL" \
  --package com.scope.enginefixture --ca "$CA" --scripts resources/frida \
  --profile certificate
# For the next phase, after stopping the preceding worker:
python3 resources/device/frida_worker.py --serial "$SERIAL" \
  --package com.scope.enginefixture --ca "$CA" --scripts resources/frida \
  --profile certificate --profile unpinning
```

These standalone commands deliberately select direct certificate/unpinning hooks. An omitted profile selects proxy as well and would route TLS through a different CA; that is a different experiment. This recipe does not exercise Rust/UI readiness, recovery or proxy capture.

| Phase / client | HTTP control | Valid Scope TLS | Foreign CA | Wrong host | Expired leaf | Actual result: API 36 ARM64, 2026-11-09 |
| --- | --- | --- | --- | --- | --- | --- |
| No hooks / platform | 200 | trust rejection | trust rejection | rejection | rejection | PASS |
| No hooks / default OkHttp | 200 | trust rejection | trust rejection | rejection | rejection | PASS |
| No hooks / `okhttp-scope` | 200 | specific pin mismatch | trust rejection | hostname rejection | expiry rejection | PASS |
| Certificate only / platform | 200 | 200 | trust rejection | hostname rejection | expiry rejection | PASS |
| Certificate only / default OkHttp | 200 | rejection at original cleaner or pin enforcement | trust rejection | hostname rejection | expiry rejection | PASS |
| Certificate only / `okhttp-scope` | 200 | specific pin mismatch | trust rejection | hostname rejection | expiry rejection | PASS |
| Certificate + unpinning / platform | 200 | 200 | trust rejection | hostname rejection | expiry rejection | PASS |
| Certificate + unpinning / both OkHttp modes | 200 | 200 despite configured wrong pin | trust rejection | hostname rejection | expiry rejection | PASS |
| Stop + fresh restart / platform and default OkHttp | 200 | original trust rejection restored | trust rejection | rejection | rejection | PASS |
| Stop + fresh restart / `okhttp-scope` | 200 | specific pin mismatch restored | trust rejection | hostname rejection | expiry rejection | PASS |

Record actual exception types/causes for every failure. For `okhttp-scope`, require `SSLPeerUnverifiedException` with `Certificate pinning failure!` for the valid-Scope no-hooks/certificate-only/Stop controls; a generic trust failure is insufficient. The default OkHttp chain cleaner retains its original manager and can reject Scope CA before the wrong pin is evaluated in certificate-only mode; that rejection alone does not prove a measured pin mismatch. Ordinary 3.14.9 pinner hooks retain the raw chain for isolated Scope validation. Kotlin lazy `check$okhttp` configurations are rejected before readiness because their cleaner can discard that chain. No chain-cleaner override is installed.

After Stop, require the worker's matching successful termination acknowledgment, check `adb -s "$SERIAL" shell pidof com.scope.enginefixture` reports absence, and launch the fresh baseline again. Missing acknowledgment, a surviving PID or unknown termination is a failed cleanup gate; do not kill arbitrary PIDs. Exercise Scope UI Stop/application restart separately to validate the full Rust/UI lifecycle and saved recovery; those current-source interactive gates require their own receipt and are not established by this worker matrix.

Proxy capture is another separate manual phase: run actual Scope on its selected listener and use only the synthetic HTTP URL with an explicitly selected proxy profile. Confirm capture in Scope, then Stop and confirm owned mapping/process cleanup. It supplies no HTTPS/pinning evidence. Do not combine that proxy experiment's CA with the direct fixture matrix.

Finally remove only your recorded test reverse mappings (`adb -s "$SERIAL" reverse --remove tcp:PORT` per owned entry), stop the temporary server and delete the disposable build directory. No wildcard mapping deletion, global trust-store change or automatic device selection is required.
