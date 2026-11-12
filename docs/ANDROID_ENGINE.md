# Snurpo-owned Android Java engine

The experimental `scope-java-v1` engine uses documented Frida/Java APIs and the unchanged third-party Frida Java bridge. It replaces the imported HTTP Toolkit runtime hooks. The project remains AGPL-3.0-or-later; [notices and bridge provenance](../THIRD_PARTY_NOTICES.md) remain applicable. No legally certified clean-room claim is made.

## Coverage

| Profile | Supported seam | Limits |
| --- | --- | --- |
| `proxy` | Java URL openConnection overloads and available OkHttp3 Builder.build | Existing clients, other Java clients and native connections are not covered. |
| `certificate` | Subsequent SSLContext.init, original basic/extended X509 managers | Existing/default contexts are not covered. Original socket/SSLEngine delegation and hostname verification remain active. |
| `unpinning` | Ordinary Java OkHttp3 CertificatePinner List and Certificate[] signatures; acceptance target 3.14.9 | Requires certificate profile; missing/unknown pinner capabilities fail startup. Custom/obfuscated/native pinners are unsupported. |

A separate empty KeyStore contains only the supplied Snurpo CA, and a real Java TrustManagerFactory performs chain validation. CA constraints, certificate validity, server-leaf checks and original Java exception types bound acceptance. A valid Snurpo chain can override an original certificate/pin rejection only at the supported seams; unrelated failures and non-Snurpo chain decisions remain unchanged. No trust-all manager, hostname bypass or expiry bypass is installed. Native TLS, Cronet, QUIC, Flutter, child processes and root hiding are unsupported.

System proxy mode remains device-wide and requires manually configured application trust for HTTPS. Neither mode injects a system CA. Legacy recovery records retain removal of their recorded owned CA.

## Lifecycle

The worker serializes runtime data as JSON, validates profiles and exactly one complete public X.509 CA before importing Frida or contacting a device and requires every requested capability before ready. Rust persists attempted-spawn uncertainty before launching the worker and the exact spawned PID before accepting ready. Terminal errors/detach/EOF cannot revive readiness; a prior worker generation cannot write a newer session's state.

Stop ends the exact process spawned by the active worker, then unloads/detaches. This is necessary because created proxy/TLS objects can outlive unhooking. Rust allows 12 seconds for the worker's bounded nine-second cleanup plus scheduling overhead, then drains stdout. A matching successful cleanup/termination receipt and successful worker exit are required before clearing pending cleanup. A receipt proving Stop occurred before any attempted spawn is also accepted without claiming termination.

A failed/timed-out worker or missing acknowledgment retains recovery. After restart, only observed absence of a saved PID in a readable root procfs view can establish that it ended. Present/inaccessible PID state and unknown attempted-spawn completion require manual reconciliation; saved target PIDs are never killed. Existing mapping ownership checks preserve external proxy/reverse/forward changes.

## Bounded host diagnostics

Worker stdout and stderr are incrementally framed at at most 65,536 bytes per line, excluding LF and its optional preceding CR. The reader retains a fixed 65,537-byte frame buffer plus an 8,192-byte input buffer per pipe. It discards oversized remainders through the next LF without growing a buffer. Final non-newline receipts and valid fragmented UTF-8 remain supported.

Oversized, invalid UTF-8 or failed stdout reads are terminal protocol failures; later readiness/termination messages cannot clear that failure or establish successful cleanup. JSON receipts are never truncated and parsed. Stderr is diagnostic only: oversized/invalid lines produce bounded diagnostics and draining resumes for subsequent lines, with the existing generation isolation.

Retained log entries contain at most 4,096 UTF-8 bytes including any truncation marker, with at most 200 entries (819,200 stored text bytes). Entries are copied into bounded fresh strings so a shortened entry cannot retain its former large allocation. These are diagnostic framing/storage limits, not a bound on all application memory or a measured Android throughput improvement. Normal Python worker messages remain compatible with the framing limit even when JSON escaping expands 4,000 Unicode characters.

## Evidence and acceptance

| Gate | Current evidence |
| --- | --- |
| Java policy and simulated Frida adapter | Executable Node suite covers original validation/pinning controls, capability rollback and supported API wiring. |
| Worker lifecycle and source construction | Executable Python suite covers profiles, JSON config, readiness, cancellation and owned-process cleanup. |
| Rust application lifecycle | Local child pipes and fixture ADB cover typed receipts, output drain, recovery, profile preflight and legacy cleanup. |
| UI | Selection/dependency regressions and production frontend build are local checks. |
| Real Android/Frida/OkHttp TLS and hostname controls | **PASS:** 75/75 actual synthetic outcomes on AOSP Android 16/API 36 ARM64, Frida 17.9.1, OkHttp 3.14.9; [exact evidence](ANDROID_ACCEPTANCE.md). |
| Actual worker app termination/restoration | **PASS:** 2/2 matching owned-process termination/exit/absence gates and fresh baseline restoration on that same API/ABI. Other targets remain unverified. |
| Native desktop proxy capture, Stop and reconnect | **PASS:** two cycles in one relocated macOS arm64 app with the owned API 36 fixture; [native evidence](ANDROID_ACCEPTANCE.md#native-desktop-capture-and-reconnect). Active-worker crash recovery remains NOT RUN. |

Simulated tests do not prove Android execution, cryptography or compatibility. The separately recorded [9 November actual acceptance](ANDROID_ACCEPTANCE.md) covers one emulator API/ABI, the direct production worker and separately measured native capture/Stop/reconnect. Earlier emulator observations used the previous backend and remain historical. No native UI/device evidence is inferred from a headless build, and no general Android compatibility claim follows from the measured target.

## Owned acceptance fixture

The [original lab APK and manual matrix](../examples/android-engine-fixture/README.md) target real OkHttp 3.14.9 with a deliberately mismatching pin, fresh platform contexts and ephemeral loopback-only valid/foreign/wrong-host/expired TLS endpoints. Build/signature and host certificate-fixture checks are separate from Android execution. The [recorded actual matrix](ANDROID_ACCEPTANCE.md) passes all 75 controlled outcomes and both worker Stop/restoration gates on the specified target. Native desktop lifecycle gates remain separate.

Recognized Kotlin lazy `check$okhttp` configurations are rejected transactionally before unpinning readiness, including classes that also expose ordinary public overloads. Their original-manager chain cleaner can discard the raw chain before the pinner sees it. No lazy support or chain-cleaner override is claimed. Ordinary Java overloads preserve the raw chain and typed original/non-Snurpo error controls.
