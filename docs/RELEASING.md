# Releasing Scope

## Review, tag and publication

The `release/<version>` branch carries the proposed changes, currently
`release/1.3.0`. Opening a proposal does not
create a tag or a public release. [Governance](GOVERNANCE.md) describes the required
maintainer review and bot-authored PR, which lets the maintainer use native
GitHub approval. Dispatch `release.yml` on this branch with `mode: proposal`
with the matching `tag: v<version>` to open that PR. The workflow validates that
the branch matches the version and reads `docs/releases/<version>-review.md`.
This mode skips all release build/upload jobs and creates no tag.

After review and successful required checks, merge the PR and tag the reviewed
main commit. Dispatch the same workflow with `mode: draft` and `tag: v1.3.0` to
build the existing tag into a draft prerelease. Inspect the assets and use
[the 1.3.0 release notes](releases/1.3.0.md) as the publication body. Publication
is a separate maintainer action; for a stable release, clear both the draft and
prerelease flags. Keep historical release assets attached to their original tags.

The first public prerelease, `v0.1.0-alpha.1`, used application version `0.1.0`;
the current source uses application version `1.3.0`.
Repository: [dalkedev/scope](https://github.com/dalkedev/scope).
The recorded 1.0.0 interactive host was macOS Apple Silicon; this does not establish
new-version interactive validation. Windows/Linux packages
have CI build and headless-protocol coverage; their interactive installers and
desktop/device behavior remain experimental and must be labeled accordingly.

## Public artifact boundary

Public app packages include the Rust engine, MCP executable, frontend, JavaScript
instrumentation source, device/setup helpers and license notices. They **exclude**
the optional native Frida worker, embedded Python interpreter and Android Frida
server. This is an enforced packaging choice, not an unresolved public-release
exception: `npm run package` creates a filtered resource stage and Tauri bundle
configuration. Local ignored Frida binaries remain untouched outside that stage.

Frida's exact 17.9.1 SDK manifest contains more than 30 native projects, including
LGPL libraries and V8. Complete artifact-specific notices and corresponding-source
requirements were not established for the prior local prebuilt experiment.
Those native files are therefore not redistributed in public packages. See
the [runtime provenance record](../resources/device/licenses/README.md).

Users can explicitly install Frida from upstream into Scope's private user runtime.
For an app installed in `/Applications` and an Android arm64 device:

```sh
python3 "/Applications/Scope.app/Contents/Resources/resources/device/install-runtime.py" --abi arm64-v8a
```

Python 3.9+ with `venv` is required for this optional step. The helper creates a
private virtualenv, installs pinned Frida 17.9.1 from PyPI and downloads the
official matching Android server after verifying its pinned archive SHA-256.
It does not install system packages or change an Android device. Rust discovers
the managed runtime automatically, so no environment variables are needed.
Selecting Start interception remains the separate action that changes a device.

The missing-runtime screen reports an exact command for that installation and
data directory before any proxy/forwarding/server changes. A fresh private runtime
installation and real Start/Stop with filtered public resources were verified on
macOS using the included Android demo; no bundled Frida binary or override
environment variable was used. A missing-runtime check also confirmed that no
recovery file or proxy/forward mapping was created.

## Rebuild from a clean tag

Use Node **22.22.3**, Rust **1.99.1** and the committed `package-lock.json` and
`Cargo.lock`. The platform also needs Tauri's native build prerequisites.

```sh
npm ci
cargo fmt --all -- --check
cargo test --workspace --locked
npm test -- --run
node --test scripts/release.test.mjs
python3 -m unittest discover -s resources/device/tests
npm audit --audit-level=high
npm run package
node scripts/public-resources.mjs .runtime/public-resources
```

`scripts/desktop.mjs` builds MCP with `--locked`, stages safe public resources and
selects a macOS `.app`, Windows NSIS installer or Linux AppImage/deb. The resource
configuration includes Scope's `LICENSE` and `THIRD_PARTY_NOTICES.md` alongside
the resource directory, so links to the included notices resolve in the bundle.

Linux build prerequisites used by CI are `libwebkit2gtk-4.1-dev`, `build-essential`,
`curl`, `wget`, `file`, `libxdo-dev`, `librsvg2-dev`, `patchelf` and `libssl-dev`.
Dependency security findings, including platform-specific warnings, are recorded
in [DEPENDENCIES.md](DEPENDENCIES.md). A passing build is not a claim that those
warnings or platform runtime checks are resolved.

The public macOS app is ad-hoc signed, without Apple notarization. Do not describe
it as notarized. Verify the final `.app` signature and launch the packaged app,
then perform a real local capture, reopen its archive and inspect MCP behavior.
Use [VERIFICATION.md](VERIFICATION.md) to distinguish tests already run from
unverified platform behavior.

## Prepare assets

Commit all intended source changes and make the version tag point to that commit
before preparing release assets. The helper refuses a dirty tracked checkout or
a mismatched tag, preventing a source archive from describing different code.

```sh
node scripts/release-assets.mjs v1.3.0
```

On macOS this produces:

- `Scope-v1.3.0-macOS-arm64.zip`, preserving the app bundle with `ditto`;
- `Scope-v1.3.0-source.zip`, produced by `git archive` from the exact tag;
- platform checksums and a build manifest recording the commit and exclusion of
  optional Frida binaries.

The macOS helper inspects the **actual app resources**, not only the staging
folder, before creating the zip. Release-resource checks reject native Frida
files, private CAs, SQLite session data, MCP bridge credentials and device
recovery state. Source archives contain tracked files only, so ignored local
runtime binaries, data and build products are excluded.

Combine checksums into `SHA256SUMS.txt` and verify them against the final files
before uploading. Review each target's build results and clearly disclose any
remaining interactive-runtime limitations. Do not reuse a package generated
before the latest source/notice changes.

## CI and draft workflow

Both desktop build matrices use `macos-15` (arm64), `windows-2022` (x64) and
`ubuntu-22.04` (x64). GitHub retired the `macos-14` runner on November 2, 2026;
use a supported runner label when updating either workflow. This build-host
selection does not change the application's minimum supported macOS version.

`checks.yml` runs on pushes and pull requests using explicit runner labels,
commit-pinned actions and locked package dependencies. It checks Rust formatting,
workspace tests, the release filter, setup integrity tests, npm security findings
and native packaging. It uploads build artifacts; it does not publish releases.

`release.yml` in draft mode is manually dispatched with an existing version tag. It checks and
builds macOS arm64, Windows x64 and Linux x64 packages, creates source/checksum
assets, then uploads them to a **draft prerelease**. It refuses to modify an
already public release. A maintainer reviews CI results and platform limitations
before publishing any draft assets; the workflow never automatically publishes
the draft. The currently selected macOS runner architecture is documented by
[GitHub's runner reference](https://docs.github.com/en/actions/reference/runners/github-hosted-runners).

The helper and workflow require an existing `refs/tags/...` reference matching
the clean checked-out commit. A similarly named branch is rejected; draft
creation uses `--verify-tag` so GitHub cannot silently tag a different commit.

The helper and workflow do not push commits or create tags. The maintainer remains
responsible for choosing the reviewed commit/tag and publishing the release.
