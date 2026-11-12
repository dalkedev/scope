# Reports with labels omitted

The default `--labels readable` retains useful method/origin/path and JSON pointer
labels while excluding raw body/header/cookie/query values. For artifacts shared
outside the capture's usual audience, `--labels ids` also removes those labels:

```sh
node tools/har-compare/cli.mjs compare \
  --baseline baseline.har --candidate candidate.har \
  --labels ids \
  --json out/analysis.json --html out/report.html --junit out/results.xml
```

Comparison, selection, strict JSON parsing, ignores and authored assertions run
on the original inputs before projection. Exit status and finding counts do not
change. The same projected report drives stdout, JSON, HTML and JUnit; original
config selectors/pointers are not embedded in an HTML details section or XML.

Retained metadata: report-local operation/finding IDs, original HAR entry
indices, counts, finding kinds/severity, HTTP statuses, JSON type names, authored
rule/requirement ordinals, coverage counts and validated Snurpo provenance/edited
flags. Removed labels: captured method/origin/path, JSON pointers, original
selection/ignore/contract configuration strings. Config is replaced with mode
and rule counts plus `redacted: true`; it is metadata, not a replayable config.
No hashes of omitted labels are emitted. Use original private HAR/config files
to resolve the retained evidence indices locally.

This is explicit data minimization, not anonymization or complete secret
classification. Interaction patterns, counts, statuses, types and provenance
remain visible. `inspect` deliberately rejects the flag: inventory requires
operation identities and should be reviewed as a private discovery artifact.

Identical inputs/config/flag/source produce identical output. Explicit
`--labels readable` matches the original default bytes. Existing/new-file and
output-collision protections apply to every format. Invalid `--labels` values
fail with exit2, and invalid analysis never creates a clean report merely because
labels were requested to be omitted.
