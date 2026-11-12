# Safe JSON interpretation

`parseJsonSafe(text, label)` returns the ordinary parsed JSON value or throws
the shared `AnalysisError` from `src/errors.mjs`. It uses no dependencies and
performs no I/O. The caller must supply a trusted diagnostic label such as
`Baseline HAR` or `Entry 1 body`; never pass a filename, URL or captured text as
the label. Diagnostics contain no offending member names, values or parser
excerpts.

The parent input layer should call this at its common parsing boundary for
HAR envelopes, configuration and selected response bodies. Unselected body
strings remain unparsed. The existing file-byte, body-byte, body-depth,
node-count and safe-integer validators still apply.

## Duplicate object names

Ordinary `JSON.parse` overwrites earlier object members with the same decoded
name. This parser rejects those ambiguous inputs with code `JSON`, including
escaped equivalents such as `"a"` and `"\u0061"`, or literal and escaped emoji
names. Independent objects may reuse a name. Unicode normalization is not
performed. `__proto__` and other prototype names are ordinary own JSON members;
the scanner tracks names with `Set` and never assigns them to a dictionary.

`JSON.parse` remains the syntax oracle. After valid syntax is established, a
linear lexical pass reads the original text and checks names before returning
the parsed value. Quoted punctuation and escaped quotes are skipped as string
contents, so this pass does not attempt to implement another JSON grammar.
The original source text remains available during the pass; overwritten members
are therefore still detected.

## Decimal round-trip policy

Every numeric token must have the same mathematical decimal value as
`JSON.stringify(Number(token))`. The check compares normalized decimal
coefficient/exponent pairs; it does not expand exponents or compare exact
binary fractions. Ordinary JSON decimals such as `0.1`, equivalent spellings
such as `1.00`, `1e0` and `1000e-3`, and signed zero are accepted.

Tokens such as `1.0000000000000001`, `1e-324` or `9007199254740990.9` lose
information when parsed to a JavaScript number and fail with code `NUMBER`.
Non-finite results also fail. This prevents two distinct source decimal values
from becoming an indistinguishable clean strict comparison. It does not add
arbitrary precision, distinguish numeric spelling, or make decimals exact
binary fractions. Integers outside the safe-integer range remain subject to the
existing body validator, even when their decimal round trip is exact.

Numeric tokens are limited to 4,096 characters and absolute exponents to
1,000,000. Nesting is limited to 128 containers, and standalone text is limited
to 10 Mi code units before parsing; the file reader retains its stricter byte
limits. Exceeding any scanner bound fails with code `LIMIT`.

## Verification

```sh
node --test tools/har-compare/json/*.test.mjs
```

The tests exercise decoded duplicate names, separate scopes, hostile quoted
punctuation, safe diagnostics, ordinary and lossy decimals, finite numeric
boundaries, Unicode keys, prototype members and resource limits. Existing
end-to-end CLI tests must also run after integration.

The policy deliberately tightens the earlier documented last-member-wins and
decimal-rounding limits. Reports should not continue to describe duplicate
names or silently lossy decimal tokens as accepted input.

Primary background: [RFC 8259 objects and numbers](https://www.rfc-editor.org/rfc/rfc8259.html),
[ECMAScript JSON.parse](https://tc39.es/ecma262/multipage/structured-data.html#sec-json.parse).
These sources explain interoperability and JavaScript parsing; the stricter
acceptance policy is Snurpo's explicit decision.
