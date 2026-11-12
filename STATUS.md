# Snurpo 1.4.2

Snurpo 1.4.2 is the first release under the new name, with mascot desktop icons,
a macOS menu-bar icon, refreshed appearance and a native workflow gallery.
Source tooling uses Node 26.13.1 LTS; installed applications need no Node runtime.
The redundant `local` label beside the logo is removed. Packages and platform
notes are available through [Releases](https://github.com/dalkedev/snurpo/releases).
Earlier Scope releases keep their original assets and checksums.
See [migration notes](docs/MIGRATING_TO_SNURPO.md) for the unchanged data directory,
CA keys, archive formats and MCP compatibility identifiers.

The reviewed 1.4.1 runtime sources passed 75 direct-worker Android outcomes and two
native macOS Android Start/Stop capture cycles on AOSP API 36 ARM64, Frida
17.9.1 and OkHttp 3.14.9. Those are recorded historical checks, not a claim of new
Snurpo package or device verification. New artifacts need their own exact-commit
checks, maintainer review and asset verification.

Snurpo includes persistent HTTP(S) capture, request/response rules, editing and
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
The HAR Action initially targets Linux runners with caller-supplied Node 26.13.1+
and Bash; its results do not validate desktop or device behavior. Derived report
labels and traffic patterns need review before sharing. Existing upstream
dependency findings remain in [DEPENDENCIES.md](docs/DEPENDENCIES.md).

Development: `npm ci` then `npm run desktop`. Packaging: `npm run package`.
[Contributing](CONTRIBUTING.md) · [Roadmap](ROADMAP.md) ·
[Release process](docs/RELEASING.md)
