# Scope-owned Android engine Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace HTTP Toolkit runtime hooks with an original CA-scoped Android interception engine in one independently reviewed PR.
**Architecture:** Original JavaScript hook/policy code runs in the selected app through the existing Python/Frida worker. Rust retains proxy/recovery management and materializes only the new assets. Imported system trust injection and unsupported root profiles are removed explicitly.
**Tech Stack:** JavaScript, Python, Rust, existing Frida Java bridge; pinned existing dependencies.
**Spec:** docs/superpowers/specs/2026-11-07-owned-android-engine-design.md

## Global Constraints

- Preserve existing Rust proxy/TLS/archive behavior and source history; no license change.
- New runtime implementations must be original, not copied or mechanically translated upstream hooks.
- Supported profiles are `proxy`, `certificate`, `unpinning`; an empty selection means all three. `unpinning` requires `certificate`.
- Unsupported `root`, `system-ca` or unknown profiles fail before any device probes or state mutation.
- No trust-all manager, hostname-verifier bypass, certificate-expiry bypass or global system trust-store mutation is allowed.
- Only validated Scope-CA chains may bypass supported OkHttp pin checks; all other chains keep original checks/exceptions.
- Readiness requires installed requested capabilities. Stop terminates only the exact worker-spawned process; failures remain visible.
- Keep legacy CA removal recovery; document proxy/root/native coverage reductions.
- Public artifacts contain no private context, paths, captures, credentials or machine identifiers.
- Never claim mock tests are Android results. Real unavailable gates are NOT RUN. No merge/release/tags/admin/engine changes.

## Review Focus

- Unrelated/expired/malformed chains must not gain trust or bypass pins.
- Partial hook installation and missing overloads must roll back, never report ready.
- Explicit profiles including stale root/system-ca values must fail before device writes.
- Worker cancellation/startup error must not leave an owned instrumented process active or kill unrelated PIDs.
- Old recovery records must still clean only Scope-owned CA/mappings after injection is removed.

### Task 1: Original hook engine and worker contract

**Files:** Create `resources/frida/scope-engine.js`, `resources/frida/tests/scope-engine.test.cjs`, `resources/device/tests/test_frida_worker.py`; modify `resources/device/frida_worker.py`.
**Interfaces:** Worker `build_script(root, ca, port, profiles) -> str` remains available. `normalize_profiles(profiles) -> list[str]` is shared by the CLI/builder. New JS exports a testable engine factory/policy boundary and runs through documented Frida `Java` APIs. JSON runtime config uses public CA/loopback proxy/normalized profiles. Readiness payload identifies engine `scope-java-v1`, selected profiles and installed capabilities; Python validates it. Existing Rust JSONL `ready/error/stopped/detached` kinds remain compatible.

- [x] Write executable JS and Python failing tests for CA-scoped pin decisions, original rejection propagation, original trust delegation, explicit/default/unsupported profiles, missing classes and transactional rollback, hostile config serialization, readiness validation and owned-PID cleanup.
- [x] Observe RED with expected missing implementation or wrong existing behavior; record exact commands/output in report.
- [x] Implement original engine (small focused helpers in one runtime file), worker JSON config construction, capability-aware ready gate and finite owned-process cleanup; preserve CLI listing/version and bounded diagnostics. Use the actual Frida API, not a production test stub. Any unimplemented requested capability is an error.
- [x] Run focused tests plus complete Python device suite; self-review. Do not claim fixture-based adapter tests are device verification.
- [x] Commit only Task1 files and report exact RED/GREEN and remaining limitations.

### Task 2: Application integration, migration and release verification

**Files:** Modify `crates/scope-device/src/lib.rs`, `src/App.tsx`, `README.md`, `docs/ANDROID.md`, `THIRD_PARTY_NOTICES.md`, `.github/workflows/checks.yml`, affected existing tests; remove six imported HTK runtime JS/config files and `resources/device/install-system-ca.template.sh`; retain Frida bridge/licenses. Add `docs/ANDROID_ENGINE.md` with explicit coverage/evidence.
**Interfaces:** Consume Task1 `scope-engine.js`, `normalize_profiles` and JSONL engine/capability payload. Rust persists worker-spawned target PID/pending-cleanup before readiness where observable and requires explicit successful termination acknowledgment (or conservative proof that the recorded process no longer exists) before clearing recovery; worker timeout/nonzero/missing acknowledgment must remain an error. New recovery fields are backward-compatible serde defaults. Startup failure allows bounded worker teardown before kill. Rust rejects unsupported profiles before ADB/root probes; ASSETS materializes only `frida/scope-engine.js`, existing Frida bridge and worker. Remove automatic system CA installation, preserve cleanup of old recovery ca_hash. UI removes unsupported root option and explains Stop ends the instrumented process.

- [x] Add regression tests for early unsupported profile rejection, asset construction and legacy owned-CA recovery behavior where affected; observe RED.
- [x] Wire original engine and remove imported runtime source/injection path. Preserve unrelated Rust proxy and recovery semantics.
- [x] Update docs/notices accurately including retained bridge distribution provenance, current original engine, explicit reduced coverage and device NOT RUN gates; no licensing cleanup beyond truthful component inventory.
- [x] Add the executable JS engine test command to the existing three-platform checks using existing pinned actions. Keep existing checks/security unchanged.
- [x] Run focused Rust device tests, all JS/Python suites, UI tests/build when available, formatting and source/privacy/provenance checks; record unavailable desktop/device gates explicitly.
- [x] Commit and report component evidence for independent spec/quality review; record real-device gates as NOT RUN.

### Task 3: Honest OkHttp boundary and reproducible Android acceptance fixture

**Files:** Modify `resources/frida/scope-engine.js`, its executable tests and `docs/ANDROID_ENGINE.md`; create `examples/android-engine-fixture/AndroidManifest.xml`, `src/com/scope/enginefixture/MainActivity.java`, `build.py`, `serve.py`, `README.md`, `.gitignore` and small host-side tests for the helpers. Update notices/plan/spec where needed. A deferred stderr-generation log race may be fixed in `crates/scope-device/src/lib.rs` with a narrow regression.
**Interfaces:** Fixture APK package `com.scope.enginefixture`; explicit local test URLs only (`http[s]://127.0.0.1:<port>/...`), `platform` and real `okhttp` request paths, bounded one-request-at-a-time execution and machine-readable synthetic-only Logcat receipts. Build takes explicit SDK/JDK/output paths, verifies locked real OkHttp3.14.9 + Okio1.17.2 artifacts by SHA-256, preserves their license notices in the APK, and performs actual APK signing/verification. No libraries, APKs, keystores or TLS keys are committed or included in production resources.

- [x] Add a failing regression showing recognized Kotlin lazy OkHttp cannot be advertised as supported: original-manager cleaning can discard the Scope peer chain before the lazy pinner sees it. Reject configurations containing `check$okhttp` before unpinning readiness, with transactional rollback. Remove the misleading lazy-support claim/code; preserve typed non-Scope/original-error controls for ordinary Java overloads. Do not add an unmeasured chain-cleaner override.
- [x] Add an original owned HTTPS test app using fresh SSLContext.init and real pinned OkHttp3.14.9; a deliberately mismatching pin must remain configured in the fixture. Preserve hostname verification, certificate trust and original request errors. A bounded local HTTP reachability control is separate from actual certificate/pin rejection.
- [x] Build helpers acquire only the two locked small test JARs from exact Maven Central URLs; SDK/JDK are supplied, never installed. Pin OkHttp SHA-256 `2570fab55515cbf881d7a4ceef49fc515490bc027057e666776a2832465aeca0`, Okio SHA-256 `f80ce42d2ffac47ad4c47e1d6f980d604d247ceb1a886705cf4581ab0c9fe2b8`. Reject corrupt downloads before executing build tools, bound download size/time, keep all generated output outside tracked source.
- [x] Provide a loopback-only synthetic TLS fixture generator/server with ephemeral Scope/foreign CAs and valid/wrong-host/expired leaf cases, finite execution/cleanup, no global trust-store change or broad device actions. Private test keys stay in a temporary build/runtime directory; no request bodies, tokens or real account data in reports.
- [x] Document exact no-hooks / certificate-only / certificate+unpinning / Stop+restart manual acceptance matrix and commands for an explicitly selected owned lab app/device. Do not automatically select/install/start a real device. Actual runtime gates remain NOT RUN after the failed software-emulator prerequisite; a compiled APK is not a TLS PASS.
- [x] Observe focused host-helper/engine RED→GREEN, compile a real signed APK with the task-local supplied SDK/JDK, record exact tool/library versions and SHA-256 artifact receipt outside source. Run affected JS/Python/Rust checks if changed, then commit only source. Independent task review covers this component; whole-branch review and draft publication remain separate gates below.

## Final review and publication gates

- [x] Obtain independent whole-branch source and requirements review, including cross-component lifecycle, compatibility and provenance.
- [x] Resolve blocking review findings with covering tests and scoped re-review.
- [ ] Record final local and hosted PASS / FAIL / NOT RUN for the exact published head.
- [ ] Create exactly one draft PR after local source review and verify its URL, head, body and state by readback.

Actual Android/Frida/TLS/unpinning/Stop gates remain NOT RUN after the failed software-emulator prerequisite. The PR stays draft unless these acceptance gates are separately evidenced; successful host fixtures or APK signatures do not replace them. No merge, release or tag operation is part of this plan.
