# Send it again, deliberately

Snurpo offers two ways to reuse a captured request. **Compose replay** sends a new exchange from Snurpo. **Copy as cURL** prepares a command for your terminal. Opening either editor does not send the request.

## Compose a replay

1. Select the captured exchange and choose **Compose replay**.
2. Review the method, **Request URL**, complete header list and body.
3. Use **Binary as base64** when editing binary body data.
4. Choose **Send request** to send it and record a new exchange.

The original capture remains available, and the replay is recorded in its original session. Current automatic traffic rules apply; manual breakpoints are skipped. To pause a message, send it through the proxy from your test client instead.

Native replay keeps TLS verification enabled, does not follow redirects and ignores environment proxy variables. Its transport timeout is 120 seconds. A timeout does not prove that the upstream server did not perform a write.

Captured authorization headers and cookies are reused unless you change them. If you change the destination, check those headers before sending.

## Copy as cURL in 1.5.0

Select **Copy as cURL**, review the command, then choose **Copy command**. If clipboard access fails, select and copy the preview text manually.

The command targets a POSIX shell such as sh, bash or zsh. On Windows, use WSL or another compatible shell; PowerShell and cmd.exe syntax are not generated.

Copying sends no traffic. Running the command sends a real request with your terminal's network settings. Snurpo rules and breakpoints are not applied. TLS verification stays enabled, cURL's configuration file is disabled, and no automatic redirects or retries are added. Terminal proxy environment variables can still apply.

## What the command preserves

The export uses the effective captured request, including repeated end-to-end headers and text or binary body bytes. It does not export the pre-rewrite original as a wire recording.

Host, Content-Length and hop-by-hop headers are normalized for the new transport. Proxy credentials and host overrides are not reproduced. Whitespace-only header values are normalized to empty values.

> The command contains the captured URL, headers and body, including credentials. It is a debugging export, not a sanitized sharing copy.

Malformed inputs, HEAD requests with a body, more than 1,024 input headers or a command reaching 128 KiB produce an explanation instead of a partial command. Use replay or [session export](sessions-export.md) when cURL export is unsuitable.
