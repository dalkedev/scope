# Snurpo rebrand — local validation, 12 November 2026

This records checks on the rebrand working tree based on
`312802c5a89813af2d85d11fbcc1c992f57cb01a`. It is not a new published release.
The source version remains 1.4.1; historical Scope 1.4.1 assets are unchanged.

## Passed locally on macOS arm64

- `npm ci` and `npm run build` (TypeScript plus production Vite bundle).
- `npm run test:ui`: 63 tests in nine files.
- `cargo test --workspace --locked`: 108 passed, one existing ignored.
- After adding the macOS-only tray feature, `cargo test --locked -p scope-desktop`:
  six tests passed. `cargo fmt --all -- --check` passed.
- HAR CLI suite: 205 tests passed.
- Device Python suite: 23 tests passed.
- Frida policy/adapter Node suite: 17 tests passed.
- Release and license-attribution Node suites: 11 tests passed.
- Third-party notices regenerated after the tray feature: 228 Rust license
  sections and seven frontend package notices, with current lockfile hashes.
- `npm run package` produced `Snurpo.app`. Bundle metadata uses Snurpo and retains
  `dev.scope.debugger`, the compatible `scope` executable and version 1.4.1.
- `codesign --verify --deep --strict` passed for the ad-hoc signed bundle.
- Public-resource verification passed for 64 files.
- Real desktop/MCP headless smoke passed: twelve tools, real localhost capture,
  credential redaction, access/write gates, persistence and restart isolation.
  It modified no Android device and sent no external network traffic.
- `git diff --check` passed.

The macOS menu-bar template has genuine transparent eye/face cutouts, no opaque
white pixels, and a 36 × 36 export. The locked native tray renderer scales it to
18 logical points and macOS supplies template tint. The menu only shows the
window or quits through the existing cleanup path. No automatic startup, proxy
or window-close behavior was introduced. Windows and Linux do not enable the
new tray feature.

## Remaining native review and boundaries

Native GUI review verified the Snurpo title/mascot and real local capture, saved
mock rule, edited replay, analysis and Stop capture. The normal native application
menu Quit ended the review process. See [capture provenance](../images/README.md).
The macOS status-item icon/menu could not be reached through available automation;
its Show/Quit actions remain interactively unverified.
No new Windows/Linux UI or Android execution was performed for this brand change.
The package is ad-hoc signed, not notarized, and is a local review artifact.

The local Rust toolchain emitted a non-fatal `rust-objcopy` missing `libLLVM.dylib`
warning during symbol stripping. Compilation, bundling and signature verification
completed successfully; this receipt does not claim a warning-free release build.

[Migration and compatibility identifiers](../MIGRATING_TO_SNURPO.md) ·
[Artwork provenance](../BRAND.md)

## First-run and interaction polish

The empty Traffic view now presents **Generate test traffic** as its primary
button, with **Connect a device** secondary. The description explains that the
six requests run on localhost. Lavender selections, primary buttons, links and
focus indicators plus mint brand details match the new mascot; HTTP method,
status, error, warning and capture roles retain their meaning. Response and replay
tab indicators use the accent color rather than a low-contrast selected-surface
color.

After these changes, all 63 UI tests and the production frontend build passed.
The final macOS bundle was rebuilt, its signature reverified and all 64 public
resource files passed verification. Calculated foreground/background contrast for
new primary buttons is 5.92:1 in light appearance and 5.51:1 in dark appearance;
the dark hover state is 4.79:1 and the mint local badge is 9.11:1. These checks cover
the changed color pairs, not a claim of a full WCAG audit.


## Light appearance contrast correction

The optional `06-analysis-light.jpg` in the local review outputs records the
**pre-fix** appearance and is omitted from the public gallery. Review found
legacy light fallbacks at 2.18:1 for insight text and 1.87:1 for endpoint hosts.
Explicit light semantic text tokens now replace those pale fallbacks; dark
sidebar tokens remain separate so its metadata does not become dark-on-dark.
Semantic success/warning/error hues are retained with stronger light foregrounds.
The light muted foreground on the insight surface is now 5.37:1; the error metric
on white is 6.44:1. Stop-capture hover uses the danger surface, not the purple
primary-action surface. Dark appearance is unchanged by this correction.

The rule library now says **Create another rule** when rules already exist. Its
seven existing rule-editor tests passed. The final light-token UI test run and
production build passed. These are focused checks, not a full accessibility audit.

The rebuilt native app was also reopened with the isolated review data directory.
Its eight captured exchanges persisted. Light analysis text and endpoint metadata
were visibly readable; `07-analysis-light-fixed.jpg` in the local outputs records
this state without a cursor. The existing-rule editor displayed **Create another
rule**. Capture remained stopped and native application-menu Quit completed.
This does not establish the separate status-item menu's Show/Quit behavior.
