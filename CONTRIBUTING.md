# Contributing to Snurpo

Snurpo is an early local HTTP debugger. Useful contributions include reproducible bugs, platform/device validation, minimal network fixtures, clearer docs and focused fixes. Check [open issues](https://github.com/dalkedev/snurpo/issues) and the [roadmap](ROADMAP.md) before starting a large feature.

## Development setup

Source builds, CI, the HAR CLI and its development lab now use Node 26 LTS.
`.nvmrc` and the workflows pin 26.13.1; package metadata and runtime checks require
at least that version. This source-tooling requirement does not add Node to the
packaged native application's runtime requirements. Earlier verification records
keep the Node versions actually used for those runs.

Install Rust, Node.js 26.13.1+ and [Tauri's prerequisites](https://v2.tauri.app/start/prerequisites/) for your host. Android changes also need SDK Platform Tools. The optional Frida runtime is described in [Android setup](docs/ANDROID.md#frida-runtime).

```sh
git clone https://github.com/dalkedev/snurpo.git
cd snurpo
npm ci
npm run desktop
```

`npm run dev` opens only the browser frontend; native capture, devices and MCP require `npm run desktop`. `npm run package` builds the platform package and companion MCP executable. Optional native Frida resources are not stored in Git.

See [architecture](docs/ARCHITECTURE.md) for workspace ownership and [implementation contract](docs/CONTRACT.md) for internal message shapes. Keep the core independent of Tauri so the desktop and MCP share behavior.

## Before a pull request

Run checks appropriate to the change and report exactly what you ran:

```sh
cargo fmt --all -- --check
cargo test --workspace --locked
npm run build
```

For frontend formatting, run `npx prettier --check "src/**/*.{ts,tsx,css}"`. For packaging changes, also run `npm run package` on the relevant host. If a check needs a device or another OS you do not have, state that limitation in the PR.

Capture changes need an integration test with a real local client/server and the relevant protocol. Parser tests, screenshots and a successful build alone do not establish end-to-end interception. Use synthetic data and localhost fixtures. For Android changes, verify that Stop restores proxy/certificate/forwarding state and preserves resources Snurpo does not own.

Avoid unrelated refactors in a bug fix. Explain the user-visible trigger, resulting behavior, validation and remaining limits. Update docs when configuration or coverage changes. Preserve vendored copyrights/licenses and refresh provenance when upstream resources change.

## Headless and MCP smoke tests

Build the real executables, close other test instances using the default proxy/demo ports, then run:

```sh
cargo build -p scope-desktop -p scope-mcp
node scripts/smoke.mjs
```

The smoke test launches the engine and MCP processes, generates actual local traffic, checks sharing/redaction/replay boundaries, then reopens the same archive to check persistence. It uses `curl` and Node's built-in `node:sqlite`; details and release-binary overrides are in [MCP-TEST.md](docs/MCP-TEST.md).

For manual headless inspection:

```sh
cargo run -p scope-desktop -- --headless --data-dir .scope/qa --demo
```

`--demo` starts the local fixture/proxy, captures a new six-request session and explicitly shares it read-only for that test run. `--demo-server` starts only the fixture. `--bridge-port` changes the default bridge port, and `SCOPE_DATA_DIR` can set the archive location. Stop with Ctrl+C. The regular desktop starts with no shared sessions.

## Report useful evidence

For a bug, include version/commit, host OS/architecture, exact steps, expected/actual behavior and sanitized logs. For Android also include Android version, device ABI, capture mode, root state and both Frida versions. Distinguish “request captured,” “edited response reached the app,” and “cleanup restored the device.”

Use the [bug form](https://github.com/dalkedev/snurpo/issues/new?template=bug_report.yml). For a suspected security problem follow [SECURITY.md](SECURITY.md), not a public capture attachment. Never include credentials, private CA keys, bridge tokens or personal traffic. Small synthetic reproductions are much easier to review.

## Project principles

- Local capture, session storage, import/export and MCP remain usable without a required account or paid gate.
- Device-wide traffic is not presented as proven per-app traffic.
- Analysis distinguishes captured evidence from hypotheses and cites exchange IDs where possible.
- Platform/runtime support claims need checks on that platform, beyond a green cross-platform compilation.
- Recovery and upstream license notices are part of the feature, not optional release cleanup.

By contributing, you agree that your contribution is available under the project's [AGPL-3.0-or-later license](LICENSE), while retained third-party components keep their own terms. No CLA is required. Please follow the [Code of Conduct](CODE_OF_CONDUCT.md).
