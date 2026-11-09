# Traffic rules

Rules are saved locally and apply to all capture sessions. Open **Rules** to
create, enable, duplicate, change priority or remove one. A disabled rule does
nothing. The editor requires an explicit all-traffic choice if no matcher is set.

## Matching and order

Matchers are ANDed: exact method, exact host (case-insensitive, without port),
path prefix and URL substring. Empty matcher fields are ignored. Rules run by
ascending priority, then ID. The engine snapshots the enabled rules and their
matches at request entry. Changing a rule affects subsequent requests, including
replay, rather than changing a response halfway through its transfer.

Request rewrites run before sending upstream. Mock and block responses do not
contact upstream. Response rewrites run on the resulting response. A terminal
mock/block stops subsequent request actions; matched response rewrites can still
apply. Manual breakpoints run after automatic actions, so an explicit manual
edit wins. Replay uses current rules and skips manual breakpoints.

## Pause an endpoint and edit the real message

Create a rule matching the endpoint's host and path, then choose **Pause & edit**.
Choose the request, response, or both stages. A pause rule contains no replacement
body: the editor is filled with the actual message when it arrives.

- **Request:** holds the message before it is sent upstream. Edit its method,
  URL, headers or body, then send it on.
- **Response:** lets the request reach upstream, then holds the received response
  before the client gets it. Edit its final status, headers or body, then send it on.
- **Both:** holds the request first, then opens a separate response pause after
  the continued request receives a response.

The paused-message banner is available from every workspace, including the rule
editor. Open it to edit the message, continue it unchanged, or abort it. Closing
the editor only hides the editor; it does not release the message. Each stage
expires after 60 seconds and aborts the transfer. The client may time out sooner.
Stopping capture aborts outstanding pauses rather than forwarding them silently.

For example, match `api.example.test` and path prefix `/v1/products`, then use:

```json
{ "type": "pause", "stage": "response" }
```

Only matching traffic pauses; other endpoints continue normally. A path prefix
also matches longer paths beginning with that text. Add a method or URL substring
when you need a narrower match. The global Request/Response breakpoint buttons
still pause all proxied traffic when enabled.

Pause selectors use the original request's rule snapshot, even if an automatic
rewrite or a manual edit changes its destination. Automatic actions run before
the manual pause for that stage. Matched pauses also apply to mock and blocked
responses, including selectors ordered after the terminal request action.
Several matching pause rules and a global breakpoint create only one editor per
message/stage; every matching pause rule is recorded at the stage actually reached.
The edited and original messages remain in the session. Direct replay skips
pause rules and does not record them as applied.

The built-in six-request demo generator refuses matching manual pause rules to
avoid waiting for an editor during an automatic batch. Use your own test client
through the proxy to exercise a pause interactively.

Each exchange records `provenance` (`upstream`, `mock`, `blocked`) and
`appliedRules`. Original messages remain available alongside edits. A mock is
synthetic evidence; its timing/status must not be treated as an upstream result.

## Full rule format

```json
{
  "id": "",
  "name": "Products maintenance fixture",
  "enabled": false,
  "priority": 100,
  "matcher": {
    "method": "GET",
    "host": "127.0.0.1",
    "pathPrefix": "/api/products"
  },
  "action": {
    "type": "mockResponse",
    "status": 503,
    "headers": [{ "name": "Content-Type", "value": "application/json" }],
    "body": "{\"maintenance\":true}",
    "bodyBase64": false
  }
}
```

An empty ID creates a new rule. An existing ID replaces the entire rule.
Matchers may also contain `urlContains`.

| Action type       | Fields                                                                                |
| ----------------- | ------------------------------------------------------------------------------------- |
| `rewriteRequest`  | Optional `method`, absolute HTTP(S) `url`, `headers`, `body`, `bodyBase64`            |
| `rewriteResponse` | Optional `status`, `headers`, `body`, `bodyBase64`                                    |
| `mockResponse`    | `status`; optional `headers` (empty by default), `body` (empty), `bodyBase64` (false) |
| `block`           | Optional `reason`                                                                     |
| `pause`           | Required `stage`: `request`, `response` or `both`; no replacement fields                 |

`headers` replaces the full header list; it does not merge by name. Repeated
headers remain separate array entries. Omitted rewrite fields keep their current
values. Body changes normalize Content-Length and remove stale content encoding,
digest and ETag metadata. Host is normalized to the destination. Changing a
destination can forward captured credentials there; review the complete request.

The engine validates a whole update before replacing the last good configuration.
Limits: 256 rules, 128 headers per action, 1 MiB decoded body per rule and 8 MiB
serialized total rule configuration. A rule body limit is separate from the
16 MiB capture limit. Matching does not execute arbitrary JavaScript or regexes.

Pause rules add a new action to the persisted rule format. Before downgrading to
a version without pause-rule support, remove or convert those rules using the
new version, or restore your pre-upgrade data backup. Merely disabling them does
not make their action readable by an older binary.

## Agent automation

MCP's global rule inventory requires **Global rule access**; creating/replacing
or deleting also requires **Allow traffic changes**. This access affects all
captures, including unshared sessions. Inventory reads omit names, matchers and
replacement payloads. Supply a full definition when updating; an inventory row
is not a restorable rule. All permissions reset when Scope restarts.

See [MCP](MCP.md), [architecture](ARCHITECTURE.md) and the real HTTP/TLS
integration tests in `crates/scope-core/tests/rules.rs`.
