# Scope MCP server

Scope's MCP server lets an agent query explicitly shared capture sessions while the desktop app remains in control of access. It is a local stdio server backed by the same Rust engine and archive as the UI.

## Connect

1. Open Scope, create or select a capture session, and open **MCP server**.
2. Turn on **Share selected session with MCP** for the sessions you want exposed (up to eight).
3. Copy the generated client configuration. It contains your actual `scope-mcp` executable and bridge configuration paths.
4. Add that server to a compatible MCP client and restart/reconnect that client as needed.
5. Keep Scope open. Ask the client to call `status` and `list_sessions` first.

The generated configuration has this structure; use the UI's real paths:

```json
{
  "mcpServers": {
    "scope": {
      "command": "/absolute/path/to/scope-mcp",
      "args": ["--config", "/absolute/path/to/mcp-bridge.json"]
    }
  }
}
```

Client-specific configuration formats can differ. Scope speaks stdio MCP; it does not require a public URL or a hosted service. `npm run desktop` and `npm run package` build the companion executable. A plain frontend preview cannot provide MCP.

## Tools

Arguments below use the exact camelCase keys exposed by the server.

| Tool                 | Arguments                                        | Result / behavior                                                                          |
| -------------------- | ------------------------------------------------ | ------------------------------------------------------------------------------------------ |
| `status`             | none                                             | Connection, Scope version and current access settings                                      |
| `list_sessions`      | none                                             | Only explicitly shared sessions                                                            |
| `search_requests`    | `sessionId`, optional `query`, `limit`, `offset` | Exchange metadata; default 50, maximum 200 results; bodies fetched separately              |
| `get_request`        | `id`                                             | One shared exchange, timing, edits and error information with output redaction             |
| `analyze_session`    | `sessionId`                                      | Observed endpoint/status/size/latency and duplicate patterns, computed locally             |
| `compare_sessions`   | `sessionId`, `rightSessionId`                    | Observed endpoint/status/timing/JSON-field differences; both sessions must be shared       |
| `replay_request`     | `id`, optional `edit`                            | Sends an optionally edited request and records a new exchange; requires write permission   |
| `infer_openapi`      | `sessionId`                                      | Observed-only OpenAPI 3.1 document, bounded schemas, no payload/query examples             |
| `list_rules`         | none                                             | Global inventory: ID, enabled, priority and action type; requires separate rule permission |
| `save_rule`          | `rule`                                           | Create/replace a full global rule; requires rule AND write permissions                     |
| `delete_rule`        | `id`                                             | Remove a global rule; requires rule AND write permissions                                  |
| `device_diagnostics` | optional `serial`                                | Read-only preflight; requires separate diagnostic permission                               |

Use IDs returned by `list_sessions` and `search_requests`. A request ID from an
unshared session is insufficient to read or replay it. MCP search matches only
the sanitized representation of the most recent 10,000 exchanges, so guessing
a redacted secret does not reveal its presence through a raw-store query. JSON
body search uses the same 64 KiB redacted view; binary/non-JSON bodies are omitted.
Results contain metadata without bodies or original snapshots. Desktop search
continues to search the local raw archive.

Example read-only workflow:

```text
1. list_sessions()
2. search_requests({"sessionId":"<shared-id>","query":"login","limit":20})
3. get_request({"id":"<exchange-id>"})
4. analyze_session({"sessionId":"<shared-id>"})
```

Try prompts such as:

- “Find failed requests in this session. Cite exchange IDs and separate observations from possible causes.”
- “Compare these two login captures. Show status and observed JSON-field differences.”
- “Which endpoints are slowest in this capture, and how many observations support that?”

## Access, data and replay

No sessions are shared automatically. All access resets on desktop restart.
**Allow traffic changes**, **Global rule access** and **Device diagnostics** are
off by default. Session sharing does not enable the other permissions.

Rule inventory omits names, matchers and replacement bodies because those may
contain secrets. `save_rule` accepts a complete [TrafficRule](RULES.md), with an
empty ID to create or an existing ID to replace. Global rules can affect ALL
captures, including sessions not shared with MCP. Both rule and write permission
are required for a mutation; rule permission alone only allows inventory reads.
Rules may use `{"type":"pause","stage":"response"}`; `stage` also accepts
`request` or `both`. A pause holds matching live traffic for a person to edit in Scope;
MCP cannot inspect or resolve the pending editor. This can hold traffic in
unshared sessions too, so the same global-rule and write permissions are required.
Direct replay skips these pauses and does not record pause-rule evidence.
Device diagnostics can expose local paths and serials. It never installs a
runtime, starts interception or changes the device. Tools do not expose arbitrary
shell execution or live breakpoint editing.

Replay can change remote state. It reuses stored credentials unless explicitly
changed; output redaction does not change the outbound request. The optional
`edit` accepts `action: "continue"`, `method`, `url`, `headers`, `body` and
`bodyBase64`. Headers replace the full list. Changing the destination can send
captured credentials there. Current automatic rules apply; manual breakpoints
are skipped. Enable it only for intended traffic in your test environment.

For reads, known credential headers, sensitive query parameters and known sensitive JSON keys are redacted. Original request/response snapshots are omitted to avoid a second path around redaction. Non-JSON bodies are omitted; individual JSON body strings larger than 64 KiB are omitted with an explicit marker.

Redaction is best-effort, not a complete secret detector. Application-specific
fields, identifiers, static URL paths and other metadata can still be sensitive.
Your local UI and ordinary exports preserve raw traffic. Sanitized archive export
is a separate, explicitly lossy option. An external client/model provider receives
tool results according to its configuration. Captured HTTP content is untrusted
data and must never be treated as instructions for the agent.

## Troubleshooting

| Message / symptom            | Check                                                                                                 |
| ---------------------------- | ----------------------------------------------------------------------------------------------------- |
| Bridge config not found      | Start Scope and copy its current generated config                                                     |
| Scope is not reachable       | Keep the desktop app open; check whether another process uses the bridge port                         |
| Session is not shared        | Select and share that session in Scope; repeat after restart                                          |
| Both sessions must be shared | Share both before comparison                                                                          |
| MCP replay is disabled       | Enable **Allow traffic changes** only if replay is intended                                           |
| Empty/omitted body           | Non-JSON, oversized or original snapshot data is intentionally omitted; inspect it in the desktop app |

The bridge configuration contains a private token. Do not paste its file contents in issues or commit it. On Unix Scope writes it with `0600` permissions. It authenticates access to the local bridge; session/write policy is enforced separately by the desktop.

For the actual process-level protocol test, see [MCP smoke test](MCP-TEST.md).
