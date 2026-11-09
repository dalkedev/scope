# Archive Measurement Implementation Plan

> **For agentic workers:** Use superpowers:executing-plans for inline implementation and a fresh independent whole-source review.

**Goal:** Reproducible bounded evidence for #28 without a production rewrite.
**Architecture:** Ignored unit harness, test-only per-engine probes, hard-bounded Python runner.
**Tech Stack:** Existing locked Rust/Tokio/rusqlite/reqwest and Python standard library.
**Spec:** `docs/superpowers/specs/2026-11-06-archive-measurement-design.md`

## Global constraints

32 logical clients, 30s request/60s cooperative batch budgets, loopback only;
current-thread/two-worker runtimes; separate debug/release; three samples per
HTTP/TLS case; fixed numeric-only collectors; no durability/security changes.

## Review focus

- Timeout paths must stop and join probes/clients or be killed/reaped by parent.
- Collector overflow must invalidate complete-count claims.
- Missing/reordered evidence and request pairing errors must be rejected.
- Positive controls must exercise real timer/network and persisted evidence.
- Reports must not include arbitrary environment, paths, bodies or errors.

### Task 1: Actual save instrumentation and evidence detector

Files: `crates/scope-core/src/lib.rs`, `src/diagnostics.rs` and submodules.
- [x] Write detector tests for missing, reordered, wrong-ID and incomplete evidence, plus capped sample collection; observe RED.
- [x] Implement test-only collector and validator; observe GREEN.
- [x] Instrument actual serialization, mutex and statement boundaries without changing SQL or production order.

### Task 2: Synthetic runtime workload and probes

Files: diagnostic submodules, `scripts/measure-archive.py`, runner unit tests.
- [x] Add ignored harness with real HTTP/TLS interception and all existing evidence semantics.
- [x] Add ready-before-load timer and independent-thread network probes, bounded cleanup and positive controls.
- [x] Add hard parent deadline, profile matrix, sanitized metadata and output validation.
- [x] Run local locked core/headless suites, controls and a small real measurement.

### Task 3: Hosted evidence and proposal

Files: measurement workflow, `docs/ARCHIVE_RESPONSIVENESS.md`, allowlisted result JSON.
- [x] Review complete source before branch publication and explicit dispatch; reconcile receipts.
- [x] Obtain actual hosted Linux/Windows debug/release results and verify artifact provenance.
- [x] Document bounded writer ordering, backpressure, cancellation/restart/error guarantees from evidence.
- [x] Independent whole-branch/result review, affected checks, and one PR #30 referencing #28.

Publication receipts (exact head, readiness, checks and final readback) are maintained
in the PR description; no merge or release is part of this plan.
