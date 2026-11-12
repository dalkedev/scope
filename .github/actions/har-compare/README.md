# Snurpo offline HAR comparison Action

Compare two local HAR captures without replaying traffic, opening the desktop,
calling a model, or sending captures to a service. This composite Action bundles
Snurpo's existing [offline comparison CLI](../../../tools/har-compare/README.md)
and generates JSON, standalone offline HTML, and JUnit XML. Observed changes
are regression candidates; [authored contracts](../../../tools/har-compare/contracts/README.md)
express explicit required statuses and JSON fields.

Requires **Node.js 22.22.3 or newer and Bash**, supplied by the caller. Linux is
the supported runner for this initial wrapper; macOS and Windows runner execution
have not been validated. No npm install, token, extra permission, or dependency is
needed by the Action. The repository's [AGPL-3.0-or-later license](../../../LICENSE)
applies.

## Use from another repository

Check out your captures/config and install Node, then invoke the Action:

```yaml
permissions:
  contents: read
jobs:
  compare:
    runs-on: ubuntu-22.04
    steps:
      - uses: actions/checkout@3d3c42e5aac5ba805825da76410c181273ba90b1 # v7.0.1
        with:
          persist-credentials: false
      - uses: actions/setup-node@949feb2413d6458794dcd2491c4babbbce0c15c1 # v7.1.0
        with:
          node-version: '22.22.3'
      - uses: dalkedev/snurpo/.github/actions/har-compare@v1.2.0
        id: compare
        with:
          baseline: private-captures/baseline.har
          current: private-captures/current.har
          config: private-captures/contract.json
          output: scope-reports
```

The example targets the published `v1.2.0` release tag.
**Pin an inspected full commit SHA before using it with
sensitive captures.** Scope v1.1.0 predates the Action; do not use that tag as its
ref. Captures in the snippet must already exist
on your runner, from your own approved capture step or private storage. Do not
commit real captures merely to use this example. On a self-hosted runner, choose a
fresh output directory for every invocation and manage local retention yourself.

For a complete example that needs no real captures, copy
[`github-actions-composite.yml`](../../../tools/har-compare/examples/github-actions-composite.yml)
into your repository's `.github/workflows/` and run it manually. It checks out
Snurpo into a separate directory, uses synthetic fixtures, and runs the local
composite Action from that checkout. Upload is **off by default** and is a caller
choice. This example is inactive here; copying/enabling it is your decision.

The Action resolves its CLI relative to its **own checkout**, even when called
with remote `uses:` syntax. It never executes a similarly named CLI from your
repository. GitHub downloads the Action and setup steps may access the network;
the comparison itself is offline.

## Inputs

| Input | Required / default | Meaning |
| --- | --- | --- |
| `baseline` | Required | Baseline HAR file. |
| `current` | Required | Current HAR file; maps to CLI `--candidate`. |
| `config` | Optional, empty | Existing CLI JSON config; empty uses CLI defaults. |
| `output` | Required | **New directory**, not an existing file or directory. Fixed report names below. |
| `labels` | `ids` | `ids` omits captured labels; `readable` explicitly retains them in reports. |
| `fail-on-result` | `'true'` | Preserve 0/1/2 process status. `'false'` defers gating to the caller. |

All file/directory paths may be absolute; relative paths resolve from
`GITHUB_WORKSPACE`, regardless of shell `working-directory`. Paths support spaces,
quotes, equals signs, dollar substitutions, backticks and other shell
metacharacters literally. Control characters (including CR/LF/NUL) and paths over
4096 code units are rejected. Use a trusted output parent directory; parent
symlinks and concurrent hostile filesystem changes are outside the protection
boundary. No user input is interpolated into executable shell source. The Node
entrypoint invokes its bundled CLI with an argument array and `shell: false`.

The output directory is exclusively created, preventing stale reports from being
mistaken for this invocation's results. Invalid analysis may leave an empty
directory; use a new path after correcting the inputs. The CLI also refuses to
overwrite files. Reports are `analysis.json`, `report.html`, and `results.xml`,
created with mode 0600 where POSIX permissions apply (directory mode 0700).

## Results and failure behavior

| `exit-code` | `result` | Default Action behavior |
| --- | --- | --- |
| `0` | `clean` | Succeeds: no configured regression candidates; informational changes may exist. |
| `1` | `regression` | Fails: complete reports remain available. |
| `2` | `invalid` | Fails: invalid config/input/output, incomplete analysis, or wrapper execution failure. No complete-report outputs. |

GitHub shows a failed step for either nonzero status; use `exit-code` to distinguish
the CLI meanings. Scalar outputs are written before process exit. If your caller
needs to consume outputs reliably on a non-clean comparison, use the explicit
deferred mode below, so the comparison step succeeds before your final gate.

| Other output | Meaning |
| --- | --- |
| `regressions`, `informational` | Integer counts for complete results; empty for invalid analysis. |
| `json-report`, `html-report`, `junit-report` | Absolute local file paths for complete results; empty for invalid analysis. |
| `error-code` | CLI error code (e.g. `JSON`, `BODY`, `CONFIG`, `INPUT`), or wrapper `ACTION_INPUT`, `ACTION_OUTPUT`, `ACTION_EXECUTION`; otherwise empty. |

The entrypoint logs classification/counts and sanitized CLI diagnostics for errors,
**not** the full analysis JSON, captured labels, file contents or original paths.
GitHub may display Action input/environment paths in its step log; choose
nonsensitive filenames and do not put credentials in paths or inputs. The CLI's
JSON stdout is consumed privately by the wrapper. A child timeout (120
seconds), oversized stdout (32 MiB), unexpected exit, malformed result or summary
mismatch becomes invalid/2. Missing Node/Bash, runner cancellation, or failure to
write `GITHUB_OUTPUT` can prevent outputs from being available; an empty output
must never be treated as clean.

### Optional upload before the final gate

Use `fail-on-result: 'false'` only when a later step **always** checks `exit-code`.
This defers both regression **and invalid-analysis** failure; it does not convert
either result into a clean comparison. The complete synthetic workflow shows an
explicit upload toggle and a final gate which also rejects missing outputs:

```yaml
- uses: dalkedev/snurpo/.github/actions/har-compare@v1.2.0
  id: compare
  with:
    baseline: private-captures/baseline.har
    current: private-captures/current.har
    output: scope-reports
    fail-on-result: 'false'
# Optional upload/review steps here; see the complete workflow for an allowlist.
- name: Enforce comparison result
  if: always()
  env:
    COMPARE_EXIT: ${{ steps.compare.outputs.exit-code }}
  shell: bash
  run: |
    case "$COMPARE_EXIT" in
      0|1|2) exit "$COMPARE_EXIT" ;;
      *) echo '::error::No valid comparison result'; exit 2 ;;
    esac
```

## Privacy and limits

Raw HAR files and configuration remain on the runner; this Action never uploads
them, fetches their URLs or replays their requests. It performs no automatic
artifact upload, PR comment, job summary publication, or account operation.
Default `labels: ids` removes captured method/origin/path, JSON field names,
pointers and original config strings from every report format. Counts, HTTP
statuses, types, entry indices and capture provenance remain. This minimizes
labels; **it does not anonymize traffic patterns or guarantee absence of secrets**.

`labels: readable` can expose sensitive hostnames, path segments and JSON field
names. Review all derived reports before sharing. Keep artifacts restricted to
their intended audience, upload only the three explicit report outputs, and use
short retention (the example uses three days). Never upload a workspace, capture
directory or wildcard that could include HAR/config files. Runner access and local
file cleanup remain the caller's responsibility.

The [CLI matching rules and bounds](../../../tools/har-compare/README.md#har-support-and-limits)
apply: selected JSON responses only, deterministic ordinal pairing of repeated
requests, bounded inputs, explicit selection/ignores, and no native archive input.
Missing bodies and unsupported selected responses fail rather than passing.
This is a change detector, not a universal API health check; use authored status
contracts to reject an unchanged error response. No desktop/device validation is
implied by an Action result.

## Local verification

From a Snurpo checkout with Node 22.22.3:

```sh
npm test --prefix tools/har-compare
node tools/har-compare/examples/run-demo.mjs
node --test tools/har-compare/action-test/*.test.mjs
```

Tests launch the real entrypoint and bundled CLI in a separate synthetic consumer
workspace, exercise the status/output contract, and check literal hostile paths,
label minimization, safe errors and stale-directory refusal. These are local
subprocess checks; they do not claim a hosted GitHub runner execution.

The [`HAR composite Action` workflow](../../workflows/har-compare-action.yml)
also invokes the actual composite on Linux for clean, regression and invalid
synthetic captures. It checks default and deferred caller outcomes, all eight
outputs, report minimization and isolation from a spoofed consumer CLI, using an
Action checkout path with spaces. It has read-only repository access and uploads
no artifacts. Both HAR workflows include Action files in their path filters, so
an Action-only edit runs the relevant checks. A green hosted run validates that
local composite invocation; remote `owner/repo/path@ref` resolution and other
runner operating systems remain separate validation targets.

Metadata and output-file behavior follow GitHub's
[composite metadata reference](https://docs.github.com/en/actions/reference/workflows-and-actions/metadata-syntax)
and [workflow command reference](https://docs.github.com/en/actions/reference/workflows-and-actions/workflow-commands).
