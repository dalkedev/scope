<p align="center">
  <img src="assets/brand/scope-banner.png" alt="Scope — See what your app is saying. HTTP(S) debugging: capture, inspect and compare." width="100%">
</p>

<p align="center">
  <strong>The HTTP conversation is yours to inspect.</strong><br>
  A desktop workbench for HTTP(S), Android instrumentation and agent-assisted debugging.
</p>

<p align="center">
  <a href="docs/GETTING_STARTED.md">Get started</a> ·
  <a href="https://github.com/dalkedev/scope/releases">Downloads</a> ·
  <a href="docs/SCREENSHOTS.md">Screenshots</a> ·
  <a href="docs/MCP.md">MCP</a> ·
  <a href="CONTRIBUTING.md">Contribute</a>
</p>

<p align="center">
  <a href="https://github.com/dalkedev/scope/actions/workflows/checks.yml"><img src="https://github.com/dalkedev/scope/actions/workflows/checks.yml/badge.svg?branch=main" alt="Build and check"></a>
  <a href="LICENSE"><img src="https://img.shields.io/badge/license-AGPL--3.0--or--later-2f64ed" alt="AGPL-3.0-or-later license"></a>
</p>

Scope routes traffic through a Rust engine, gives you control over requests and responses, and saves the result in a local SQLite archive. Inspect it yourself, reopen it tomorrow, or let an MCP client work from the same captured evidence.

**Local capture, saved sessions and export need no account or subscription.** Built-in analysis runs locally. You choose which sessions an external agent can access.

> **This README describes Scope 1.3.0.** Endpoint rules can pause a real request, response or both for manual editing before forwarding. Other endpoints keep moving; the paused-message editor is available from every workspace. Automatic rules, the offline HAR Action and the verified GLib backport remain included. Check [Releases](https://github.com/dalkedev/scope/releases) for published builds and platform notes. [Pause an endpoint →](docs/RULES.md#pause-an-endpoint-and-edit-the-real-message) · [Verification record →](docs/VERIFICATION.md)

<a href="docs/SCREENSHOTS.md"><img src="docs/images/traffic-inspector.jpg" alt="Real captured traffic: a products request returns a local HTTP 503 mock, with its rule and response JSON visible." width="100%"></a>

## Follow a request all the way through

| Capture & inspect                                                                                     | Change & repeat                                                                                           | Save & understand                                                                                               |
| ----------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------- |
| HTTP and CA-signed HTTPS interception. Repeated headers, binary bodies, timing and explicit failures. | Ordered request/response rewrites, local mocks and blocks. Pause a live transfer or compose a new replay. | Named sessions with notes, tags and pins. HAR/native archives, session comparison and observed OpenAPI schemas. |
| [Capture walkthrough →](docs/GETTING_STARTED.md)                                                      | [Rule semantics →](docs/RULES.md)                                                                         | [Architecture & storage →](docs/ARCHITECTURE.md)                                                                |

A rule-generated response is marked as a **mock**. Rewrites retain the **original** message. A replay creates a **new exchange**. The inspector keeps those distinctions visible so your evidence stays useful.

<table>
  <tr>
    <td width="50%"><a href="docs/RULES.md"><img src="docs/images/traffic-rules.jpg" alt="Persistent rule editor: match the local products endpoint and return a maintenance response."></a><br><strong>Your rules, saved.</strong><br>Match method, host, path or URL. Rewrite, mock or block.</td>
    <td width="50%"><a href="docs/GETTING_STARTED.md#automate-a-local-mock"><img src="docs/images/replay-composer.jpg" alt="Replay composer with an edited POST destination and JSON body."></a><br><strong>Change it. Send it again.</strong><br>Edit the destination, method, full headers and body.</td>
  </tr>
</table>

## A real capture in a minute

Open Scope and select **Generate real test traffic** — the lightning bolt beside **New session**. It sends six real localhost requests through the proxy, including an intentional sign-in error and repeated product calls.

1. Select a row and inspect the response, headers and timing.
2. Open **Analysis** to follow an observation back to its exchanges.
3. Stop capture, quit and reopen. Your session is still there.

No Android device is needed for this first run. [Continue with HTTPS, Android, breakpoints and export →](docs/GETTING_STARTED.md)

### Find the request. Check the API change.

The desktop now accepts combined filters such as `filter: method=POST status>=500`
or `filter: body~="rate limit"`. Conditions apply before pagination; ordinary
text searches remain available. [Search syntax →](docs/SEARCH.md)

The source distribution also includes a separate, dependency-free Node CLI:

```sh
node tools/har-compare/cli.mjs compare --baseline before.har --candidate after.har \
  --json new-report.json --html new-report.html --junit new-report.xml --labels ids
```

Compare saved HARs locally, author status/JSON requirements and attach deterministic
reports to CI. Exit codes distinguish a clean comparison (`0`), configured
regressions (`1`) and incomplete/invalid analysis (`2`). Observation alone is not
an API contract: an unchanged HTTP 500 needs an explicit status assertion to fail.
Reports minimize captured data; inspect them before sharing.
[CLI, examples and privacy limits →](tools/har-compare/README.md)

The [reusable GitHub Action](.github/actions/har-compare/README.md) wraps the same
offline CLI for Linux runners. It exposes reports and separate `0`/`1`/`2`
results, defaults to minimized labels and lets the caller defer its final gate.
It never uploads captures or reports automatically. Supply Node 22.22.3+ and Bash;
review reports before choosing to share them.

### Build from source

Install Rust, Node.js **22.13+** and [Tauri's native prerequisites](https://v2.tauri.app/start/prerequisites/). The repository pins the Rust compiler and dependency lockfiles.

```sh
git clone https://github.com/dalkedev/scope.git
cd scope
npm ci
npm run desktop
```

`npm run package` builds a native application and its companion MCP executable. `npm run dev` opens the frontend preview; capture needs the desktop app.

## Android, with setup you can inspect

Discover connected devices and installed apps over ADB. Use the **device-wide proxy** path, or instrument a **selected process with Frida** and explicit hook profiles. Read-only diagnostics explain authorization, root, ABI, runtime/server versions, mappings and recovery state before you connect.

Frida is an optional, separate runtime installation. The experimental Scope-owned engine supports selected Java URL, SSLContext and OkHttp3 seams; native TLS and custom pinners are unsupported. **Stop ends the instrumented app process** before restoring recorded Scope-owned state and preserving external changes. System proxy mode requires manually configured app trust for HTTPS and never installs a system CA. [Setup, supported paths and recovery →](docs/ANDROID.md) · [Diagnostic reference →](docs/DIAGNOSTICS.md)

## An agent can read the evidence. You set the scope.

Share selected sessions in **MCP server**, then copy the generated configuration into your MCP client. The twelve tools cover search, request inspection, analysis, comparison, observed OpenAPI, edited replay, global rules and device diagnostics.

| Permission         | What it permits                                                                                            |
| ------------------ | ---------------------------------------------------------------------------------------------------------- |
| Shared sessions    | Read and analyze only the sessions you select.                                                             |
| Traffic changes    | Explicitly resend captured or edited requests.                                                             |
| Global rule access | Read a sanitized inventory; writes additionally need traffic-change permission. Rules affect all captures. |
| Device diagnostics | Read environment and connection evidence, including local paths and device IDs.                            |

Everything starts off and resets on restart. Known credentials are redacted from MCP results; redaction is best-effort. Raw archives remain local, and a separate sanitized export creates a clearly lossy sharing copy. [Tool reference & privacy boundaries →](docs/MCP.md)

## Built to be inspected

**Rust engine · Tauri 2 · React · TypeScript · SQLite · stdio MCP**

The same engine powers the native inspector and MCP. Real HTTP/TLS fixtures test interception, rules and replay; device fixtures test setup and ownership; the process smoke reopens an actual archive and checks agent access after restart.

<a href="docs/SCREENSHOTS.md"><img src="docs/images/analysis.jpg" alt="Analysis of the real local capture: observed endpoints, intentional errors, timings and synthetic response provenance." width="100%"></a>

| Today                   | Boundary                                                                                                                                                                  |
| ----------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| macOS arm64             | Local UI/package evidence is recorded by version; 1.3.0 interactive validation is recorded separately by scenario. Ad-hoc signed; not Apple-notarized.                                          |
| Windows x64 / Linux x64 | CI builds, packages and headless MCP/capture smoke pass. Interactive UI and device validation remain open. Intel macOS is unverified.                                     |
| Android / Frida         | Earlier smoke covered the previous hook backend on one emulator. The current Scope-owned engine has local simulated tests only; real Android/Frida/OkHttp TLS and Stop acceptance are NOT RUN.                       |
| Capture                 | Bodies are buffered up to **16 MiB each**. Streaming, WebSocket frames, trailers and HTTP3 are not implemented.                                                           |
| Archives & analysis     | Records currently load into memory. Large-session streaming/export is planned. Observed schemas are not complete API contracts.                                           |
| Dependencies            | A Linux glib unsoundness advisory remains tracked in [#11](https://github.com/dalkedev/scope/issues/11). See the full [dependency record](docs/DEPENDENCIES.md). |

Breakpoints expire after 60 seconds. [What was actually tested →](docs/VERIFICATION.md)

## Help shape the next release

Useful contributions include reproducible device/OS reports, protocol fixtures, platform validation and improvements to large-capture handling. The roadmap connects proposals to [real public user feedback](docs/USER-NEEDS.md), with a clear distinction between delivered features and planned work.

[Contributing](CONTRIBUTING.md) · [Roadmap](ROADMAP.md) · [Report a bug](https://github.com/dalkedev/scope/issues/new/choose) · [Security](SECURITY.md) · [Changelog](CHANGELOG.md)

Every change goes through maintainer review and required platform checks. [Ownership & release process →](docs/GOVERNANCE.md)

---

<p align="center">
  <a href="docs/BRAND.md"><img src="assets/brand/scope-logo.svg" alt="Scope logo and brand assets" width="48" height="48"></a>
</p>

An independent project by **[dalkedev](https://github.com/dalkedev)**. Scope uses its original Java interception engine with Frida and an unchanged, attributed Frida Java bridge. The bridge distribution provenance and third-party licenses remain in [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md).

**AGPL-3.0-or-later** · [License](LICENSE) · [Third-party notices](THIRD_PARTY_NOTICES.md) · [Brand assets](docs/BRAND.md)
