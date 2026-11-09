# Recorded verification

## Actual Scope-owned Android acceptance — 2026-11-09

**PASS:** the direct production-worker matrix observed 75/75 expected synthetic outcomes and 2/2 owned-process Stop/restoration gates on AOSP Android 16/API 36 ARM64, Frida 17.9.1 and OkHttp 3.14.9. All phases, original SSL exception classes, pin/hostname markers, initial two failed outcomes, the explicit Java Key-cast correction and exact engine/worker hashes are recorded in [Android acceptance](ANDROID_ACCEPTANCE.md) and its [allowlisted JSON](verification/android-engine-2026-11-09.json).

Fresh baseline requests after both Stops restored original rejection behavior; matching successful termination receipts, worker exit 0, app-process absence and removal of owned reverse mappings were observed. A genuine OkHttp 4.9.3/Kotlin 1.4.10 APK confirmed baseline HTTP 200, then unsupported lazy-pinner rejection before readiness, worker exit 1 and successful owned-process cleanup. This establishes rejection, not Kotlin unpinning support.

**PASS:** a rebuilt, relocated macOS arm64 app completed two Android start–Stop cycles in the same process. Actual platform/OkHttp HTTP requests reached an independent loopback server, appeared as HTTP 200 with the expected JSON, and remained in SQLite after stopping. Both spawned processes and owned reverse mappings disappeared; Start became available again, and an active Frida panel retained its actual mode. The [native UI receipt](verification/android-native-ui-2026-11-09.json) records the source hashes and scope. Full Rust workspace (100 tests), UI (57 tests), Java engine (17 tests), Python worker (23 tests), production build/package and relocated desktop/MCP smoke pass.

Crash/restart recovery with an active Android worker, physical devices, other API/ABI combinations and native TLS remain unverified. No global Android-support claim follows from this measured target. Earlier NOT RUN entries below describe their dated evidence, not the current result.

## Android worker diagnostic memory follow-up — 2026-11-07

The follow-up to the initial reviewed engine head `4edeb09` bounds worker stdout/stderr framing before JSON parsing and retained device logs. See [exact limits and lifecycle behavior](ANDROID_ENGINE.md#bounded-host-diagnostics).

A reproducible synthetic profile sends 205 diagnostics of 16,384 bytes and retains the latest 200. Stored UTF-8 payload fell from **3,276,800 to 819,200 bytes (75% less)**. The bounded candidate's actual retained String capacities also total 819,200 bytes. A malformed/custom 1 MiB JSON diagnostic is now a terminal stdout protocol failure, rather than a stored large entry. These are host diagnostic-storage measurements, not total RSS, normal-traffic latency, JNI or Android throughput claims. The fixed framing buffers add a known bounded allocation per pipe; no general low-traffic memory improvement is inferred.

- **PASS:** observed RED→GREEN production regressions for oversized unterminated stdout, over-limit readiness/cleanup receipts, UTF-8 storage boundaries/capacity and invalid UTF-8 after a valid Stop. Previously that read error silently ended the monitor; it now preserves failure/recovery.
- **PASS:** 45 device Rust tests and 94 headless core/device/MCP tests; 16 Java engine adapter tests; 23 Python device tests. Added guards revalidate the same peer/CA after expiry changes and verify exact CA input-size boundaries and reread changed contents.
- **PASS:** ten independent production-path acceptance probes cover fragmented UTF-8, exact/over-limit LF/CRLF/EOF, no-newline overflow, sticky failure despite later receipts, I/O error after Stop, bounded stderr resumption/generation isolation and actual child-pipe draining/cleanup. These copied-source probes are separate from the committed regression suite and are not Android evidence.
- **PASS:** independent scoped source review, formatting and whitespace checks. One unchanged fake-ADB fixture produced `Text file busy`; an isolated check and final full suite passed. No timeout or fixture behavior was weakened; the failure is preserved in the implementation receipt.

A separate class-wrapper-cache candidate was rejected: the retained Frida bridge already caches successful wrappers, warmed class-factory misses were zero before/after, and bounded Node timing samples were mixed. Fewer Java.use dispatches do not establish a native or Android speedup; no such product change was made.

The emulator prerequisite investigation progressed beyond the earlier crash site using ordinary private directory preparation, but ADB still aborts because its standard user-storage directory is read-only in this executor. No host security setting, standard home directory or system trust store was changed. At that point actual Android/Frida/TLS/pinning/hostname/Stop acceptance was **NOT RUN**. The later [9 November target-specific record](ANDROID_ACCEPTANCE.md) supersedes that direct-worker gate; it does not infer other platform or native desktop lifecycle results. Exact final hosted check receipts are recorded in the same PR after publication.

## Scope-owned Android engine boundary — 2026-11-07

The source candidate reviewed on 7 November replaces the imported hook backend with Scope-owned Java hooks and ends the owned app process on Frida Stop. Local executable Node/Python policy/adapter and Rust lifecycle tests use simulated Java/Frida or local child processes; they do not execute Android TLS. At that review, real Android/Frida/OkHttp TLS, hostname controls, API/ABI compatibility and Stop restoration acceptance were **NOT RUN**; the later [9 November measured-target record](ANDROID_ACCEPTANCE.md) is separate. Earlier emulator receipts below describe the previous backend and remain historical. See [current engine coverage](ANDROID_ENGINE.md).

Local source gates passed on Linux: 84 locked headless Rust tests (core/device/MCP), 56 UI tests plus TypeScript/Vite build, 26 Node engine/license/resource tests, 22 device-worker Python tests, 17 integrity-script tests and 10 fixture-helper tests. The final worker preflight regression was observed failing on malformed DER, relabelled synthetic key DER, trailing DER and a non-CA leaf, then passing with complete single-CA parsing before Frida import. The affected Python and embedded-worker Rust suites were rerun after that fix; other checked component source remained unchanged.

The owned fixture APK compiled, signed and passed apksigner v2/v3 verification with the real checksum-locked OkHttp 3.14.9/Okio 1.17.2 test dependencies. Host TLS controls accepted the valid fixture CA and rejected foreign, wrong-host and expired peers. These are host/build results, **not Android execution**. One bounded software-emulator prerequisite boot failed with SIGSEGV before Android startup; cause is unestablished. Independent component and whole-branch source reviews resolved all reported findings with covering regressions and scoped re-reviews. The first hosted Windows run exposed CRLF serialization differences after the bounded binary read. Explicit LF/CRLF/CR byte-input regressions reproduced that failure, and universal-newline normalization restored the prior text-read behavior without changing CA validation. Hosted exact-head check receipts belong in the draft PR and are separate from these local results.


## Endpoint manual editing candidate — 2026-11-06

Scope 1.3.0 adds a `pause` Traffic rule with request, response or both-stage
selection. This record describes the local candidate, separately from hosted
checks and release artifacts.

- **PASS:** locked Rust workspace: 78 tests on macOS arm64, including 16 real
  rule integration tests and the native demo/pause conflict guard. Tests cover
  HTTP/TLS edited wire values, original matcher snapshots, automatic action
  ordering, terminal mock/block responses, deduplication, replay exclusion,
  invalid-edit retention, expiry, abort and stop cleanup.
- **PASS:** 55 UI tests, TypeScript and Vite production build. Regressions cover
  pending messages across workspaces, separate resolve state, exact unchanged
  payloads, stale/out-of-order polling, preservation of extension-method case,
  and an editable proxy port that survives status polling before capture starts.
  Regular capture, demo and Android starts use that selected port; invalid values
  fail before session/listener/device side effects. A delayed pre-stop running
  snapshot cannot replace the next idle port selection.
- **PASS:** 17 integrity tests and exact GLib archive/patch/locked-selection
  verification; eight license attribution and three release/resource tests.
  Regenerated notices contain 226 Rust sections and seven frontend notices;
  third-party versions and the vendored GLib source remain unchanged.
- **PASS:** native macOS arm64 packaging and strict ad-hoc signature verification.
  The packaged desktop and embedded MCP executable pass the real smoke workflow,
  including authentication, capture, redaction, persistence, restart isolation and
  graceful shutdown. Public resource filtering excludes optional native Frida.
- **PASS:** an isolated native app, separate data directory and loopback HTTP
  fixture exercised a host/path-matched both-stage rule. Before continuation the
  upstream had no request. The native editor changed `X-Preview` and request JSON;
  the independent server received both exact changes. Its actual HTTP 200 response
  then paused before the client received a body. Editing status to 202 and replacing
  its JSON produced those exact values at the independent curl client. The banner
  and editor were used from Connection settings, with global breakpoints off.
  SQLite retained the original request, original HTTP 200 response and one reached
  pause evidence entry for each stage; integrity check returned `ok`.
- **PASS:** independent source review, release metadata/notice comparison and
  changed-source privacy scan. Private device captures and runtime files are not
  included in this source change or the synthetic fixture.

**NOT RUN locally:** interactive Windows/Linux, Intel macOS, new physical-device
acceptance, HTTP/3 or WebSocket frame capture. Hosted three-platform checks and
downloaded release-asset verification remain separate publication gates. Large
bodies still have the 16 MiB rejection limit; this change does not implement
streaming or claim archive-writer performance improvements. See
[rule behavior and downgrade precautions](RULES.md).

## Concurrent rule fixture follow-up — 2026-11-06

At `2ada212`, the [Windows job](https://github.com/dalkedev/scope/actions/runs/42281855023/job/128112730439)
passed GLib integrity and all eight attribution tests, then the combined
rule-ordering/concurrency fixture reported 29 of 32 client request timeouts.
Its shared helper specified five seconds; the failed rules binary completed in
15.15 seconds. This is observed test evidence, not measured per-request disk
latency. The original combined test passed locally before this change.

Source inspection shows six synchronous, mutex-held SQLite saves per successful
mock exchange: 192 autocommit statements for the 32-client burst. On the default
current-thread Tokio runtime these can delay network/timer progress. This is a
plausible causal explanation, not proof of Windows storage latency or a GLib
regression. Production persistence, SQLite settings and runtime timeouts are unchanged.

The fixture now separates `rules_run_before_breakpoints` (one exact exchange,
original/manual snapshots, response status/body and three-rule evidence) from
`concurrent_mock_requests_keep_all_evidence` (32 clients and 32 unique complete
mock exchanges, exact status/body and rule evidence). General clients retain
five seconds; only the stress client uses 30 seconds with a 60-second cooperative
batch deadline. Every spawned client is joined, including errors/panics. On a
batch timeout remaining tasks are explicitly aborted and joined, then the whole
test fails with elapsed/completed/joined/success counts. Synchronous work can
delay observing a Tokio timer; this is not a hard wall-clock guarantee for a
blocked disk. Clients are never retried or silently dropped.

Local checks: all ten real rule tests pass, including 32/32 successful joined
clients. Private controlled one-nanosecond client and batch budget probes both
fail as intended while joining 32/32 tasks; normal source was restored exactly.
These probes validate harness failure/cleanup paths, not Windows latency or a
reproduction of its filesystem conditions. The full locked headless workspace
suite passes 90 tests (41 core, 23 device, two MCP and 24 GLib). Formatting and
exact vendor/lockfile/notice preservation checks pass.

[Issue #28](https://github.com/dalkedev/scope/issues/28) records the separate
archive-writer responsiveness investigation, measured Windows/Linux acceptance
criteria and links to body-storage/backpressure work in #12. No production writer
redesign or performance improvement is claimed by this test-only maintenance fix.
New-head hosted Windows results remain separate PR check evidence; interactive
GTK/dialog acceptance is still outside these tests and #11 remains open.

## License attribution path follow-up — 2026-11-06

The GLib backport attribution now compares real filesystem manifest paths.
The previous lexical comparison lost the backport/provenance links when macOS
resolved a temporary-directory alias differently from Cargo's manifest path.
A real directory alias reproduced the loss locally before the fix; a missing
manifest also incorrectly claimed the backport. Both regressions now pass,
without modifying vendored source, copyright/license text or dependency versions.

Fixtures create existing manifest files and execute the real notice generator
in a child Node process. Only Cargo discovery data is stubbed using a test-only
Node preload, so the same tests run on Linux, macOS and Windows without Unix
shebang or Windows shell wrappers. The normal checks and release-build matrix
now run attribution regressions on all three platforms. Missing files return a
non-match; other filesystem errors remain visible instead of silently claiming
that source is verified.

Local Node 22.22.3: eight attribution tests and three release/resource tests pass;
TypeScript/Vite production build and public-resource verification pass. Actual
cargo-about 0.9.3 regeneration produces 226 Rust sections and seven npm notices,
byte-identical to the previous committed JSON/HTML. The unchanged lockfile hashes
match both generated inventories:

| Lockfile | SHA-256 |
| --- | --- |
| `Cargo.lock` | `6b8d93fb0ae484a4fbd6093f0055a7653881ed2a393076fed34b0905fcaa0479` |
| `package-lock.json` | `c5ffec0b4bcf200ad40550cfe7796c4f402b98ed5a7aaf59ee8183436e2fd07b` |

GLib retains its gtk-rs repository/authors plus upstream crate, modified-source
and provenance links. All 121 vendored files still pass the exact archive+patch
verifier and locked Cargo selection. A fresh independent review found no
production issue; its minor Windows JSON-escaping assertion gap was corrected.
Hosted platform attribution results are separate PR check evidence.

## GLib hosted acceptance follow-up — 2026-11-06

At `0799385`, [hosted run 42269942248](https://github.com/dalkedev/scope/actions/runs/42269942248)
passed Linux and macOS workspace/frontend/package/desktop-engine/MCP checks.
Windows failed in the integrity verifier with `Patched VariantStrIter source
checksum mismatch`, before its remaining tests could run. Those successful
platform results supersede the earlier local-host native prerequisite limit
for those hosted platforms; they do not prove interactive GTK/dialog acceptance.

The replay runs outside the repository's `.gitattributes`. An isolated inherited
global config with `core.autocrlf=true` and `core.eol=crlf` reproduced the same
checksum failure on Linux. The fix pins `core.autocrlf=false` and `core.eol=lf`
only on the replay command, without changing global config, vendor bytes, hashes
or byte comparisons. The regression verifies successful replay and that the
injected global config is unchanged. It runs in the existing three-platform CI
matrix, alongside the full tamper/selection suite.

The first follow-up head, `eec083e`, passed replay and exposed a second Windows
issue in [job 128100947923](https://github.com/dalkedev/scope/actions/runs/42278486459/job/128100947923):
Python tried to decode Cargo's UTF-8 metadata with CP1252. An actual locked Cargo
process reproduced `UnicodeDecodeError` locally when only the parent's default
encoding was forced to CP1252. Metadata capture now explicitly decodes UTF-8,
without changing JSON/source bytes. The additional regression uses the real
Cargo process and is skipped only if Cargo is not installed; normal platform CI
provides Cargo. Its RED/GREEN cycle is separate from the Git newline regression.

Current local follow-up checks:

| Status | Check | Result |
| --- | --- | --- |
| EXPECTED FAIL then PASS | Hostile inherited Git config regression | Original replay reproduces the exact checksum mismatch; command-local settings fix it. |
| PASS | Python integrity/selection tests | 17 cases, including inherited CRLF, legacy-codepage metadata, unchanged user config and previous tamper checks. |
| PASS | Full-tree provenance and locked Cargo selection | All 121 original files plus the exact upstream backport; same real version and lockfile. |
| PASS | Linux optimized GLib regression crate | 24 cases: nine iterator regressions plus two logging, nine translation and four ownership checks. |
| PASS | Locked workspace excluding desktop | 89 tests: 40 core, 23 device, 2 MCP and 24 GLib cases. |

The [CodeQL triage record](security/glib-0.18.5-codeql-triage.md) identifies all seven
exact baseline alerts, proposes dispositions with source/type/FFI/lifetime
reasoning, and documents concrete tests and their limits. The tests characterize
existing behavior; no pointer/logging production fix or vendor change is claimed.
All alert decisions remain with the independent operator. New-head hosted results
are separate PR check evidence and must not be inferred from the old-head passes.
Interactive GTK/dialog/installer/device acceptance remains untested; keep #11 open.

## GLib 0.18.5 backport — 2026-11-06

This narrow change is based on Scope 1.2.1 (`98ba0ef`) and preserves the
GTK 0.18 dependency family. [Backport provenance](../vendor/glib-provenance/README.md)
describes the exact upstream fix, checksum verification and removal conditions.
No application version or unrelated dependency version changes are included.

Local checks used Linux x86-64, Rust 1.99.1, Node 22.22.3 and the existing Zig
C compiler/linker. The iterator tests linked the actual installed GLib/GObject/GIO
2.88 runtime through local library search settings; no system packages were installed.

| Status | Check | Result / boundary |
| --- | --- | --- |
| PASS | `python3 scripts/verify-glib-backport.py --check-selection` | All 121 upstream crate files equal the verified original archive plus exact upstream patch; locked Cargo metadata selects only local GLib 0.18.5 at `vendor/glib`. |
| PASS | `python3 -m unittest discover -s scripts/tests` | 15 integrity/selection cases, including tampered inputs/source/licenses and registry/wrong-path/unused selection rejection. |
| PASS | `cargo test --locked --release -p scope-glib-variant-regression` | 9 optimized iterator tests. Seven independent content-reading cases segfaulted with pristine 0.18.5; all nine pass with the backport. |
| PASS | `cargo test --workspace --exclude scope-desktop --locked` | 74 tests: 40 core, 23 device fixtures, 2 MCP and 9 GLib iterator regressions. Real local HTTP/TLS tests and fake ADB are not interactive GUI/device acceptance. |
| PASS | `cargo fmt --all -- --check` | Vendored GLib remains excluded; original source bytes are preserved. |
| PASS | `npm test -- --run`; `npm run build` | 15 frontend tests and TypeScript/Vite production build. |
| PASS | `node --test scripts/licenses.test.mjs scripts/release.test.mjs` | 6 attribution tests and 3 release/resource tests. |
| PASS | `python3 -m unittest discover -s resources/device/tests` | 3 installer tests. |
| PASS | `npm test --prefix tools/har-compare` | 205 HAR tests. Contract lab also passed after its separate locked dependencies were installed. |
| PASS | `npm audit --audit-level=high` | Zero reported npm vulnerabilities. |
| PASS | Public resource staging/verification | 67 filtered files including the newly built MCP helper. |
| FAIL (environment) | `cargo test --workspace --locked` | `gdk-sys` cannot run `pkg-config` for `gdk-3.0 >= 3.22`; GTK development prerequisites are unavailable on this host. |
| FAIL (environment) | `npm run package` | Optimized locked `scope-mcp` build and frontend build pass; Tauri packaging fails at `glib-sys` because `pkg-config` / native desktop development metadata is missing. No installer produced. |
| NOT RUN | `node scripts/smoke.mjs` | Requires the desktop binary that this host cannot build; existing core/MCP tests are separate evidence. |
| NOT RUN | Interactive GTK/dialogs, installer launch, Android/device and other architectures | Headless/iterator tests do not establish these acceptance criteria. Issue #11 must remain open. |

The first local headless compile needed the existing Zig `CC`/`AR` selectors;
with those configured the complete available suite passed. Initial npm installation
used an unwritable default cache; rerunning with a local cache passed. An initial
contract-lab invocation lacked its separately locked XML dependency; after its
locked install the checks passed. These were environment/setup failures, not
suppressed test failures. Existing upstream GLib warnings are preserved.

The established `cargo-about` 0.9.3 generator regenerated 226 Rust license sections
and seven npm runtime notices. All original license texts are byte-identical; GLib
retains gtk-rs attribution and links the upstream crate and backport provenance.
Both recorded lockfile hashes match and the generated files contain no local paths.

Current `cargo audit` 0.22.2 returned exit 0 against database commit
`309392b18f6b17022f82c701f300433126901068`: zero vulnerability-list entries and
three unmaintained findings (`pin-utils`, `proc-macro-error`, `rustls-pemfile`).
Local path GLib is excluded by this tool's registry-source filter; its absence
is not security verification. See [DEPENDENCIES.md](DEPENDENCIES.md) for advisory
IDs and the backport's independent source/test evidence. No ignores are configured.

A fresh independent whole-change review found no substantive issues. Hosted
three-platform checks remain separate evidence; local results are not a claim
that hosted CI passed.

## 1.2.1 maintenance preparation — 2026-11-06

This candidate updates the macOS CI and release build host from `macos-14` to
`macos-15`. The old macOS job on main
[failed before executing steps](https://github.com/dalkedev/scope/actions/runs/42097291143)
with `The configuration 'macos-14-arm64-us-default' is not supported`.
Windows and Linux jobs from that run passed. This infrastructure failure does
not establish an application or test failure.

Application/npm/Cargo/Tauri versions are prepared as 1.2.1. External dependency
versions and application code are unchanged. The macOS runner change does not
change application deployment-target settings. Exact-head CI, native packaging and
final asset checks remain release gates; this preparation record does not claim
new interactive desktop/device validation. The macOS package remains ad-hoc
signed without Apple notarization. Windows/Linux interactive behavior remains
experimental, and Intel macOS is unverified.

## 1.2.0 release preparation — 2026-11-05

Local checks on the combined `release/1.2.0` candidate used macOS arm64,
Node 22.22.3 and Rust 1.99.1. Original heads from #3, #15, #22 and #23 are
preserved as merge parents. Application/npm/Cargo/Tauri versions agree on 1.2.0;
the private HAR tool/lab packages retain separate 0.1.0 identifiers.

- Rust formatting and all 70 workspace tests passed: 40 core, 23 device,
  five desktop and two MCP. Fixtures exercise real local HTTP/TLS and fake ADB;
  these tests do not establish a newly validated device/desktop interaction.
- TypeScript/Vite production build and all 15 frontend interaction tests passed.
- All 205 HAR tests, including composite entrypoint/integration-helper tests,
  passed locally. These launch the real bundled CLI in synthetic consumer
  subprocesses. Synthetic demo and sprint examples also passed.
- Three Node release/tag/resource tests and three Python installer tests passed.
  npm audit reported zero vulnerabilities, including development dependencies.
- Gitleaks 8.30.1 found no leaks in a clean snapshot of the candidate's tracked
  source, excluding installed dependencies and build products.
- Dependency notices regenerated as 226 Rust sections and seven npm runtime
  packages. Both committed-lock hashes, all local Rust package versions and the
  exact installed Lucide 0.577.0 license were checked; the JSON/HTML include its
  Feather-derived MIT text.

Fresh exact-head hosted platform, HAR/browser and composite-Action workflow
results remain separate release gates. Prior dependency PRs passed three-platform
build/package/engine-MCP checks; this does not establish the combined candidate.
No 1.2.0 native package, interactive desktop/device run, draft assets or publication
was verified by the local checks above. macOS ad-hoc signing is not Apple
notarization; Windows/Linux interactive behavior remains experimental and Intel
macOS is unverified. Existing dependency findings remain in
[DEPENDENCIES.md](DEPENDENCIES.md). Historical results below keep their original
version boundaries.

## 1.0.0 release preparation — 2026-11-02

These checks were recorded on `release/1.0.0` before publication, with source and
package version `1.0.0`. [GitHub Releases](https://github.com/SculptTechProject/scope/releases)
records published versions and downloadable artifacts. The earlier alpha evidence
is retained below as a separate historical baseline.

Checks run during candidate preparation:

- 52 Rust workspace tests passed: 22 core, 23 device, 5 desktop policy/redaction
  and 2 MCP transport tests. Rules use real HTTP/TLS fixtures; diagnostics and
  cleanup use fake ADB fixtures rather than a newly booted emulator.
- Regressions cover proxy Stop during a held replay, deletion/cancellation
  ownership, Scope session IDs versus payload credential names, valid OpenAPI
  schema fields and redacted-search membership isolation.
- 10 frontend interaction tests passed on Node 22.22.3; TypeScript/Vite built.
- Strict workspace Clippy, actionlint, three Node release/tag-resource tests and three
  Python installer tests passed. npm audit reports zero vulnerabilities.
- The macOS arm64 native package built and its complete ad-hoc signature verified.
- Cargo audit's vulnerability list has zero entries; the Linux glib unsoundness
  and two unmaintained-dependency warnings remain documented in DEPENDENCIES.md.

Cross-platform [CI run 41440372077](https://github.com/SculptTechProject/scope/actions/runs/41440372077)
passed all three jobs at source commit `6448b60`: Windows 2022 x64, Ubuntu 22.04
x64 and macOS 14 arm64. Each job ran the Rust/frontend/resource/installer tests,
native packaging and the real-process MCP/capture/persistence smoke. These are
build and headless-protocol checks, not interactive desktop/device validation on
Windows or Linux. The final PR head is checked separately by GitHub Actions.

### Local native candidate checks

- Fresh synthetic archive: real demo traffic visibly transitioned from in-flight
  to completed. Rename, notes, tags and pin persisted; active-capture deletion
  stayed disabled.
- Created, duplicated, reordered, toggled and deleted rules in the native UI.
  The enabled product mock returned actual HTTP 503 and the configured JSON to
  an independent proxied curl client; provenance and applied rule were visible.
- Composed a POST replay with a new URL/query, custom header and JSON body.
  The saved exchange showed status 200, the edited fields and normalized length.
- Activity filtering and no-device diagnostics worked; all MCP permissions were
  toggled and restored off. Capture stopped and native Quit released all three
  listeners. No emulator was started.
- Four new genuine native screenshots show traffic, rules, replay and analysis,
  with neither the upper recording bar nor mouse pointer. No generative editing
  or image retouching was used. See [gallery](SCREENSHOTS.md).
- After final packaging, `scripts/smoke.mjs` passed against executables INSIDE
  the public `.app`: twelve tools, OpenAPI, real capture, redaction, access gates,
  SQLite persistence, restart isolation and graceful shutdown. Final resources
  exclude optional native Frida files; the complete ad-hoc signature verifies.

No new physical Android or Windows/Linux desktop interaction is implied. Native
branch protection requires maintainer review before merge; the release workflow
builds an existing tag into a draft and does not publish automatically.

## Historical alpha baseline

Date: 2026-11-02. Host: macOS arm64. Application version 0.1.0; verification baseline for prerelease `v0.1.0-alpha.1`.

## Automated checks

- `cargo test --workspace --locked`: **16 passing tests**. Eight core tests (one unit and seven real-protocol integration/migration tests), seven Android adapter tests, one MCP credential-redaction test.
- `npm run build`: TypeScript and Vite production bundle passed.
- `npm run package`: native release application and separate MCP executable built; macOS `.app` produced.
- `scripts/smoke.mjs` ran successfully against release binaries: real MCP initialize/initialized, seven tool definitions, live local captures, analysis, authentication, sharing scope, default replay denial and credential redaction.
- That smoke test shut down and reopened the same SQLite archive: seven complete exchanges remained, shared-session access reset, and previously shared real session/exchange IDs were denied.
- Core tests include actual CA-signed HTTPS interception, both breakpoint directions, originals, binary data, concurrent pairing, replay, HAR/native roundtrips, 16 MiB rejection, timeout/abort/shutdown, legacy schema migration and persistent session revisions after completion.

## Native macOS interface

The packaged application was launched through its actual `.app` bundle.

- Generated six real requests through the proxy to the local fixture server.
- Selected captured exchanges and inspected actual response JSON.
- Quit/reopened the app: saved requests remained visible.
- Exported a HAR using the native Save dialog to `scope-native-verified.har`; parsed the resulting file and confirmed all 13 response bodies were present.
- Imported that file through the native picker: a separate 13-request session appeared.
- Opened Analysis and confirmed observed endpoint/error counts.
- Enabled a response breakpoint; a real curl request paused. Edited JSON in the native editor and continued. The waiting curl client received `{"editedInScope":true,"plan":"open-source"}`.
- This check exposed a stale-row issue when a response updated an existing exchange. The fix uses persistent session revisions plus pending transitions; integration assertions cover revision changes after request/response completion. A second native run then visibly changed the same 30th row from Paused to 200 / edited without adding a request; curl received `{"finalBuildVerified":true}`.
- macOS titlebar overlay was removed after it interfered with controls. Standard native titlebar controls and Save dialog were then verified.

## Real Android / Frida checks

See [Android test detail](ANDROID.md#recorded-local-smoke-test). One existing rooted Android 16 arm64 emulator and only the included `com.scope.demo` app were used.

- System proxy mode captured four real app GET/POST requests. Scope CA visibility was verified in the app namespace. Stop restored previous proxy and removed Scope-owned mappings/certificate.
- Frida 17.9.1 captured three additional requests while the global proxy remained disabled. Stop detached and stopped its owned server.
- The bundled standalone worker and automatic Android arm64 server selection were then tested with Python/Frida overrides unset: two more real app requests were captured, followed by verified cleanup.

## Boundaries

These checks establish a local functional alpha, not universal Android compatibility or production stability. Windows/Linux jobs are included but were not executed in this local run. Physical devices, long-duration sessions, WebSocket frames, HTTP3 and arbitrary native Android networking stacks were not verified. The macOS artifact is not Apple-notarized.

The standalone Frida worker/server checks above used a development bundle. The public `v0.1.0-alpha.1` package deliberately omits these optional native binaries because a complete artifact-specific Frida dependency notice inventory has not been established. Users can install the optional runtime locally; source integration and profiles remain included. See [Android setup](ANDROID.md#frida-runtime), [release procedure](RELEASING.md), and [third-party notices](../THIRD_PARTY_NOTICES.md).

Publication and CI status are recorded by [GitHub Releases](https://github.com/SculptTechProject/scope/releases) and [Actions](https://github.com/SculptTechProject/scope/actions), separately from the local checks above.

## Public alpha preparation

Additional checks on the public macOS arm64 bundle:

- All **16 Rust workspace tests**, strict workspace Clippy (`--all-targets --locked -- -D warnings`), TypeScript and production builds passed.
- **Two Node release-resource tests** and **three Python installer tests** passed. The installer rejects a corrupt server download before writing an executable.
- Actual bundled resources were inspected: optional Frida/Python native binaries, capture archives, private CA keys and MCP bridge tokens are absent. Scope's license, dependency notices and source setup helper are present.
- `codesign --verify --deep --strict` passed after the packaging helper signed the complete final bundle with a local **ad-hoc** signature. It is not Apple Developer signed/notarized.
- `scripts/smoke.mjs` ran against the desktop and MCP executables **inside that public app bundle**. Real capture reads, seven MCP tools, authentication, session access, credential redaction, default replay denial, shutdown and persisted archive/restart isolation passed.
- Gitleaks 8.30.1 scanned the complete committed source history with no detected secrets. Build products are excluded from the source archive.
- The alpha's four [screenshots](https://github.com/SculptTechProject/scope/blob/e2611d640e10fd2be0c97b0095fed6826ce0ddcb/docs/SCREENSHOTS.md) were actual native captures. In the final public alpha, a proxied curl response paused; the native editor changed HTTP 200 to 503 and supplied synthetic maintenance JSON. The waiting client received that exact body, the saved row became `503 / edited`, and Original retained status 200 and the original JSON. The current gallery shows the 1.0 candidate.
- A separate concurrency check kept a direct replay waiting at a controlled localhost fixture while another response passed through the proxy. Edit/Continue remained usable, the client received `{"demo":true,"concurrentBreakpointPassed":true}`, and replay finished after the fixture was released. This verifies the separation of replay state from other breakpoint actions. Replay itself sends directly and does not trigger proxy breakpoints.
- A replay selection race was corrected so an asynchronous result cannot select an old session's exchange after switching sessions. TypeScript/build and independent code review passed. The final native app was left with capture stopped and both breakpoints disabled.

See [release preparation](RELEASING.md) and [dependency findings](DEPENDENCIES.md) for artifact and platform boundaries.
