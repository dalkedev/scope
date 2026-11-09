# Scope-owned Android interception engine

## Intent and scope

Replace HTTP Toolkit-origin interception/unpinning scripts with original Scope code using documented Android, Java and Frida APIs. Retain Frida and its independently attributed Java bridge as third-party instrumentation infrastructure. This is a source migration, not a new TLS implementation or a license change. Retain public source history and accurate third-party notices.

The existing Scope Rust HTTP(S) proxy, CA, archive and upstream certificate verification stay unchanged. New runtime code must not be copied or mechanically translated from the removed upstream hooks. No claim of legally certified clean-room development is made.

## Approach

A small per-process Java engine is the default and only built-in hook backend. Keeping the imported hook backend indefinitely would leave the ownership problem unresolved. Reimplementing Frida or every native networking stack would expand this work beyond a reviewable change. Scope-owned Java hooks on Frida provide a bounded, testable boundary.

Supported profiles are `proxy`, `certificate`, `unpinning`; an empty selection means all three. `unpinning` requires `certificate`. Unsupported `root`, `system-ca` or unknown profiles fail before any device probes or state mutation. No native TLS, QUIC, Flutter, Cronet, custom obfuscated pinners, child process or root-hiding coverage is claimed.

`proxy` directs supported Java HTTP clients to the ADB-reversed loopback Scope listener. `certificate` adds trust only for chains validated against the supplied Scope CA while preserving original validation for other chains. No trust-all manager, hostname-verifier bypass, certificate-expiry bypass or global system trust-store mutation is allowed. `unpinning` may skip supported OkHttp pin checks only after the peer chain validates against the configured Scope CA; non-Scope chains must execute original pin checks and preserve their result/exception.

## Runtime and lifecycle contract

The Python worker validates finite profile, PEM and port inputs before device work. Source construction uses JSON serialization for runtime data, not source-text replacement. The engine reports per-capability installation results. Readiness requires successful installation of every requested profile, not merely entry into Java.perform. Unsupported classes/overloads or hook errors produce actionable bounded diagnostics and no success claim. Installation failure rolls back installed method implementations.

Process-created proxy/trust objects can outlive method unhooking. Therefore Stop terminates the exact process spawned by this worker, then unloads/detaches; it does not promise that unloading alone restores the running app. It must not kill an unrelated process. Cleanup errors remain visible, and no clean-stop receipt is emitted after failed cleanup. The worker emits a spawned-PID event before attach and an explicit termination acknowledgment only after cleanup. Rust preserves pending target-process cleanup in recovery records; it must not convert a killed/failed worker into a successful Stop. A saved owned PID can be checked for absence before clearing an interrupted session; ambiguous state is an actionable error, never a reason to kill an arbitrary or reused PID. Existing Rust recovery of owned ADB/server state and legacy CA removal remains available after restart.

## Compatibility changes

Remove the imported system-CA injection template and automatic rooted proxy-mode trust-store injection. System proxy mode remains device-wide; HTTPS requires manually configured application trust. Do not remove legacy CA cleanup for recovery records created by earlier versions. Remove root-hiding selection from the UI; stale API/CLI selections are rejected before mutations. Explain these reductions prominently instead of claiming full parity with the imported scripts.

## Validation and delivery

Use executable JavaScript policy/adapter tests, worker builder/lifecycle tests and existing Rust/UI/device/release suites. Include controls proving unrelated or invalid chains still fail, original pin exceptions survive, unsupported capabilities never become ready, partial startup rolls back, and Stop ends only the owned PID. Simulated Frida/Java adapter tests are explicitly not Android execution evidence.

Real Android/Frida/OkHttp TLS, app-stop restoration and API/ABI compatibility are separate acceptance gates. Record exact PASS / FAIL / NOT RUN without inventing coverage. An independently reviewed PR may remain draft if these device gates cannot be executed. No merge, release, tags, engine/service changes or live external account actions.

## Primary API references

- https://frida.re/docs/javascript-api/#java
- https://developer.android.com/privacy-and-security/security-config
- https://github.com/square/okhttp

## Pinned OkHttp compatibility clarification

Primary-source inspection of OkHttp4.12 shows that the client's original trust manager can reject Scope CA during lazy Handshake chain cleaning; Handshake then exposes an empty peer chain to `check$okhttp`. The pinner hook alone cannot safely recover the lost raw chain. Reject this recognized lazy configuration before claiming unpinning readiness. Retain only the ordinary Java overload path targeted by pinned OkHttp3.14.9 until a coherent cleaner/trust-manager integration has actual runtime validation. This deliberately reduces compatibility instead of treating an empty chain as trusted.

Ship an original, buildable lab APK and bounded loopback TLS fixtures so missing Android acceptance can be reproduced when an owned device is available. Real pinned OkHttp3.14.9 and Okio1.17.2 are test-only dependencies, checksum locked; generated APKs and test keys never enter production resources or source control. APK compilation/signature verification is a separate gate from executing Android TLS/unpinning/Stop tests.
