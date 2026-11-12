# Check saved HARs offline

Snurpo includes a separate dependency-free Node CLI for comparing two saved HAR captures. It does not open the desktop, replay requests, call a model or upload the captures. This remains a local CI experiment, with its own version `0.1.0`.

## Run a comparison

Use a Snurpo source checkout and Node.js 26.13.1 or newer. No npm install is needed for the CLI itself. From the repository root:

```sh
node tools/har-compare/cli.mjs doctor
node tools/har-compare/cli.mjs compare \
  --baseline before.har --candidate after.har \
  --json new-report.json --html new-report.html \
  --junit new-report.xml --labels ids
```

Supply captures you already saved. Output paths must be new and distinct; existing files are not overwritten. The CLI also writes analysis JSON to stdout, so choose where your shell or CI retains it.

| Exit code | Meaning |
| --- | --- |
| `0` | Analysis completed with no configured regression candidates. Informational findings may remain. |
| `1` | Completed analysis found regression candidates; reports are available. |
| `2` | Invalid input, configuration, output or incomplete analysis. This is not a clean result. |

The default `shape` mode detects changes such as removed observed JSON fields and changed types. `strict` also checks scalar value changes. Repeated matching requests pair in HAR encounter order; this is a deterministic assumption, not proof that two operations mean the same thing.

## Express a requirement explicitly

An unchanged HTTP 500 does not fail merely because it is an error. A change detector needs an authored requirement to reject that case. For example, save this as `contract.json`:

```json
{
  "contracts": [
    {
      "method": "GET",
      "origin": "https://api.example.invalid",
      "path": "/users/7",
      "allowedStatuses": [200]
    }
  ]
}
```

Pass `--config contract.json` and substitute your actual operation. Unsupported or missing selected response bodies fail analysis rather than being silently skipped. Mixed browser HARs often need explicit operation selection.

## Use the GitHub Action

The repository's composite Action wraps the same CLI on Linux. The caller supplies Node 26.13.1+, Bash and the capture files. A step can use:

```yaml
- uses: dalkedev/snurpo/.github/actions/har-compare@v1.5.0
  id: compare
  with:
    baseline: captures/before.har
    current: captures/after.har
    output: new-snurpo-reports
    labels: ids
```

Pin an inspected full commit SHA for a stable dependency. The output directory must not exist. The default gate fails on either `1` or `2`; deferred gating requires your own final check that also rejects a missing result.

Neither the CLI nor the Action automatically uploads artifacts. `labels: ids` removes captured labels from reports but does not guarantee anonymity. Review reports before sharing them.

See the [CLI reference](https://github.com/dalkedev/snurpo/blob/v1.5.0/tools/har-compare/README.md) for matching and selection limits, and the [Action reference](https://github.com/dalkedev/snurpo/blob/v1.5.0/.github/actions/har-compare/README.md) for a complete workflow and output contract.
