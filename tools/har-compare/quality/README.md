# Bounded mutation checks

This optional dependency-free quality harness asks whether existing process-level tests detect twenty specific behavioral breaks. It never changes the working tree or Git state. It copies only allowlisted product source/test/fixture files into a fresh directory outside `tools/har-compare`, applies one mutation per copy, and preserves evidence.

From the repository root, using the pinned runtime:

```sh
/opt/scope-lab/toolchains/node-v26.13.1-linux-x64/bin/node \
  tools/har-compare/quality/mutation-check.mjs \
  --output /tmp/scope-mutation-run-NEW
```

The output directory must not exist. Omit `--output` for a unique temporary directory. Select one fixed mutation with `--only disable-http-status`. The selection catalog is `scope-fixed-twenty-v5`, with no random seed or random mutation search. Historical eighteen-mutant receipts describe the earlier catalog and their own recorded source snapshot.

| Mutation ID | Deliberate break | Existing blackbox test |
| --- | --- | --- |
| `disable-http-status` | Omit HTTP status comparison. | HTTP 200 to 500 must fail even with JSON subtree ignored. |
| `collapse-occurrences` | Inspect JSON only for the first repeated response. | Losing a field in occurrence two cannot hide behind another full response. |
| `ignore-all-values` | Disable all strict scalar value comparisons. | Timestamp ignore's strict value-aware negative control must fail. |
| `skip-selected-body-validation` | Replace selected body parsing with an empty body. | Missing selected JSON capture must fail closed. |
| `disable-duplicate-check` | Disable duplicate decoded JSON name detection. | Ambiguous selected JSON bodies must fail, while independent numeric controls remain intact. |
| `disable-contract-status` | Disable authored status allowlist enforcement. | Unchanged HTTP 500 must violate an explicit `[200]` contract. |
| `widen-scoped-ignore` | Apply one endpoint's ignore to every endpoint. | Health timestamp noise must not hide billing changes. |
| `disable-unused-scoped-check` | Accept a pointer absent from its own operation. | A billing match cannot validate the health rule. |
| `disable-contract-json-missing` | Remove missing-required-pointer detection. | A field absent from both observations must have a missing-contract finding. |
| `disable-number-roundtrip` | Accept silently rounded decimal tokens. | Selected JSON that loses decimal information must fail analysis. |
| `disable-inspect-config-budget` | Advertise an oversized selection template. | Inspect must decline templates the CLI cannot read. |
| `disable-junit-failure` | Omit XML failure elements. | A regression retains both exit 1 and XML failure evidence. |
| `disable-junit-escaping` | Emit XML metacharacters without escaping. | Hostile labels remain data, without injected elements. |
| `lossy-query-identity` | Restore replacement-based URLSearchParams matching. | Distinct non-UTF8 octets must remain separate operations. |
| `disable-url-surrogate-check` | Permit lossy raw URL surrogate conversion. | Unpaired URL surrogates must fail before normalization. |
| `strip-query-name-bom` | Strip a leading BOM from decoded name ordering. | A BOM-prefixed query name remains distinct when pairs reorder. |
| `strip-body-bom-bytes` | Strip transport BOM bytes before bodyless-response validation. | BOM bytes cannot become an empty HEAD/204 body. |
| `reject-supported-document-bom` | Remove the shared single-document-BOM policy. | Identical plain/base64 BOM-prefixed JSON must both remain supported. |
| `ids-leak-operation-labels` | Copy captured method/origin/path into minimized output. | Opt-in IDs output must omit planted operation labels across JSON/HTML/JUnit. |
| `ids-leak-configuration` | Copy original selector/pointer configuration into minimized output. | The redacted config must not retain private contract labels. |

The repeated-response mutant models the masking effect of collapsing per-response evidence; it does not implement a complete alternative union-schema algorithm. These are focused checks, not an exhaustive mutation score or proof that every potential regression is tested.

## Evidence and classification

Each mutation first runs its unchanged targeted test subset. A green control must actually select named tests: Node can otherwise report a passing file wrapper when no test name matches. The harness checks matching TAP subtest identities as well as counters. Then exactly one source anchor must match, the mutated module must pass `node --check`, and the same targeted test subset runs again.

Results are reported separately:

- `killed`: selected tests fail with an assertion after a green unchanged control.
- `survived`: selected tests still pass after the behavioral change.
- `timeout`: the mutated test process group exceeded its deadline. This is not counted as a kill.
- `control-failed`: unchanged controls fail, time out, or select no actual tests.
- `invalid-mutant` / `invalid-run`: anchors or syntax do not match expectations, or execution cannot establish a meaningful test result.

The report includes SHA-256 hashes and byte sizes for frozen source files, targeted test names, counts/statuses, and execution durations. Every mutation copy contains `control.tap`, `syntax.tap`, and, when reached, `mutated.tap`. `receipt.json` records kills, survivors and inconclusive outcomes without silently treating tooling errors as coverage. Re-run with a new directory after source changes; old receipts describe only their recorded source snapshot. The source manifest is hashed before and after a run; `sourceUnchanged` records whether the allowlisted live product files match at the initial snapshot and final read. It does not observe transient concurrent changes between those reads. A false value can reflect concurrent edits by another worker and is not silently reported as source preservation. Mutants always run against the frozen initial snapshot.

CLI exit `0` means every selected mutant was killed, `1` means at least one survived or was inconclusive, and `2` means the harness could not set up a complete run. These are quality-tool exits; product CLI regression semantics remain independent.

## Scope and bounds

Only `cli.mjs` plus `.mjs`, `.json`, and `.har` files under `src/`, `contracts/`, `json/`, `formats/`, `test/`, and `examples/fixtures/` are copied. Hidden directories/files and `node_modules` are excluded. Symlinks are rejected, including allowlisted root directory symlinks; regular source files are opened without following links. No `.git`, credentials, environment files, browser profiles, installed dependencies or arbitrary repository attachments are copied.

Snapshots are bounded to 256 files, 1 MiB per file, and 32 MiB total. A run selects at most 20 explicit mutations, with a default 15-second deadline per child and 1 MiB combined child output. Child tests inherit only a fixed system executable path, their temporary directory and UTF-8 locale; parent authentication/environment values are not forwarded. The harness terminates its own Unix child process group on timeout or output overflow, including test workers and CLI children. It is intended for the project's Linux/Node environment.

This runs authorized local source/tests. It does not sandbox malicious test code, instrument all network activity, contact a CI provider, execute a remote workflow, replay HAR traffic, or publish artifacts. Existing targeted tests use synthetic data and child product CLI processes.

## Harness tests

```sh
/opt/scope-lab/toolchains/node-v26.13.1-linux-x64/bin/node --test \
  tools/har-compare/quality/*.test.mjs
```

Thirteen tests use real subprocesses and small synthetic source fixtures to establish actual kills, survivors, red controls, no-test filters, missing anchors, syntax errors, timeouts, environment exclusion, source preservation, detected concurrent changes and snapshot exclusions. They require no root dependencies or network access.

## Independent query corpus

`query-corpus.mjs` exercises 39 fixed synthetic query pairs through both product CLI commands. It checks lossless non-UTF8 name/value matching, UTF-8 aliases, form spaces and plus signs, duplicate order, one decoding pass, malformed percent literals, Unicode/BOM distinctions and rejected raw URL surrogates. Each case compares exit status and inventory variant count; synthetic header/body sentinel values must remain absent from both outputs.

```sh
/opt/scope-lab/toolchains/node-v26.13.1-linux-x64/bin/node \
  tools/har-compare/quality/query-corpus.mjs \
  --output /tmp/scope-query-corpus-NEW
```

An optional `--source DIRECTORY` runs a frozen product snapshot. The new output directory contains only synthetic HAR/config inputs, CLI JSON outputs and a receipt with source hashes, expected/observed results and failure counts. Children receive a minimal environment, a five-second timeout and 1 MiB output cap. Source modules are hashed before and after; a matching digest establishes equality at those reads, not an absence of transient concurrent edits. The corpus does not snapshot arbitrary repository files or represent a general URL-standard conformance suite.

CLI exits are `0` for all fixed cases passing, `1` for an observed mismatch and `2` for harness setup failure. A pre-fix frozen snapshot produced explicit red controls for binary-query collisions; keep historical receipts separate from current-source results.

## Resource boundary controls

`resource-check.mjs` runs eleven fixed synthetic controls around the documented URL, entry, selected-node, file-size and body-depth limits. Three controls combine 2,000 entries with 4,096-code-unit URLs, including non-UTF8 query bytes. Each input is compared with itself; accepted controls must return valid JSON, exit 0 and zero regressions, and rejected controls must return the expected analysis error and exit 2.

```sh
/opt/scope-lab/toolchains/node-v26.13.1-linux-x64/bin/node \
  tools/har-compare/quality/resource-check.mjs \
  --output /tmp/scope-resource-controls-NEW
```

The optional `--source DIRECTORY` chooses the runtime source. A new output directory contains a frozen allowlist of runtime modules, synthetic HAR inputs, raw CLI results, resource measurements, per-case `progress.json`, and a final source-hashed `receipt.json`. Tests, dependencies, hidden files, environment files and Git data are not copied. Source files must be regular files of at most 1 MiB, with at most 64 runtime modules; source symlinks are rejected. The source is read again after the run to report equality at those two observations.

Children use the frozen source, a minimal executable-path/temporary-directory/locale environment, a 256 MiB V8 old-space cap, a 30-second deadline and a 1 MiB combined output cap. The harness terminates its child process group on timeout or output overflow. `/usr/bin/time` reports maximum RSS in KiB; the V8 old-space cap is **not** an RSS limit. These local controls do not establish a general performance guarantee, a malicious-code sandbox, or a strict process-memory limit. Their measurements depend on the host and runtime. No traffic is replayed and no remote service is contacted.

CLI exits are `0` when all eleven controls match, `1` for mismatches/timeouts, and `2` for setup failure. If interrupted, preserve completed outputs and `progress.json`; absence of a final receipt is incomplete evidence, not an assertion that the product failed.
