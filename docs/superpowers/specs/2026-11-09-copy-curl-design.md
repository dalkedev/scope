# Captured request to cURL

## Intent and scope

Close the terminal handoff gap: select an exchange, preview a POSIX-shell cURL
command, explicitly copy it, and run it separately. Opening or copying never
sends traffic. Keep the current 1.4.1 version; “toward 1.0” means product maturity, not a downgrade.

## Approaches

- Copy as cURL: small, directly useful interoperability improvement. Selected.
- Large-capture streaming: high value, but requires archive/body architecture
  work and memory measurements; remains a separate project.
- WebSocket inspection: useful protocol coverage, but needs capture, storage,
  inspector and replay design; remains a separate project.

## Behavior

Inspector opens a preview dialog labeled for sh/bash/zsh (not PowerShell/cmd).
Warn that URL, headers and body can contain credentials. Preview is the actual
command; clipboard success/failure is visible and manual selection is available.
Use effective captured request fields, never mutate the exchange. Preserve
method case, repeated end-to-end headers, query/path and body bytes. Normalize
Host, Content-Length and hop-by-hop headers including Connection-nominated ones.
Suppress cURL's extra Accept, User-Agent, Content-Type and Expect defaults where
absent. Empty captured header values must be emitted using cURL's semicolon form.
Whitespace-only SP/HTAB header values become empty headers. Empty captured
framing is regenerated as Content-Length: 0. HEAD uses --head and nonempty HEAD
bodies are rejected with a Compose replay alternative. Do not automatically follow redirects, retry, disable TLS checks
or route through Scope. Disable curlrc and URL globbing; restrict HTTP(S).

Text bodies use printf with a fixed %s format piped to --data-binary @-; base64
bodies decode locally and use POSIX printf %b octal bytes, including NUL. Reject
invalid base64/URLs/header values/control characters before producing a command.
Bound generation to 128 KiB of command UTF-8, with early limits before expansion.
For larger captures explain that HAR/native export or Compose replay is available.
No backend, MCP, dependency, lockfile or runtime-version changes.

## Validation

Unit tests cover quoting/injection, leading @, repeated/empty headers, transport
normalization, Unicode, binary/NUL, bounds, HEAD and malformed imports. Component
tests cover explicit copy, rejection/manual fallback and selection changes. A
POSIX-only integration test runs the generated command with real cURL against a
loopback HTTP server and compares method, target, headers and exact body bytes.
Full UI suite and production build required. Native GUI and Windows shell execution
remain separate acceptance, not inferred from jsdom or Linux tests.

## Review decisions

Independent review found stale same-ID previews, whitespace-header removal and
missing empty-POST framing. Regression tests reproduced each before the fix.
The preview remounts when the generated command changes, which also prevents old
clipboard promises from reporting success for a newer request. The command bound
is 131071 UTF-8 bytes so the terminating NUL fits Linux's single-argument limit.
