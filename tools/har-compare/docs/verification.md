# Verification

## Reproduce from repository root

Use Node.js 26.13.1 or newer. The runtime CLI is dependency-free. Install only
the separately locked development lab for DOM/browser checks:

```sh
npm ci --prefix tools/har-compare/lab --ignore-scripts
npm test --prefix tools/har-compare
node tools/har-compare/examples/run-demo.mjs
node tools/har-compare/examples/sprint/run.mjs
node tools/har-compare/examples/check-html-dom.mjs
```

For a clean sandbox-enabled Chromium and synthetic screenshots, follow
[the isolated CI job](ci.md). Browser installation is optional locally; an
unavailable sandbox is a failed browser check, never a passing substitute.

## Local review baseline: 2026-11-04

Source baseline: `2895fcb3ae8fc2cbf3184e79cc4256c918293494` on PR #18.
Fresh local checks on Node.js 24.22.2:

| Check | Observed result |
| --- | --- |
| `npm test --prefix tools/har-compare` | 182 passed, 0 failed |
| `node tools/har-compare/examples/run-demo.mjs` | Passed all seven expected outcomes; six report pairs match checked-in bytes |
| `node tools/har-compare/examples/sprint/run.mjs` | Passed all thirteen scenarios; repeated output and checked-in golden bytes match |
| `node tools/har-compare/examples/check-html-dom.mjs` | Passed hostile text/no executable resource/no network attempt checks in jsdom |
| `npm run build` | TypeScript/Vite passed |
| `npm test -- --run` | 10 passed across 4 files |
| `cargo fmt --all -- --check` | Passed using repository-pinned Rust 1.99.1 |

A fresh independent read-only review ran all 182 product tests and thirteen
additional adversarial CLI cases on both Node.js 22.22.3 and 24.22.2. It covered exits 0/1/2, malformed and missing
captures, scoped ignores over repeated query variants, endpoint boundaries,
required fields beneath ignored nodes, invalid UTF-8 query identity, HTML/XML
escaping and metadata-only projection. No reproducible correctness/privacy/HTML
blocker was found in that bounded review. This is not a comprehensive security
audit or a guarantee of complete secret removal.

The synthetic demonstrations verify the distinction between observed changes
and authored requirements: unchanged HTTP 500 or a field absent in both captures
can pass observation mode but fail an explicit contract. Invalid or lossy JSON
fails analysis with exit 2. Report checks use synthetic secret markers rather
than real traffic.

## Hosted verification: 2026-11-04

Verified source: `b3fe2b7c78ef8a5c6b5f4ba384b98fb8ccd5d021`.
The isolated workflow is now active. Its [push run](https://github.com/dalkedev/scope/actions/runs/41900596149)
completed successfully on Node.js 22.22.3. Job logs confirm **182 product tests**,
**29 lab tests**, the synthetic demos/goldens and DOM QA passed.

The real sandbox-enabled Chromium check passed on Chromium **156.0.8078.4**:

- The unsafe positive control executed script and detected an attempted request.
- The report executed no script and attempted no network request; hostile text
  remained visible text with no executable/resource nodes.
- CSP blocked injected inline script and unhashed inline CSS; the authorized
  stylesheet rendered. Generated local navigation and details expansion passed.
- Both collapsed and expanded screenshots were generated. They were inspected:
  hostile markup is readable as text, the report layout remains usable, and the
  analysis section expands. This is a bounded synthetic review, not a general
  usability or exhaustive security assessment.

[Download the synthetic artifact](https://github.com/dalkedev/scope/actions/runs/41900596149/artifacts/13358225571):
`lab/output/browser-tF9Jwr/receipt.json`, `hostile-report.png` and
`hostile-report-expanded.png`, plus the six generated demo report pairs.
The downloaded archive matched GitHub's SHA-256 digest and contained only the
allowlisted report JSON/HTML, screenshots and receipt, with no HAR or browser
profile files. Artifacts expire after three days; reproduce with [CI](ci.md).

The previously blocked OAuth workflow upload was resolved by a maintainer's
branch update; the assistant did not change credentials or repeat the rejected
push. A fresh independent source review found no blocker in that workflow.
It uses fresh non-persistent browser contexts with `chromiumSandbox: true`.

Existing desktop/CodeQL checks for `6c1595d` all completed successfully.
Checks for `b3fe2b7` were still completing when this record was written; consult
[the current PR checks](https://github.com/dalkedev/scope/pull/18/checks) for the
actual latest head. Successful recorded runs do not establish a later commit's
CI result. Mark ready only after all declared checks for that head pass.

## Local limits and separate evidence

The local full-workspace attempt with `cargo test --workspace --locked --offline`
failed on uncached `atk 0.18.2`; the local host lacks `pkg-config`. Local headless
workspace tests passed **47 tests** with `--exclude scope-desktop --locked
--offline`. Full hosted workspace tests passed on Linux/macOS/Windows for
`6c1595d`; this does not install desktop prerequisites locally.

Local sandboxed Chromium remains unavailable (exit 2
`BROWSER_SANDBOX_UNAVAILABLE`). Hosted evidence above supersedes the earlier
absence of real-browser/CSP/screenshots, not the local limitation. No sandbox
was disabled and no authenticated browser was used.

No new device claim, device test, package release or production deployment is
part of this HAR change. Capture claims remain limited to the named synthetic
localhost lab scenarios; these report checks do not establish arbitrary native
capture, large-session memory safety or device behavior.

## Boundaries

Readable host/path/field/config labels may contain sensitive information.
`--labels ids` minimizes labels, not interaction patterns or identity risk.
Excluded operations are outside selected-response analysis. Repeated calls pair
by encounter order; reordered or request-body-dependent variants remain ambiguous.
Synthetic capture lab results establish only the named localhost scenarios.
