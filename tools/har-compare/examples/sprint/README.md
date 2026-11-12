# Static sprint demonstrations

Run thirteen deterministic examples of inventory, authored requirements, scoped
noise and invalid JSON from the repository root:

```sh
node tools/har-compare/examples/sprint/run.mjs
```

Use Node 26.13.1 or later; the prepared sprint runtime is
`/opt/scope-lab/toolchains/node-v26.13.1-linux-x64/bin/node`. No npm dependencies,
network access, browser or model are needed. The checked-in HARs are authored
synthetic data at `https://synthetic.invalid`, not real captured traffic. Timing
fields are fixed zeros. URLs are never requested or replayed.

The runner creates a fresh ignored `tools/har-compare/out/sprint-*` directory.
It executes every CLI scenario twice with separate new output paths, requires
byte-identical stdout/JSON/HTML/JUnit, and checks the literal expected counts,
finding kinds, JSON pointers, operation labels, evidence entries and exit codes
shown below. Golden files are read-only during an ordinary run. The final
`receipt.json` path is printed only after every check passes.

| Scenario | Exit | Literal expected result |
| --- | --- | --- |
| `inventory` | 0 | Three operations; two JSON selectors; `analysisPerformed: false`. |
| `unchanged-500-observed` | 0 | Zero changes in one paired response; no authored requirement. |
| `unchanged-500-asserted` | 1 | One `contract-status`, candidate status 500, one authored check. |
| `unchanged-500-explicitly-allowed` | 0 | Explicit `[500]` allowlist passes one authored check. |
| `missing-from-both-observed` | 0 | Zero changes; `/user/role` is absent in both captures. |
| `missing-from-both-required` | 1 | One `contract-json-missing` at `/user/role`, one authored check. |
| `missing-from-both-required-ids` | 1 | Same failure, check count and original entry evidence; captured/configuration labels omitted with `--labels ids`. |
| `ignored-parent-still-required` | 1 | Ignoring `/user` skips one comparison node; `/user/role` requirement still fails. |
| `timestamps-strict-control` | 1 | Two `value-changed` findings at `/timestamp`, for billing and health. |
| `timestamps-health-only` | 1 | Health timestamp ignored once; billing `/timestamp` still changes. |
| `timestamps-both-ignored` | 0 | Two explicitly scoped nodes ignored; zero changes. |
| `duplicate-json-rejected` | 2 | `JSON` error; no requested JSON/HTML/JUnit output file exists. |
| `lossy-json-rejected` | 2 | `NUMBER` error for a decimal that loses round-trip precision; no output files. |

The ordinary unchanged controls are essential: observed equality cannot establish
that a field is required or a server is healthy. The explicit requirements
express developer intent. Likewise the strict timestamp control proves that the
clean ignored result comes from the selected rules, rather than an algorithm
that ignores every JSON value.

Each selected mixed HAR has three original entries: HTML entry 1 is excluded;
health is entry 2; billing is entry 3. Both strict and scoped comparisons require
two selected entries and one excluded entry per side. Billing's remaining finding
must cite original entry 3, even though the HTML entry was not analyzed.

The inspect result is an inventory. Its successful exit is not a regression test.
Its exact suggested `configTemplate` contains `/billing` and `/health`; it does
not silently select the HTML document or analyze response bodies.

For a separate real Snurpo proxy → native HAR export → CLI proof using only
synthetic loopback HTTP, see the [optional native capture harness](../../toolchains/README.md#optional-native-capture-interoperability-proof).
Its evidence is distinct from these static fixtures.

## Run individual feature commands

All commands below print JSON to stdout. Exit 1 and exit 2 are intentional for
the corresponding examples; they are not shell failures to repair.

```sh
node tools/har-compare/cli.mjs inspect \
  --input tools/har-compare/examples/sprint/fixtures/mixed-baseline.har

# Unchanged HTTP 500 still fails the explicitly authored [200] requirement: exit 1.
node tools/har-compare/cli.mjs compare \
  --baseline tools/har-compare/examples/sprint/fixtures/unchanged-500.har \
  --candidate tools/har-compare/examples/sprint/fixtures/unchanged-500.har \
  --config tools/har-compare/examples/sprint/configs/status-200.json

# A field missing from both captures fails a requiredJson rule: exit 1.
node tools/har-compare/cli.mjs compare \
  --baseline tools/har-compare/examples/sprint/fixtures/missing-from-both.har \
  --candidate tools/har-compare/examples/sprint/fixtures/missing-from-both.har \
  --config tools/har-compare/examples/sprint/configs/required-role.json

# Health noise does not hide the billing timestamp change: exit 1.
node tools/har-compare/cli.mjs compare \
  --baseline tools/har-compare/examples/sprint/fixtures/mixed-baseline.har \
  --candidate tools/har-compare/examples/sprint/fixtures/mixed-timestamps.har \
  --config tools/har-compare/examples/sprint/configs/scoped-health.json

# Remove scoped noise rules with this strict control: two findings, exit 1.
node tools/har-compare/cli.mjs compare \
  --baseline tools/har-compare/examples/sprint/fixtures/mixed-baseline.har \
  --candidate tools/har-compare/examples/sprint/fixtures/mixed-timestamps.har \
  --config tools/har-compare/examples/sprint/configs/strict.json

# Reject duplicate decoded JSON names: exit 2.
node tools/har-compare/cli.mjs compare \
  --baseline tools/har-compare/examples/sprint/fixtures/missing-from-both.har \
  --candidate tools/har-compare/examples/sprint/fixtures/duplicate-json.har

# Reject silent decimal loss: exit 2.
node tools/har-compare/cli.mjs compare \
  --baseline tools/har-compare/examples/sprint/fixtures/number-baseline.har \
  --candidate tools/har-compare/examples/sprint/fixtures/lossy-json.har
```

Add `--json NEW_FILE --html NEW_FILE --junit NEW_FILE` to a comparison to save
reports. All paths must be new. The runner verifies that requesting all three
formats for invalid JSON leaves none of them behind. The error still appears as
JSON on stdout; `goldens/*.error.json` preserve that diagnostic, not a completed
analysis report.

## ID-only report labels

`missing-from-both-required-ids` repeats the required-field failure with
`--labels ids`. It still exits 1 with one authored check, one
`contract-json-missing` finding, stable report-local operation/finding IDs,
contract indices and original baseline/candidate entry 1. Its JSON includes
`privacy.labels: "ids"` and `config.redacted: true`. JSON, HTML and JUnit omit
the original method, origin, operation path, JSON pointer and raw configuration
labels. The runner asserts these properties before accepting its golden files.

```sh
node tools/har-compare/cli.mjs compare \
  --baseline tools/har-compare/examples/sprint/fixtures/missing-from-both.har \
  --candidate tools/har-compare/examples/sprint/fixtures/missing-from-both.har \
  --config tools/har-compare/examples/sprint/configs/required-role.json \
  --labels ids
```

The redacted report cannot reconstruct the original configuration. Preserve that
configuration and the source HAR privately to interpret entry references or rerun
the check. Counts, HTTP statuses, JSON types and capture provenance remain visible
when present; removing labels does not anonymize traffic patterns. The default
readable-label scenario remains separately checked with its original golden bytes.

## Golden artifacts and maintenance

`goldens/` includes inventory JSON, completed analysis JSON/HTML/JUnit, and the
two stdout error diagnostics. There are no invented HTML/JUnit success artifacts
for failed analysis. The runner checks that synthetic header, query and raw body
value markers never appear in output. Readable-label goldens retain synthetic
origin/path/field names as evidence; the ID-only goldens omit those labels.

A clean JUnit analysis has one `clean-analysis` testcase. Otherwise testcases
represent findings. This runner independently checks suite/testcase/failure
counts and expected evidence; full XML parser validation is a separate lab check.
HTML receives static script/CSP checks here. No screenshot, browser execution or
runtime CSP enforcement is claimed.

After reviewing an intentional report-format or semantics change, explicitly
regenerate goldens:

```sh
node tools/har-compare/examples/sprint/run.mjs --refresh
```

`--refresh` first executes the same independent semantic assertions and repeat
checks for every case, then updates only changed or new golden files.
Byte-identical existing goldens are left untouched, including their modification times. It does not rewrite inputs
or configs. Review `git diff` of the goldens, then rerun without `--refresh` to
verify the checked-in bytes. Existing generated run directories are preserved.
