# Snurpo HAR comparison local lab

An optional, isolated development lab for the dependency-free CLI in `../cli.mjs`.
It uses synthetic localhost traffic only. Installing this lab does not change the
root package, lockfile, application runtimes, active workflows, or product CLI.
The CLI itself requires no Playwright dependency and never replays captured URLs.

## Pinned tools and installation

Use the project's selected Node **22.22.3** runtime. The lab locks
**playwright 1.64.1** and **playwright-core 1.64.1**, with npm integrity digests in
`package-lock.json`. This version's official package declares Node `>=20`.
Playwright's own browser manifest pins Chromium headless shell
**156.0.8078.4 / revision 1248** and Firefox **157.0 / revision 1555**.
These are package manifest versions, not successful browser-launch evidence.
For independent JUnit XML validation, the lab additionally pins
`@xmldom/xmldom 0.9.12` (MIT, Node `>=14.6`, no runtime dependencies). It stays
inside this lab; the product CLI still uses only Node built-ins.

Select Node **22.22.3** on `PATH`. From the repository root, use these POSIX
shell commands to keep both caches in this lab's ignored output:

```sh
cd tools/har-compare/lab
export npm_config_cache="$PWD/output/npm-cache"
export PLAYWRIGHT_BROWSERS_PATH="$PWD/output/playwright-browsers"
npm ci --ignore-scripts --no-audit --no-fund
node node_modules/playwright/cli.js install chromium --only-shell
```

The cache locations are explicit and local to the checkout. An existing pinned
browser installation can be selected with its own `PLAYWRIGHT_BROWSERS_PATH`.
Browser download is an explicit development setup step, not an implicit product
behavior. Do not use `install-deps`, `--with-deps`, global npm
installation, an authenticated browser profile, or a sandbox-disabling flag.
Firefox can be installed explicitly with
`node node_modules/playwright/cli.js install firefox`.

Sources checked on **2026-11-03**:

- [Official tagged package metadata](https://github.com/microsoft/playwright/blob/v1.64.1/packages/playwright/package.json)
  identifies the version, exact core dependency, Node floor and Apache-2.0 license.
- [Official browser manifest](https://github.com/microsoft/playwright/blob/v1.64.1/packages/playwright-core/browsers.json)
  associates this package version with the browser revisions.
- [Official browser installation guidance](https://playwright.dev/docs/browsers)
  explains version-specific browsers and explicit cache selection.
- [Official BrowserType API](https://playwright.dev/docs/api/class-browsertype)
  documents launching independent browser processes; Chromium is launched with
  `chromiumSandbox: true` here.
- [Official BrowserContext API](https://playwright.dev/docs/api/class-browsercontext)
  documents isolated contexts, request interception and HAR recording.
- [Official xmldom 0.9.12 metadata](https://github.com/xmldom/xmldom/blob/0.9.12/package.json)
  identifies the independent XML parser pin, runtime floor and MIT license.
- [Official Node 22.22.3 archive](https://nodejs.org/en/download/archive/v22.22.3)
  identifies the existing project runtime. This lab does not replace it.

## One-command verification

After setup, with the browser cache variable above still set:

```sh
node verify.mjs
```

The `npm run verify` and `npm run doctor` commands invoke the same bounded check.
The command runs fixture integration tests, captures actual localhost HTTP
exchanges using Node, executes ten CLI controls, runs a bounded eight-seed
response corpus, eleven authored-contract controls, thirty-five JSON/scope/query-byte
boundary controls, eleven report-output controls and four hostile-payload
controls with independent JUnit parsing, then attempts browser security checks.
It prints the exact fresh `output/verify-*/receipt.json` destination.

Exit status:

| Exit | Meaning |
| --- | --- |
| `0` | Fixture tests, localhost controls and browser checks passed. |
| `1` | An integration check failed, or product source changed during the run. Inspect the receipt and TAP file. |
| `2` | Localhost checks passed; browser verification is blocked. No browser success is claimed. |

Each run creates a new directory and preserves earlier evidence. `output/` and
`node_modules/` are ignored by Git. Generated fixtures contain only local lab
traffic. Do not replace them with personal or authenticated captures.

The top-level receipt links the per-group receipts; `fixture-tests.tap` sits
beside `receipt.json`. The group `cases` counts cover distinct controls, excluding
inventory discovery, repeated analysis and positive probes. The current six groups total **77 CLI controls**
over **142 captured HTTP responses**; six additional child-process probes test
the replay detectors. Fixture integration tests make their own separate requests.
`browser.screenshots` stays empty after a failed launch. The selected product
source SHA-256 is evidence about the checked bytes; it is not a Git commit ID.

Within the exact `output/verify-*` directory printed by a run:

| Evidence | Relative path | Controls / captured responses |
| --- | --- | --- |
| Overall status and source hashes | `receipt.json` | Aggregate status; `partial`/exit `2` preserves passing localhost evidence |
| Fixture/process tests | `fixture-tests.tap` | 29 tests; their traffic is separate |
| Required demonstrations | `localhost-demo/node-demo-receipt.json` | 10 / 24 |
| Seeded repeated responses | `seeded/seeded-receipt.json` | 6 / 32 |
| Authored requirements | `contracts/contracts-receipt.json` | 11 / 16 |
| JSON, scope and query boundaries | `boundaries/boundary-receipt.json` | 35 / 66 |
| Exclusive report outputs | `artifacts/output-receipt.json` | 11 / 2 |
| Hostile field names | `payloads/payload-receipt.json` | 4 / 2 |

The browser result is recorded separately under `browser` in `receipt.json`.
A blocked browser is not an additional passing control and produces no screenshots.

Fresh runs select a new ephemeral origin, timestamp and output directory, so
capture files and full receipts intentionally differ across runs. Byte-identical
JSON is required when reanalyzing the same saved HAR/config files. Reproducibility
means the same declared checks and outcomes under the pinned runtime, rather
than equality of fresh network captures or browser screenshots.

The receipt hashes product `.mjs` modules before and after verification, including
`cli.mjs` and modules under `src/`, `contracts/`, `json/` and `formats/`. It records
relative filenames, byte sizes and SHA-256 values; no source contents are copied
into the manifest. Tests, hidden files, dependencies, memory, Git and generated
outputs are excluded. Symlinked sources are rejected. A changed source snapshot
makes an otherwise successful run `inconclusive` with exit `1`. This detects
ordinary concurrent editing; it is not a filesystem attack detector or a hash of
the entire repository. Snapshots are bounded to 128 modules, 1 MiB per module,
8 MiB total and 64 source directories within eight nested levels.

## Runtime child environment

The runtime launchers explicitly pass only `PATH`, `HOME`, `LANG`, `LC_ALL`,
`LC_CTYPE`, `TMPDIR`, `TMP`, `TEMP` and `PLAYWRIGHT_BROWSERS_PATH` when present.
The existing `HOME` is preserved so normal browser executable/cache lookup can
work; it is never repurposed and no authenticated profile is attached.
CLI children, synchronous fixture/report checks and browser launches share this
allowlist. Caller-provided child environments are filtered too. Owner auth,
proxy credentials, `NODE_OPTIONS`, extra CA injection and arbitrary private
variables are excluded. The product CLI needs no owner credentials or proxy.
Explicit npm/browser download setup is separate from these runtime launches.

Two actual-child tests exercise both supplied and inherited environments. They
use synthetic private sentinels plus an invalid `NODE_OPTIONS` flag, require the
children to start normally, check that private names are absent, and confirm
selected public runtime paths survive. Neither test dumps an environment or
uses real credential values. This controls inheritance, not what trusted child
code could read from the filesystem.

## Localhost traffic demonstration

This path works without a browser installation:

```sh
node run-node-demo.mjs
```

It starts a disposable server bound to `127.0.0.1` on an ephemeral port and records
four actual HTTP responses for each of six scenarios. The server remains alive
and responsive during all CLI checks. The script then stops it, repeats a saved
HAR analysis offline, and writes HAR/config/JSON/HTML files plus
`node-demo-receipt.json` under `output/node-demo-*`.
The creator explicitly says **Node lab, not browser capture**. Node does not
execute the HTML page or discover its assets; the recorder requests the known
synthetic document, CSS, SVG and API routes. Its HAR timing fields are synthetic
zeros and must not be used as performance measurements.

All scenarios keep the API method, origin, path and query identical. A synthetic
request header selects the response variant, so matching does not accidentally
turn a response change into a changed operation URL.

| Control | Expected CLI exit | Required evidence |
| --- | --- | --- |
| Missing role | `1` | `field-removed`, `/user/role`, original HAR entry 4 on both sides |
| HTTP 500 | `1` | `http-failure`, independently unchanged JSON shape |
| Timestamp, strict | `1` | `value-changed`, `/timestamp` |
| Same timestamp change, ignored | `0` | Explicit `/timestamp` ignore in strict mode |
| Unchanged selected API | `0` | One selected and three excluded entries per HAR |
| Mixed capture without selection | `2` | `BODY`: HTML/CSS/SVG are not silently parsed as JSON |
| Wrong API selector | `2` | `SELECTION_UNUSED`, rather than an empty success |
| Deliberately omitted captured JSON body | `2` | `BODY`; exporter-gap negative control derived from actual traffic |
| Selected API returns real HTML 500 | `2` | `BODY`; selected HTML is not silently excluded |
| Selected API returns real empty JSON | `2` | `JSON`; captured empty JSON is malformed, distinct from an unavailable body |

The script runs `inspect` against the actual mixed HAR and uses its
`configTemplate` as the selection for `compare`. It reruns each successful
analysis to require byte-identical JSON from identical input/configuration. An
independent child-process HTTP probe must increment the live fixture request
counter exactly once before its snapshot. CLI processes run asynchronously so
the server remains responsive; the counter must stay unchanged through all
`inspect` and `compare` calls. After the server stops, the saved unchanged HAR
comparison must still produce identical JSON. This is a destination-specific
replay control, not an operating-system network audit.

The generated configs contain the current ephemeral origin. To reproduce a
single CLI analysis against saved files, use that directory's generated config:

```sh
node ../cli.mjs compare \
  --baseline output/node-demo-REPLACE/baseline.har \
  --candidate output/node-demo-REPLACE/missing-field.har \
  --config output/node-demo-REPLACE/missing-field.config.json
```

`REPLACE` is a placeholder: use the exact directory printed by your run. Saved
HAR analysis remains offline after the fixture server stops.

## Bounded generated response corpus

```sh
node run-seeded-checks.mjs
```

This records **32 real HTTP responses**: eight fixed seeds with two occurrences
per seed in both captures. It is a bounded generated corpus, not random fuzzing.
The second candidate occurrence cycles through unchanged, field removal, type
change and value change. Field names include a slash, tilde, Unicode, emoji,
HTML-looking text, `__proto__` and `constructor`.

The candidate reverses ordinary query parameter order. Eight query variants must
remain eight comparison operations with all sixteen responses correctly paired;
one unchanged response must not hide a changed repeated occurrence. The server
also varies JSON vendor MIME types and synthetic timestamp noise. HAR metadata
contains unrelated bracket/quote strings, and actual synthetic authorization and
query sentinels must not appear in reports.

The script uses `inspect` to create the exact selector and checks three original
controls: shape plus noise ignore has **4** regression findings; strict plus
noise ignore has **6**; strict without the ignore has **22**, including sixteen
noise changes. Expected escaped JSON pointers and original occurrence numbers
are checked directly. Three further controls repeat shape/strict analysis with
an operation-scoped `/noise` ignore across every query variant and occurrence,
then combine a global noise ignore with a disjoint scoped `/record` subtree
ignore. The latter has zero regressions, with sixteen global and sixteen scoped
skips. A separate child HTTP probe proves the live no-replay
counter; all CLI calls must add zero requests. Each run writes its exact
`output/seeded-*/seeded-receipt.json` path. Static escaped-HTML assertions here
are separate from the blocked browser checks.

## Authored contract and JUnit integration

```sh
node run-contract-checks.mjs
```

This records sixteen actual localhost exchanges and runs eleven controls across
JSON, HTML and JUnit outputs:

- An explicit status/required-string contract passes with two executed checks.
- Unchanged HTTP 500 and a field missing from both captures each pass ordinary
  observation comparison, then fail an explicit authored requirement.
- A JSON ignore cannot hide an authored requirement failure. A strict timestamp
  ignore can coexist with passing independent status/type requirements.
- Allowing HTTP 500 in a contract preserves the ordinary 200→500 regression;
  disallowing it reports both ordinary and authored failures.
- A misspelled contract selector and a rule targeting an excluded CSS response
  both fail `CONTRACT_UNUSED` with exit `2`.

Original HAR evidence remains entry 4. A live child-process probe proves the
request counter; contract evaluation must add no HTTP requests. Raw synthetic
response values must not appear in any report.

The base demo, seeded corpus and authored-contract checks request real CLI
`--junit` output. The independent pinned XML parser reads every generated XML
file and verifies root/suite counts, testcase/failure/skipped nodes, finding
identity and decoded JSON pointers against the JSON analysis. Its own negative
controls reject malformed escaping, incorrect counts, injected markup and DTDs.
The seeded HTML-looking field name must remain XML text. This checks the
project's JUnit mapping, not every consumer's JUnit dialect or a full XML schema.
A clean comparison has one successful `clean-analysis` testcase; JUnit testcases
represent findings, rather than requests or the number of authored checks.

## JSON interpretation and operation-scope boundaries

```sh
node run-boundary-checks.mjs
```

This records sixty-six actual HTTP responses and runs thirty-five CLI controls. The
fixture serves deliberately raw JSON text; parsing and reconstructing it in the
recorder would erase duplicate members and silently round lossy numeric tokens.
The selected response parser must reject duplicate, escaped-duplicate and nested
duplicate member names, malformed strings, lossy decimals, underflow and
overflow. Invalid UTF-8 is preserved as canonical HAR base64 and must fail
`ENCODING`, rather than turn into a valid replacement character. Equivalent
decimal spellings and signed zero remain accepted; an
ordinary decimal value change still produces a strict-mode regression. One UTF-8
BOM is accepted before JSON; a second BOM fails `JSON`. The recorder uses actual
response bytes, retaining both BOMs and their byte sizes rather than allowing
`Response.text()` to silently strip one.

Ten query controls send bounded percent-encoded request targets through the
server and require its observed raw targets to match exactly. Hexadecimal case,
valid Unicode percent encodings, `+` versus `%20`, a literal percent versus `%25`,
and ordinary parameter order are equivalent. Invalid `%FF` versus a replacement
character or `%FE`, BOMs in parameter names/values, and reversed duplicate
parameter order remain distinct operations. Reports must omit those query bytes
and the synthetic authorization/body sentinels.

Two distinct API routes expose the same top-level and nested timestamp pointers.
Strict mode reports four changes. Ignoring only the health route leaves both
billing changes visible; ignoring both routes yields a clean comparison. Further
controls require unused/excluded operation rules and changed parents to fail
closed, reject overlapping global/scoped pointers, and preserve HTTP failures
and explicit required-field assertions despite noise ignores.

All invalid controls explicitly request JSON, HTML and JUnit output and must
leave no success artifacts. Every completed analysis is validated with the independent XML parser, checked
for synthetic header/query/body value leakage and repeated for identical JSON.
A positive child HTTP probe establishes the fixture counter; all CLI calls must
add zero requests. Five clean, regression and invalid-input comparisons repeat
after the server is closed and must retain identical stdout and exit status.
The run preserves its exact `output/boundary-*/boundary-receipt.json` destination.

## Report-output boundaries

```sh
node run-output-checks.mjs
```

Eleven process controls use two saved real HTTP responses to check exclusive
output creation. Existing JSON, HTML and XML artifacts must survive unchanged;
if a later output collides, earlier newly created reports must be rolled back.
The CLI must reject output aliases, input HAR/config paths, symlinks and
directories. Passing and failing comparisons create exact stdout JSON plus
independently parsed JUnit, with `0600` report permissions and newly created parent
directories. A live positive probe and stable counter verify no requests reach
the fixture during these file-output checks. This checks ordinary local file
operations in a trusted temporary directory, not concurrent filesystem attacks.
The run writes `output/artifacts-*/output-receipt.json`.

## Hostile JSON field gallery

```sh
node run-payload-checks.mjs
```

Two actual HTTP captures contain twelve fixed field names: script/element/CDATA
closing strings, handler-looking text, XML metacharacters, entity-looking text,
null/control characters, a lone surrogate, valid Unicode, a carriage return and
`__proto__`. Four CLI controls check removal, addition, unchanged input and
explicit required fields combined with JSON noise ignores. All twelve additions
must appear as informational findings and skipped JUnit testcases; removals and
authored missing fields must produce twelve failures.

Independent XML parsing checks counts, structure, permitted attributes, decoded
pointers, no DTD/stylesheet instructions and no synthetic header/query/body
values. The oracle follows the [XML 1.0 character ranges](https://www.w3.org/TR/xml/#charsets)
and [line-ending normalization](https://www.w3.org/TR/xml/#sec-line-ends): invalid
characters replaced by the writer are compared as U+FFFD, and carriage returns
are normalized by the XML parser. Only xmldom's explicit warning about a legal
replacement character is allowed; other parser warnings/errors still fail.
Static HTML checks reject executable/resource markup, and repeated CLI analysis
must return identical JSON. These are text/DOM/XML semantics, not browser
execution or perceptual layout verification. The exact receipt is written to
`output/payloads-*/payload-receipt.json`.

## Browser checks and browser HAR capture

```sh
node browser-check.mjs
node capture-demo.mjs
```

Pass `firefox` as the final argument to select that explicitly installed browser.
The default Chromium launch requires its sandbox. Neither command connects to an
existing profile or browser service. Every context is disposable; request routes
block all destinations except the exact local document or fixture origin.

`browser-check.mjs` generates a report from malicious **synthetic field names**
containing script-closing, script, image handler and iframe payloads. It checks:

- An intentionally unsafe control document executes the sentinel and attempts a
  blocked localhost network request, proving both detectors are active.
- The generated report contains the hostile input as text, has no executable or
  resource nodes/attributes, executes no sentinel, attempts no intercepted
  request, and opens no dialog or popup.
- Report navigation permits only local hash anchors with one matching target.
  Remote, script/data/file URLs, non-anchor hrefs and duplicate targets fail.
- Header/query/body value sentinels are absent, report CSS is applied under its
  hash-based CSP, and native details expand on click.
- Successful runs produce real full-page screenshots of collapsed and expanded
  reports, and record the launched browser's actual version.

The unsafe detector control cannot contact its target: all requests other than
its initial file document are intercepted and aborted. No real service or secret
is used. Browser request interception observes document requests, not every
browser-process background activity; the result is deliberately scoped that way.

`capture-demo.mjs` executes the HTML/CSS/SVG/API fixture in a real browser and
records full HAR bodies with Playwright. It writes one HAR and screenshot per
scenario plus an exact API selector. This path is distinct from the Node
recorder; a failed launch never produces pretend browser captures/screenshots.

## Observed host limitation

On **2026-11-03**, the final integration verification passed twenty-nine lab tests and all
seventy-seven CLI controls over 142 actual HTTP responses. Product-source hashes
matched before and after that run. Sandbox-enabled Chromium
could not launch: **No usable sandbox**.
The normal Firefox launch reported **CanCreateUserNamespace / CLONE_NEWPID:
EPERM**, followed by an unavailable profile error. Relocating disposable temp
storage into the task directory did not resolve that Firefox failure.

No host security settings, namespace policy, browser sandbox, credentials or
approval settings were changed. No browser test, screenshot inspection, CSP
runtime verification or real-browser HAR capture passed on this host. The
verification receipt reports `partial` and exit `2`; browser checks remain
runnable on a compatible host. Existing jsdom checks are separate evidence.

## Test strategy and limits

| Boundary | Check | Coverage target |
| --- | --- | --- |
| Fixture behavior | Node integration tests against a real loopback server | Every scenario and invalid scenario |
| HTTP body capture | Real Node HTTP responses to HAR | JSON, HTML, CSS and SVG; original response bytes |
| CLI contract | Ten process-level controls plus inspect → config → compare | Findings, exits, scope counts, repeatability, no fixture replay |
| Generated corpus | Fixed eight-seed real HTTP sequence | Query order, repeated occurrences, escaped fields, shape/strict/ignore controls |
| Authored requirements | Eleven localhost CLI controls | Explicit pass, unchanged failures, ignore independence, selected scope |
| JSON/scope boundaries | Thirty-five controls over sixty-six real responses | Raw duplicate/lossy JSON, UTF-8/BOM bytes, query identity, exact operation isolation, unused rules, status and contract independence |
| Report-output boundaries | Eleven controls over two real responses | Exclusive output, collision rollback, input/symlink preservation, private file modes |
| Hostile fields | Four controls over two real responses with twelve field names | XML normalization and escaping, informational/skipped mapping, authored requirements |
| Receipt provenance | Before/after product module hashes | Stable source state, changed-source inconclusive result, bounded source allowlist |
| JUnit | Independent pinned XML parser and detector controls | Well-formed XML, report counts, escaped field paths, no value sentinels |
| HTML security | Real browser with unsafe detector control | Text escaping, request interception, CSP CSS, interaction |
| Browser capture | Playwright full HAR | Mixed page assets and selected API on a compatible host |

This lab does not prove native Snurpo interception, full HAR standards
conformance, OS-wide network silence, browser coverage across operating systems,
performance regression detection, or release readiness. The core CLI tests cover
parser/config/resource limits separately; run them with `node --test
../test/*.test.mjs`. No active GitHub workflow, remote CI run, production service
or public post is created by the lab.
