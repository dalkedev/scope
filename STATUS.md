# Scope 1.4.0 release candidate

Application version: `1.4.0`. This source branch combines the Scope-owned
experimental Android Java engine with an opt-in archive measurement harness.
The reviewed runtime sources passed 75 direct-worker Android outcomes and two
native macOS Android Start/Stop capture cycles on AOSP API 36 ARM64, Frida
17.9.1 and OkHttp 3.14.9. The final versioned package, exact-head hosted checks,
maintainer review and draft asset verification are separate release gates.
A version in source does not establish a published release; see
[Releases](https://github.com/dalkedev/scope/releases) for downloads and exact
platform manifests/checksums.

Scope includes persistent HTTP(S) capture, request/response rules, editing and
replay, structured traffic filters, HAR/native archives, observed-traffic
analysis, Android proxy/Frida integration and twelve scoped MCP tools. The
separate HAR CLI and composite Action compare local captures; authored contracts
provide explicit status/JSON requirements.

[Recorded verification](docs/VERIFICATION.md) separates local candidate checks,
exact-commit hosted CI and historical macOS/Android interaction. Fresh combined
CI, draft asset verification and interactive desktop/device checks are separate
release steps. Windows/Linux interactive behavior remains experimental; Intel
macOS is unverified. The macOS package is ad-hoc signed, without Apple notarization.

Public packages exclude optional native Frida worker/server binaries. Users may
install the optional runtime separately; source integration, setup helpers and
supported Java hook profiles remain included. System proxy mode is device-wide
and does not install a CA. Native TLS, custom pinners and Kotlin lazy pinners are
unsupported; active-worker crash/restart recovery, physical devices and other
Android API/ABI combinations remain unverified. See
[Android acceptance](docs/ANDROID_ACCEPTANCE.md).

Archive harness timings retain their recorded historical source provenance. They
do not establish a production writer change or archive-throughput improvement.
[Archive measurements](docs/ARCHIVE_RESPONSIVENESS.md). [Android setup](docs/ANDROID.md)

Bodies remain buffered up to 16 MiB each. WebSocket frames, trailers, HTTP/3,
large-session streaming and long-duration stability are outside verified coverage.
The HAR Action initially targets Linux runners with caller-supplied Node 22.22.3+
and Bash; its results do not validate desktop or device behavior. Derived report
labels and traffic patterns need review before sharing. Existing upstream
dependency findings remain in [DEPENDENCIES.md](docs/DEPENDENCIES.md).

Development: `npm ci` then `npm run desktop`. Packaging: `npm run package`.
[Contributing](CONTRIBUTING.md) · [Roadmap](ROADMAP.md) ·
[Release process](docs/RELEASING.md)
