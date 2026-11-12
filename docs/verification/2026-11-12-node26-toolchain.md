# Node 26 source-tooling baseline — 12 November 2026

Current source builds, CI, the offline HAR CLI and its optional development lab
now require **Node.js 26.13.1 or newer**. `.nvmrc` and active CI/example workflows
pin **26.13.1**. This is an explicit source-tooling minimum increase; installed
native applications do not acquire a Node runtime dependency.

The selected version is the current Node 26 LTS security release, confirmed by the official
[release index](https://nodejs.org/dist/index.json) and
[November security release announcement](https://nodejs.org/en/blog/vulnerability/november-2026-security-releases).
The previous 22.22.3 pin is outdated. An outdated or vulnerable runtime is not
evidence that its distribution was malicious or that an installation was
compromised; this change makes no such claim.

## Scope of the change

- Root, HAR and lab `engines.node` metadata agree on `>=26.13.1`; corresponding
  root package metadata in lockfiles matches, with no dependency version changes.
- CLI and doctor checks enforce the same full minimum version. The sprint
  demonstration coordinator also rejects older runtimes.
- Active setup, release, CLI/lab and reproduction instructions use 26.13.1.
  Historical release notes, benchmark data and dated verification evidence keep
  the versions actually used in those runs.
- The change does not weaken CI, approval, sandbox, package-integrity or source
  verification gates. The current environment is selected per command; it does
  not replace the user's global Node installation.

## Focused checks

- Seven doctor process tests passed, including rejection of 26.13.0 and a passing
  26.13.1 fixture, receipt collision handling and lock preservation.
- The real Node 26.13.1 binary passes both CLI `doctor` and the local toolchain
  doctor; the previous Node 22 runtime is rejected by the CLI with exit 2 and
  the explicit `RUNTIME` result.
- Package/lock root metadata and active pin searches are checked separately from
  the historical records. Generated license notices are refreshed for the new
  root lockfile hash.

Production builds, functional tests, native packaging and hosted CI are recorded
by their own runs. These focused version checks alone do not establish their
success on the final proposal commit.

## Functional checks on the selected runtime

The official macOS arm64 distribution was downloaded from `nodejs.org`; its
SHA-256 matches the official `SHASUMS256.txt`. The selected executable reports
`v26.13.1`. Production frontend build, all 63 UI tests, 205 HAR CLI tests and 11
release/license tests passed on that runtime.

The original failing HAR workflow compared newly branded generated HTML with
old Scope goldens. Exactly 32 title/header replacements across 16 HTML files
correct that mismatch. JSON/XML evidence and CSP/security assertions are
unchanged. The exact failing demo stage now passes on Node 26.13.1: six original
demos plus invalid-input control, thirteen sprint cases, deterministic repeated
output checks and hostile-content DOM tests. Sandboxed Chromium 156.0.8078.4
also passed the real-browser CSP/network controls with no hostile script
execution or network attempts; the positive detector controls passed.
