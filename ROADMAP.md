# Roadmap

Scope is built around a complete loop: select a target, capture, inspect/change,
save, reopen and analyze. The original 1.0.0 proposal below is historical;
the current source version is 1.4.1. See [Releases](https://github.com/dalkedev/scope/releases)
for published artifacts and [workflow research](docs/COMPETITOR-GAPS.md) for the
next interoperability candidates.
This roadmap is a direction, not a delivery schedule. [Public issue research](docs/USER-NEEDS.md)
links specific user needs to work that is delivered or still planned.

## Proposed in 1.0.0

- Ordered persistent rewrite/mock/block rules with original messages and provenance.
- Edited replay; session notes/tags/pins/deletion; local metadata-only activity.
- Read-only Android/Frida preflight and actionable setup diagnostics.
- Observed OpenAPI schemas, pending-aware analysis and optional sanitized exports.
- Twelve MCP tools with explicit session/write/global-rule/diagnostics permissions.
- Required maintainer review and platform build checks before merge.

## Next: make the first release dependable

- Validate packaged apps on Windows and Linux, then publish a clear host/device compatibility matrix.
- Expand physical-device and Android-version testing with reproducible fixtures and explicit cleanup checks.
- Add macOS signing/notarization and document provenance for every bundled native runtime.
- Exercise interrupted capture, device disconnect/reconnect and long sessions beyond the short functional smoke tests.
- Improve actionable diagnostics for CA trust, pinning, native networking and unavailable Frida runtimes.

## Capture and archive

- Bounded streaming/body storage for large messages and long captures.
- Configurable retention and efficient indexed search (manual deletion exists in 1.0).
- Streaming export, progress/cancellation, size estimates and measured memory bounds for large archives.
- Reliable per-request source identity before claiming concurrent multi-device attribution.
- WebSocket frame inspection and replay where technically meaningful.
- Better handling of additional HTTP features and Android native networking stacks, with tests per supported path.

## Understand and automate

- Portable, versioned rule collections with safe import/export and reusable fixtures.
- More useful traffic comparison with clear evidence links and fewer noisy field differences.
- Configurable custom secret fields beyond 1.0's best-effort known-field sanitizer.
- Explicit custom instrumentation scripts with source/license review and recovery tests.
- MCP capabilities that preserve explicit session scope and distinguish read-only analysis from traffic changes.

## Principles for accepting scope

Local session storage and export stay available without an account or paid gate. Capture support claims require actual protocol/device evidence. New instrumentation must preserve upstream notices and include a reliable Stop/recovery path. Observed traffic must remain distinguishable from a claimed complete API specification.
