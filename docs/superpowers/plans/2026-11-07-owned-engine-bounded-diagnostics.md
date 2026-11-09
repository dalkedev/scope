# Bounded Android worker diagnostics follow-up

> **For agentic workers:** Use subagent-driven implementation, TDD and independent review for this focused existing-engine optimization.

**Goal:** Bound host memory consumed by worker stdout/stderr and retained device diagnostics without changing TLS, interception or recovery guarantees.

**Architecture:** Retain the current Scope Java/Frida engine and JSONL protocol. Bound framing before JSON parsing and bound stored diagnostic bytes. No trust decisions, expiry checks or path validation are cached.

**Spec:** Existing `docs/superpowers/specs/2026-11-07-owned-android-engine-design.md`; current worker messages are limited to4000 Python characters. Their ASCII JSON escaping can require48000 bytes, so the protocol frame ceiling is65536 bytes excluding LF/optional CR.

## Evidence and choices

A synthetic malformed/custom worker retained a1MiB log entry;200 entries of16KiB retained3276800 bytes. Standard Python emit already limits normal messages. This is defensive host-memory control, not an observed Android throughput result or demonstrated TLS bypass.

A separate fixed class-wrapper-cache candidate reduced repeated Java.use dispatch but the retained bridge already caches wrappers. Warm class-factory misses were zero before and after; Node timings were mixed. Do not apply it or claim Android/JNI speedups.

## Constraints and review focus

- Maximum stdout/stderr line:65536 bytes, enforced during incremental reads before JSON parsing; never allocate proportional to an oversized line.
- Stored diagnostic entry: at most4096 UTF8 bytes including any truncation marker; maximum200 entries. Preserve valid UTF8 boundaries.
- Oversized/malformed stdout is a terminal protocol failure and retains recovery. Never truncate and parse readiness or termination receipts. Continue bounded draining/discard through line boundaries so pipes and cleanup do not deadlock.
- Stderr is diagnostic only: report/truncate oversize safely, discard its remainder and resume subsequent lines; retain generation isolation.
- Preserve LF/CRLF and final non-newline receipt behavior, exact matching Stop acknowledgments, output-drain grace and existing pending-PID recovery. I/O/UTF8 errors must not create a successful cleanup acknowledgment.
- Preserve full TLS/CA/pin/hostname/expiry/original-error behavior, support scope, native settings, dependency locks, source history and draft status. No merge, release, dispatch or host-security change.

## Implementation and validation

- [x] Add production-path regressions demonstrating oversized log retention and unbounded framing; observe RED.
- [x] Implement a shared bounded incremental reader and byte-limited retained diagnostics in `crates/scope-device/src/lib.rs`; keep changes focused.
- [x] Verify exact/over-limit frames, fragmented and no-newline input, UTF8/CRLF, recovery after oversize, terminal oversized stdout followed by apparently valid ready/stopped, and stale stderr generation. Prove deliberate controls fail without bounds.
- [x] Run affected Rust suites and independent adversarial tests; compare deterministic retained bytes, not incompatible wall times.
- [x] Obtain independent scoped review and resolve findings; no actionable findings remain.

Publication and exact-head hosted check receipts are tracked in the same draft PR after ref reconciliation. No merge or ready transition is implied.

Real Android/Frida acceptance remains a separate prerequisite gate. Local host tests never count as Android execution.
