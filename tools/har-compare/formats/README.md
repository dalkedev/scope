# JUnit XML export

`junitReport(report)` in `junit.mjs` renders an already-produced Snurpo comparison report as deterministic JUnit-style XML. It has no runtime dependencies, file IO, network requests or model calls. The CLI coordinator owns output path creation and exit codes.

```js
import { junitReport } from './formats/junit.mjs';
const xml = junitReport(comparisonReport);
```

One `testsuite` named `scope-har-compare` contains one `testcase` per finding. Regression findings contain `failure`; informational findings contain `skipped` and are not successful assertions. An analysis with no findings contains one passing `clean-analysis` testcase. Therefore JUnit testcase counts describe reported findings, not requests, paired responses or assertion coverage. Suite counts derive from actual finding severities, independently of summary counters. Unknown severities fail closed with `REPORT`.

Stable finding IDs/kinds identify cases. Available operation method/origin/path labels appear in case names; missing labels after a privacy transform are supported. Evidence includes entry references, occurrence, pointer, type/status details, and contract indices when present. Full report/config objects, arbitrary additional fields, request keys, body values and query data are never serialized. Any privacy transformation must happen before this formatter runs. Origin/path labels and JSON pointers can still be sensitive; inspect artifacts before sharing.

No timestamps, hostname, durations or other run metadata are added. Ordering follows the existing deterministic finding order. Regression output does not alter the CLI's configured-regression decision; the formatter neither exits nor converts informational findings into failures. Analysis errors remain errors, rather than fabricated passing JUnit results.

Every emitted data string escapes XML metacharacters, including both quotation marks. Characters prohibited by [XML 1.0](https://www.w3.org/TR/xml/#charsets), including NUL and lone UTF-16 surrogates, become Unicode replacement characters (`U+FFFD`). Valid Unicode, including supplementary characters, is preserved. The output declares UTF-8, contains no DTD/entity declarations, and references no external resources.

The module accepts the internal comparison report's `operations` and `findings` arrays, with at most 4,000 operations (the union of two bounded captures) and 10,000 findings. Serialized UTF-8 XML is capped at 4 MiB; excess output raises `LIMIT` without returning partial XML. It does not validate an arbitrary third-party report schema.

Run the focused tests from the repository root:

```sh
/opt/scope-lab/toolchains/node-v26.13.1-linux-x64/bin/node --test tools/har-compare/formats/*.test.mjs
```

Production code and standard Node tests require no root dependencies. One optional parser integration test uses `python3` and its standard-library `xml.etree.ElementTree`; it is explicitly skipped if `python3` is unavailable. On 2026-11-03 all 11 formatter tests passed, including that parser check. This establishes XML well-formedness for the tested fixtures, not compatibility with every CI provider or a remotely executed CI upload.
