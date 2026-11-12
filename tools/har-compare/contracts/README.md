# Authored response contracts

An observed baseline cannot establish which response fields are required. Optional developer-authored contracts detect, for example, HTTP 500 present in both captures, or a required field missing from both responses.

The CLI coordinator integrates this module under optional configuration property `contracts`:

```json
{
  "contracts": [
    {
      "method": "GET",
      "origin": "https://api.example.test",
      "path": "/users",
      "allowedStatuses": [200],
      "requiredJson": [
        { "pointer": "/user/id", "type": "string" },
        { "pointer": "/user/active", "type": "boolean" }
      ]
    }
  ]
}
```

Each rule requires `method`, `origin`, `path`, and at least one nonempty assertion collection. Unknown keys, duplicate selectors/pointers and invalid values fail with `CONFIG`. Omit the optional property to disable contracts; an explicit empty array is invalid.

## Semantics

- Selectors use the same exact canonical method/origin/path matching as `includeOperations`. Methods normalize to uppercase. HTTP(S) origins include nondefault ports and must omit credentials, trailing slash, query and fragment. Paths are exact URL pathnames, without query or fragment. Literal double-slash paths stay on the configured origin.
- Every selected candidate response matching a selector is checked, including repeated occurrences and all query variants. A baseline response does not have to satisfy the contract. Original HAR entry numbers identify evidence; the optional baseline entry pairs by matching key and encounter ordinal.
- `allowedStatuses` contains explicit final HTTP status integers, 200–599. It checks the candidate even when baseline status is identical. An allowed status does not suppress ordinary comparison findings or JSON requirements. JSON ignores never suppress contracts.
- `requiredJson` contains a JSON Pointer and exact type: `object`, `array`, `string`, `number`, `boolean`, or `null`. Missing paths and wrong types produce distinct regression findings. There is no coercion, inferred schema, optional-field syntax, value comparison, range constraint or wildcard expansion.
- Pointers follow [RFC 6901](https://www.rfc-editor.org/rfc/rfc6901): `""` selects the whole document, `"/"` an empty member name, `~1` decodes to `/`, then `~0` to `~`. Arrays require canonical zero-based indices; `/items/01`, `/items/-` and `/items/length` do not resolve array elements. A literal `*` can name an object member but has no wildcard meaning. Traversal uses own properties only; literal JSON properties named `__proto__` are safe to inspect.
- Empty/non-JSON bodies fail JSON requirements once per response. The CLI must finish normal selected-entry parsing/validation first; contracts never make missing or malformed capture data acceptable.
- A selector matching baseline entries but no candidate entries produces one `contract-operation-missing` regression using the first baseline entry. A selector matching neither selected capture fails `CONTRACT_UNUSED`, including operations excluded by selection. Rules do not extend analysis to excluded traffic.

Passing describes configured requirements for captured responses. It does not prove complete API compatibility, traffic coverage, availability, performance or security.

## Module interface

```js
import { validateContracts, evaluateContracts } from './contracts/index.mjs';
const contracts = validateContracts(config.contracts);
const result = evaluateContracts(baseline.entries, candidate.entries, contracts);
// { checks: number, findings: [...] }
```

Validation returns a new JSON-safe array sorted by method/origin/path, with numeric status lists and lexically sorted pointers. It does not mutate input. Its argument is the contracts array, not the top-level config. Evaluation consumes already-normalized entries from `src/input.mjs` and validated contracts. It performs no file IO, network calls, request replay or model calls.

`checks` counts one status check plus each JSON requirement for each matching candidate response. Empty/non-JSON bodies still count individual requirements while emitting one body finding. A wholly missing operation counts one check. This is not a count of requests or passed assertions.

Findings share `id` (for example `contract-finding-00001`), `rule`, `severity: "regression"`, and `evidence: { baselineEntry, candidateEntry }`. Evidence uses original one-based HAR entry numbers, or null when no matching side exists. `rule` and `requirement` are one-based indices in canonical validated configuration, not original input order. Resolve an authored pointer using `contracts[finding.rule - 1].requiredJson[finding.requirement - 1].pointer`.

| Kind | Additional fields | Meaning |
| --- | --- | --- |
| `contract-status` | `status.after` | Candidate status is outside its allowlist. |
| `contract-json-missing` | `requirement` | Required pointer did not resolve a JSON value. |
| `contract-json-type` | `requirement`, `types.before`, `types.after` | `before` is the authored expected type; `after` is actual candidate type. |
| `contract-json-body` | `types.before`, `types.after` | `before` is `json`; `after` is the normalized body kind. |
| `contract-operation-missing` | None | Selector matched baseline entries only. |

Findings exclude raw body values, captured keys, matching keys, URLs, query values, authored pointers and full status lists. Errors never echo rejected config. Top-level reports still contain authored config; selectors/pointers can themselves be sensitive, so review reports before sharing. This module does not claim secret detection.

## Bounds and tests

At most 100 rules, 100 statuses per rule, 100 JSON requirements per rule, and 1,000 JSON requirements total are accepted. Pointers allow 1,024 JavaScript code units and 64 reference tokens. The parser owns entry/body limits. Evaluation fails `LIMIT` above 10,000 findings or 4 MiB of serialized finding evidence, without silent truncation.

From the repository root:

```sh
/opt/scope-lab/toolchains/node-v26.13.1-linux-x64/bin/node --test tools/har-compare/contracts/*.test.mjs
```

Tests use real HAR parsing and cover authored status/type failures, missing-from-both fields, repeated/query variants, exact selectors, empty JSON availability, pointer escaping/prototype safety, config/budget failures, canonical ordering and redacted output. CLI integration is tested separately in the parent prototype.
