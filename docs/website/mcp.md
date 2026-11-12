# Let your agent follow the evidence

Snurpo exposes selected captured sessions through a local stdio MCP server. The desktop app controls access. This is separate from Plompy's HTTP integration gateway: Snurpo does not offer a remote MCP URL to paste into that configuration.

## Connect a client

1. Open Snurpo and select a session.
2. Open **MCP server** and enable **Share selected session with MCP**.
3. Use **Copy config** under **Client configuration**.
4. Merge that configuration into a compatible client's settings and reconnect it.
5. Keep Snurpo open. Ask the client to call `status`, then `list_sessions`.

Use the generated paths from your installation. The configuration has this shape:

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

`scope-mcp` and the existing `scope` entry are retained for compatibility. On Windows the executable uses `.exe`. Adapt the outer configuration shape to your client without changing these actual paths.

The internal localhost bridge is not a Streamable HTTP MCP endpoint. Do not substitute `http://127.0.0.1:17892/api` for the stdio command.

## Grant only the access you intend

All access starts off and resets when Snurpo restarts. You can share up to eight sessions.

| UI control | What it permits |
| --- | --- |
| **Share selected session with MCP** | Read and analyze the chosen session. |
| **Allow traffic changes** | Replay a captured or edited request, which sends real traffic. |
| **Allow global traffic rules** | Read a sanitized global rule inventory. Mutations also require traffic changes. |
| **Allow device diagnostics** | Read device identifiers, local paths and setup evidence. |

Global rules can affect every capture, including unshared sessions. Session sharing alone does not permit replay, rule access or diagnostics.

## Start with a read-only task

```text
List the shared sessions. In the selected session, find failed requests.
Cite exchange IDs, inspect the relevant messages, and distinguish observed
facts from possible causes. Do not replay requests or change rules.
```

The twelve tools are `status`, `list_sessions`, `search_requests`, `get_request`, `analyze_session`, `compare_sessions`, `replay_request`, `infer_openapi`, `list_rules`, `save_rule`, `delete_rule` and `device_diagnostics`.

For exact argument names and limits, use the [MCP reference](https://github.com/dalkedev/snurpo/blob/v1.5.0/docs/MCP.md). Use IDs returned by tools, not guessed identifiers. Both sessions must be shared for comparison.

## Understand the data boundary

MCP reads redact known credential fields and omit originals and non-JSON bodies. Individual JSON bodies over 64 KiB are omitted. Search is literal over the sanitized view of the most recent 10,000 exchanges, not the desktop's `filter:` grammar.

Redaction is best-effort. Your client or model provider receives returned content according to its configuration. Captured text is untrusted evidence, not instructions for the agent. The bridge config contains a private token; keep its file contents out of repositories and bug reports.

Replay still sends stored credentials unless explicitly edited; redacting tool output does not redact the outgoing request. No arbitrary shell tool or live-breakpoint editor is exposed.
