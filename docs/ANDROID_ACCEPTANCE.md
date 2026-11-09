# Scope-owned Android engine acceptance — 9 November 2026

**PASS: 75/75 controlled Android outcomes and 2/2 Stop/restoration gates**, using AOSP Android 16/API 36 ARM64, emulator 37.1.11, Frida 17.9.1 and real OkHttp 3.14.9. Each cell is an actual synthetic APK request with a Logcat receipt. This is one measured API/ABI combination.

The five phases are baseline, certificate-only, fresh baseline after Stop, certificate+unpinning, and fresh baseline after Stop. Each phase covers platform TLS, default OkHttp, and fixture-CA OkHttp against HTTP, valid Scope TLS, foreign CA, wrong hostname, and expired certificates.

The valid Scope request succeeds for platform TLS with certificate hooks and for both OkHttp modes with certificate+unpinning despite the configured wrong pin. Foreign CA, wrong hostname and expired leaves remain rejected with the original SSL exception classes. Fixture-CA OkHttp produces the specific `Certificate pinning failure!` marker in baseline, certificate-only and both restored phases. Both Stops produce matching successful owned-process termination receipts, exit 0 and observed app-process absence; fresh requests restore the original rejection behavior. Owned reverse mappings are removed.

The first real run found 2 failed expected outcomes: both ordinary OkHttp valid Scope controls remained rejected despite readiness. Bounded diagnostics identified a `TypeError` when the Frida PublicKey wrapper attempted inherited `getAlgorithm()`. An explicit `java.security.Key` cast fixes it; the final production source 75-cell replay passes.

A separate actual APK with genuine OkHttp 4.9.3/Kotlin 1.4.10 makes baseline HTTP 200. Certificate+unpinning correctly rejects its lazy `check$okhttp` configuration before readiness, exits 1, acknowledges successful owned-process cleanup, and leaves no app process. This establishes rejection, not Kotlin unpinning support.

Engine SHA-256: `0f9b35debb5527a2213d18db82aa571b470ef438b2cc3c958a72f968d0f1b498`.

The accompanying JSON contains all 75 allowlisted observations and the initial-failure/fix trace. Raw Logcat, local paths, process IDs, build receipts and ephemeral certificate keys remain private. The direct worker matrix is separate from the native desktop checks below. No system trust store, real account or external app traffic was used.


## Reproducible evidence and boundaries

The [allowlisted JSON receipt](verification/android-engine-2026-11-09.json) records the exact engine/worker hashes, initial two failures, all 75 final observations, both Stop gates and the genuine Kotlin compatibility control. The [owned fixture recipe](../examples/android-engine-fixture/README.md) describes the test inputs. The measured source is based on `f738ab4bb8f235f2a1055602f162b4b36aa52852` plus the explicit Java Key cast recorded in that receipt; it is not a claim that the unmodified base commit passed.

The 75 cells are five phases × three client configurations × five HTTP/certificate cases. A rejected foreign, wrong-host or expired leaf counts only when the expected SSL error is observed; reachability failures do not establish a TLS-negative result. The fixture-CA OkHttp mode provides the specific wrong-pin controls independently of system trust rejection.

The current native desktop checks below establish controlled proxy capture, Stop and reconnect. Crash/restart recovery with an active Android worker remains NOT RUN; persisted-PID recovery has host regression coverage. Physical devices, other API/ABI combinations, other Frida/OkHttp versions, native TLS, Cronet, QUIC, Flutter, custom/obfuscated pinners, child processes and long-session stability remain unverified or unsupported as described in [engine coverage](ANDROID_ENGINE.md).


## Native desktop capture and reconnect

A rebuilt, relocated macOS arm64 app completed two start–Stop cycles in the same
native application process against the owned API 36 emulator. One OkHttp request
and one platform request reached an independent loopback HTTP server through
Scope, appeared as HTTP 200 in Traffic, and exposed the expected JSON in the
native inspector. No direct fixture-port reverse mapping was present.

Stop ended each exact worker-spawned application process and removed the owned
proxy reverse mapping. The next Start action became available without restarting
Scope; the second capture succeeded. Reopening the active device panel retained
the actual Frida mode and locked configuration controls. After final Stop, the
app was idle, recovery state was absent and all three local diagnostic/candidate
exchanges remained in SQLite with a successful integrity check.

This found and fixed two lifecycle/UI regressions: successful Stop retained an
identity that made the interface offer Restore forever, and a reopened Frida
panel described System proxy instead. Failed cleanup still retains the identity
and recovery record for retry. Rust and UI regressions cover both boundaries.

The [allowlisted native UI receipt](verification/android-native-ui-2026-11-09.json)
binds those observations to source hashes. The relocated packaged desktop/MCP
smoke also passed real local capture, authentication, redaction, persistence,
same-directory restart and sharing isolation. An initial smoke launch through a
symlinked build path failed its bundle-resource assertion; testing the actual
relocated application bundle passed without changing the check.

These checks do not establish crash recovery while an Android worker is active,
physical-device compatibility or native TLS support.
