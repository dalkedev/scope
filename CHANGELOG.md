# Changelog

User-visible changes are recorded here. Back up important captures before upgrading.

## Unreleased

## 1.4.0 candidate — 2026-11-09

- Replace imported Android runtime hooks with the experimental Scope-owned Java
  URL/SSLContext/ordinary OkHttp engine. Validate only the configured Scope CA
  through a real isolated trust manager; retain original TLS/hostname failures.
- End the exact worker-owned app process on Frida Stop, retain uncertain cleanup
  recovery and refuse to kill saved PIDs after restart. Bound worker framing and
  retained diagnostics; show the actual active mode and allow repeated Start/Stop.
- Remove automatic system CA injection and root-hiding selection. System proxy
  capture remains device-wide; native TLS, custom pinners and lazy Kotlin OkHttp
  unpinning remain unsupported. Target-specific Android evidence is recorded
  separately from untested API/ABI and active-worker crash/restart behavior.
- Add an opt-in archive measurement harness, process controls, numeric compatibility
  receipts and a measurement workflow. Historical timings retain their source
  provenance; no production archive-writer or throughput improvement is claimed.

## 1.2.2 — 2026-11-06

- Backport the upstream GLib `VariantStrIter` soundness fix to the compatible
  Linux GTK 0.18 dependency family. Retain the real 0.18.5 version, original
  license, pristine source archive and exact upstream patch.
- Verify every vendored file and Cargo's locked selection before checks and
  release builds. Optimized Linux regressions exercise the affected iterators;
  verification preserves exact bytes with Windows Git settings and decodes
  Cargo metadata explicitly as UTF-8.
- Preserve upstream and modified-source attribution in redistributed notices.
  Record individual CodeQL flow reviews and concrete ownership/conversion tests
  while retaining code scanning. Interactive GTK/dialog and device acceptance
  remains open in #11.

## 1.2.1 — 2026-11-06

- Move macOS CI and release packaging to the supported macOS 15 arm64 runner.
  This changes the build host and leaves application deployment-target settings unchanged.
- Refresh release instructions and the current README to distinguish published
  1.2.0 builds from the 1.2.1 maintenance update.

## 1.2.0 — 2026-11-05

- Reusable offline HAR comparison composite Action with JSON, HTML and JUnit
  outputs, minimized labels by default and explicit clean/regression/invalid
  results. Deferred gating lets callers inspect reports before enforcing the
  result; uploads remain an explicit caller choice.
- New Scope logo, repository banner and desktop icon assets. Existing real
  application screenshots and desktop interaction colors are retained.
- Updated pinned GitHub Actions, Lucide icons and TypeScript. Redistributed
  dependency notices now match the committed locks and include Lucide's
  Feather-derived MIT notice.

## 1.1.0 — 2026-11-05

- Structured desktop traffic search with combined method, host, status, body,
  lifecycle and edited predicates. Literal search remains available, and syntax
  help includes clickable examples. [Query syntax and limits](docs/SEARCH.md).
- Offline HAR comparison CLI with exact operation selection, scoped JSON ignores,
  explicit status/field/type contracts and deterministic JSON, escaped HTML and
  JUnit reports. [Usage and privacy boundaries](tools/har-compare/README.md).
- Lower native session-import peak memory by consuming the owned exchange tree
  instead of cloning it. Bounded Linux debug-profile measurements and unchanged
  atomic validation are documented in [the results](docs/NATIVE_IMPORT_MEMORY.md).
- Active isolated HAR CI with hostile-report and sandboxed Chromium/CSP checks;
  cross-platform desktop tests cover the combined changes.
- Current maintainer CODEOWNERS and a version-matched bot-authored release
  proposal flow. Required approvals and branch protections remain enabled.

## 1.0.0 — 2026-11-02

- Persistent ordered request/response rewrites, mock responses and blocking, with validated atomic updates and per-request rule snapshots.
- Applied-rule evidence, upstream/mock/blocked provenance and preserved original messages, including HAR/native roundtrips.
- Replay composer for method, destination, full repeated headers and body. Current rules apply; manual breakpoints are bypassed.
- Session rename, notes, normalized tags, pins and deletion with active capture protection; metadata survives native archives.
- Explicit in-flight/completed state and interrupted-capture recovery. Analysis excludes unfinished transfers from errors and latency.
- Read-only Android/Frida preflight with root/authorization/ABI/version/mapping/recovery evidence and actionable failure messages.
- Observed-only OpenAPI 3.1 inference with bounded schemas, no example values and explicit provenance/limits.
- Twelve MCP tools, separate global-rule and diagnostics permissions, edited replay and default-deny writes. Access resets on restart.
- Optional lossy sanitized export, plus a bounded local metadata-only audit journal.
- Native maintainer review requirements, CODEOWNERS, stale approval dismissal and cross-platform PR builds.
- Public issue research and a roadmap that distinguishes delivered behavior from future work.

Verification results and remaining platform/dependency limits: [docs/VERIFICATION.md](docs/VERIFICATION.md).

## v0.1.0-alpha.1 — 2026-11-02

Application/package version: `0.1.0`. GitHub prerelease; macOS arm64 public artifact.

### Added

- Rust HTTP(S) interception with a generated Scope CA and binary-safe SQLite sessions.
- Native Tauri desktop UI: traffic inspector, request/response bodies and headers, timing, saved sessions and device setup.
- Request/response breakpoints with editing, original snapshots, abort and bounded expiry; separate request replay.
- HAR import/export and a lossless Scope session format.
- Observed endpoint/error/size/latency analysis and session comparison.
- ADB device/app discovery, device-wide proxy mode and selected-process Frida mode with selectable attributed hook profiles.
- Recorded cleanup/recovery state for device proxy, temporary certificates, forwarding and Scope-owned Frida server resources.
- Seven stdio MCP tools with authenticated loopback transport, explicit session scope, output redaction and opt-in replay.
- A local HTTP fixture and Android demo app for real, reproducible capture checks.
- Automated integration tests, native macOS/Android verification records and desktop build workflows.

### Known limits

- macOS arm64 is the first locally verified host; other desktop runtime/device combinations require validation.
- Bodies are buffered up to 16 MiB each. WebSocket frames, HTTP trailers and QUIC/HTTP3 are not captured.
- Android hook coverage targets selected Java APIs; Android 16/Frida compatibility is experimental.
- Analysis and import/export load session records into memory; long-running stability has not been established.
- The public macOS package is not Apple-notarized. Optional Frida native binaries are installed separately by the user; source integration and hook profiles are included.

Exact checks and device coverage: [docs/VERIFICATION.md](docs/VERIFICATION.md).
