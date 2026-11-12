# Change the next exchange

Open **Traffic rules** and select **New rule**. A rule can rewrite a request or response, return a mock, block a request, or pause a real message for editing. Rules are saved locally and affect all capture sessions.

## Make a local maintenance response

After generating the [local fixture](first-capture.md), create this rule in the editor:

| Field | Value |
| --- | --- |
| Method | `GET` |
| Host | `127.0.0.1` |
| Path starts with | `/api/products` |
| Action | **Mock response** |
| Status | `503` |
| Header | `Content-Type: application/json` |
| Body | `{"maintenance":true}` |

Save and enable the rule, then send a new matching request. The inspector marks the result **Mock response**. A mock does not contact the upstream server. Disable the rule when the experiment is finished.

## Pause an endpoint

1. Match the intended host and path, then choose **Pause & edit**.
2. Under **Pause at**, choose request, response or both. Save and enable the rule.
3. Send a request through the proxy from your client.
4. Open the paused-message banner. The editor contains the actual message.
5. Edit and continue it, continue unchanged, or abort the transfer.

Request pauses happen before upstream delivery. Response pauses hold the result before your client receives it. Both creates a separate pause for each stage.

> A pause expires after 60 seconds and aborts the transfer. Closing the editor does not release it. Your client's own timeout may be shorter.

The global Request/Response breakpoint buttons pause all proxied messages when enabled. Endpoint pause rules only affect their matches. Stopping capture aborts outstanding pauses.

## Matching and ordering

Matchers are ANDed: exact method, exact host without port, path prefix and URL substring. Lower priority numbers run first, then rule ID. The request keeps the enabled-rule snapshot and original matching result from its entry point.

Automatic edits run before manual pauses. A terminal mock or block stops later request actions, but matched response rewrites and pauses can still apply. **Replay** uses current automatic rules and skips manual breakpoints.

Replacing headers replaces the complete list. Changing a destination can forward existing credentials there, so review the whole request. Rule bodies are limited to 1 MiB, independently of the 16 MiB capture limit.

See the [complete rule schema and ordering reference](https://github.com/dalkedev/snurpo/blob/v1.5.0/docs/RULES.md) for programmatic configuration and downgrade compatibility.
