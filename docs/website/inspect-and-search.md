# Find the interesting request

Select a session, then pick a row in the traffic table. **Request** and **Response** select a message; **Body**, **Headers** and **Timing** show its details. An edited exchange also exposes **Original**.

Repeated headers remain separate entries. Mocked and blocked responses carry their provenance, and applied rules are listed in the inspector. A pending exchange is not automatically a failed exchange.

## Search the whole selected session

An ordinary query is a case-insensitive substring across method, URL, status, headers and stored bodies. Spaces remain part of that substring.

Use the exact lowercase prefix `filter:` to combine structured conditions:

```text
filter: method=POST status>=500
filter: host=api.example.test status=429
filter: body~="rate limit"
filter: state=pending
filter: edited=true method!=GET
```

All conditions must match the same exchange. **Search syntax** under the field offers clickable examples.

| Field | Supported operators | Meaning |
| --- | --- | --- |
| `method` | `=`, `!=` | Exact method, ignoring ASCII case. |
| `host` | `=`, `!=` | Exact hostname without port, ignoring case. |
| `status` | `=`, `!=`, `>`, `>=`, `<`, `<=` | HTTP status from 100 to 599. |
| `body` | `~=` | Substring in plain request or response bodies; excludes base64 bodies. |
| `state` | `=` | `pending`, `error` or `complete`. |
| `edited` | `=` | `true` or `false`. |

Quote values containing spaces. Inside quotes, `\"` represents a quote and `\\` a backslash. There is no OR, regex or nested-expression syntax. Invalid queries show an error rather than silently becoming a successful empty search.

`state=error` means a completed exchange with a recorded error or HTTP status at least 400. `state=complete` excludes those errors. A missing status matches none of the status operators, including `!=`.

## Two filters, different reach

Structured conditions apply before pagination across the selected archive session. The **All traffic / Successful / Errors / Edited** dropdown filters the currently returned page.

To search literal text beginning with a reserved prefix, use:

```text
text:filter: method=POST
```

Structured queries are limited to 4096 UTF-8 bytes and 32 conditions. Search still scans saved records; it is not a large-session indexing feature. MCP search has a separate sanitized view and does not accept this structured grammar. [MCP details](mcp.md).
