# Snurpo rename residue audit: 12 November 2026

This is a source audit of the pending Scope → Snurpo change. It is not a claim
that the GitHub rename, deployment or a new binary release has completed.

## Current identity checked

Reviewed README, current product/setup/architecture docs, package metadata,
release scripts and workflow labels, app/window labels, frontend assets,
desktop icons, dependency attribution, generated notices and brand provenance.
Current project links use `github.com/dalkedev/snurpo`; current artwork uses
the Snurpo mascot/banner and the dedicated macOS template icon.

The follow-up search found and corrected:

- the remaining Scope wording in `resources/licenses/README.md`;
- an outdated Scope 1.0.0 limitation label in the current architecture document;
- the current issue link in the native-import explanation;
- the CI download artifact label;
- the private HAR tool/lab package names and matching lab lockfile metadata.

The HAR executable now exposes `snurpo-har-compare` while retaining its existing
`scope-har-compare` alias. An unused old header SVG moved from `docs/images/`
to `assets/brand/legacy/`; no current file referenced it.

## Intentional remaining names

| Category | Why it remains |
| --- | --- |
| `dev.scope.debugger`, application-data directory, `scope.sqlite3`, CA filenames and `Scope Local Debug CA` subject | Preserve existing sessions and certificate trust. |
| Rust `scope-*` crates, `scope` desktop binary, `scope-mcp`, IPC `scope_command`, `SCOPE_*`, `scope-theme` and semantic CSS variables | Preserve existing developer commands, integration configuration and preferences. |
| `scope-session`, `.scope.json`, HAR `_scope`, OpenAPI `x-scope`, Frida/Android class and ownership identifiers | Preserve versioned formats, runtime ownership and cleanup compatibility. |
| Old releases, dated acceptance/benchmark/security records, historical screenshots and fixture receipts | Record what was actually built or measured; do not rewrite evidence. |
| `assets/brand/legacy/` | Clearly archived former artwork, exact original prompts and provenance. |
| Synthetic test headers, certificate-case labels, frozen HAR fixture producer labels and protocol test symbols | Preserve test inputs and evidence semantics. |
| Ordinary words such as permission scope, scoped ignores, or third-party package names | These are concepts or upstream names, not stale product branding. |

Historical records can retain old GitHub URLs; their repository redirect is
verified separately after the external rename. The old published Scope 1.4.1
downloads must retain their original names and checksums until a separately
versioned Snurpo release is published.

## Validation and limits

Focused searches found no old repository URL or removed active logo/banner
reference outside those documented historical/compatibility categories. Changed
Markdown local targets resolve. Package JSON parses and HAR lab root names match
its lockfile; whitespace checks pass. Native screenshots and menu-bar behavior
are tracked in the separate [rebrand verification receipt](2026-11-12-snurpo-rebrand.md).
This audit did not repeat the full functional test suites.
