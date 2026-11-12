# Follow the request.

Snurpo is a desktop HTTP(S) debugger with a curious nose and a local archive. Route a request through it, inspect what happened, change the next exchange, and keep the evidence for later.

Capture, saved sessions, export and built-in analysis work without an account or subscription. Your application still contacts its normal servers. Connecting an external MCP client is a separate, explicit choice.

## Get a useful result first

1. [Install Snurpo](installation.md) for your operating system.
2. [Capture the included local traffic](first-capture.md). No phone or certificate setup is needed.
3. Pick a request and inspect its response, headers and timing.
4. [Replay it or copy a cURL command](replay-curl.md), then compare the result.

![Snurpo's native traffic inspector showing captured requests and response details.](/assets/traffic-inspector.webp)

This native screenshot records Snurpo 1.4.2. The guides describe 1.5.0, which adds Copy as cURL.

## Choose the next question

| What are you investigating? | Start here |
| --- | --- |
| What did my app send? | [Inspect and search](inspect-and-search.md) |
| Why is HTTPS failing? | [HTTPS and certificate trust](https.md) |
| What happens if the API returns an error? | [Rules and breakpoints](rules-breakpoints.md) |
| What changed between two captures? | [Analysis and comparison](analysis.md) |
| Can my agent inspect this evidence? | [MCP access](mcp.md) |
| Can I capture an Android app? | [Experimental Android workflows](android.md) |
| Can this become a CI check? | [Offline HAR comparison](har-compare-ci.md) |

## Know what you are looking at

A mock is labeled as a mock. Edited messages retain originals. A replay creates a new exchange. These distinctions matter when you explain a failure or share a report.

Snurpo currently buffers bodies up to 16 MiB each. It does not implement streaming capture, WebSocket frames, HTTP trailers or HTTP/3. Observed API schemas describe your sample; they are not a complete contract.

Snurpo was previously called Scope. Some file names and MCP identifiers remain compatible by design. [Release and migration notes](releases.md) explain what stays the same.
