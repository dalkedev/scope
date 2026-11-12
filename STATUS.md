# Snurpo 1.5.0

Snurpo 1.5.0 adds **Copy as cURL** to the request inspector. Preview a captured
request, review its credentials, then explicitly copy a command for sh, bash or
zsh. Text/binary bodies and repeated headers are supported. Opening or copying
the preview sends no traffic; execution happens separately in your terminal.
See [the guide](docs/GETTING_STARTED.md#copy-a-request-to-your-terminal) for
normalization and size limits, and [release notes](docs/releases/1.5.0.md).

[Downloads](https://github.com/dalkedev/snurpo/releases/tag/v1.5.0) retain the
Snurpo branding introduced in 1.4.2. The existing native screenshot gallery records
that earlier release, with original capture provenance. Source tooling uses
Node 26.13.1 LTS; installed applications need no Node runtime. Earlier releases
keep their assets and checksums. See [migration notes](docs/MIGRATING_TO_SNURPO.md)
for unchanged local data, CA and MCP identifiers.

Local feature validation passed 87 tests on Node 26.13.1, including real sh/zsh
loopback execution with all 256 binary byte values. Native GUI clipboard checks
and fresh tagged package verification remain separate acceptance steps. This
feature does not add PowerShell or cmd.exe syntax or broaden device support.

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
