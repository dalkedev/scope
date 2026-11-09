# Getting started

Scope stores captures locally and works without an account. Start with the included fixture to check the desktop, then connect a client or Android app.

## Install a release

1. Download the artifact for your host from [GitHub Releases](https://github.com/dalkedev/scope/releases). Read its release notes for architecture, optional Frida resources and signing status.
2. Compare its SHA-256 with the release checksum file, then extract the archive or run the platform installer.
3. On macOS, move `Scope.app` to Applications and open it. The public package is ad-hoc signed without Apple notarization. If Gatekeeper blocks it, use macOS **System Settings → Privacy & Security → Open Anyway** after verifying the source and checksum. Do not disable Gatekeeper globally.

The interactively tested host is **macOS arm64** (Apple Silicon). Windows x64 and Linux x64 have CI build/headless-protocol coverage; their interactive installers, desktop and device behavior remain unverified. Read the release notes for Linux's known dependency advisory. Optional Frida native binaries are not included in public packages; [install them locally](ANDROID.md#frida-runtime) for Frida mode.

## Capture local test traffic

Click the lightning bolt beside **New session**. Its tooltip is **Generate real test traffic through the proxy**. Scope starts the local fixture at `127.0.0.1:17891` and captures six requests through the proxy at `127.0.0.1:17890`.

The fixture includes a deliberate 401 sign-in response, a slow endpoint and repeated product requests. These are real loopback requests, labeled as demo traffic. Scope does not populate an empty archive with fabricated captures.

Select a row and switch between request, response, headers, body, timing and originals. Open **Analysis** to inspect observed endpoint/status/timing patterns. **Compare** accepts two saved sessions.

Find related failures with `filter: method=POST status>=400`, or search ordinary
text as before. **Search syntax** below the field offers clickable examples.
[Fields, quoting, lifecycle states and search limits](SEARCH.md).

## Send traffic from a desktop client

Create a session and select **Start capture**. Configure your client to use the address shown by Scope:

```sh
curl --proxy http://127.0.0.1:17890 http://example.com
```

For HTTPS, open Scope's connection settings and copy its public CA path. Tell the client to trust that certificate:

```sh
curl --proxy http://127.0.0.1:17890 --cacert /absolute/path/from/Scope/scope-ca.pem https://example.com
```

Use the actual path from your installation. Scope generates its own CA and keeps the private signing key in its application data directory. Share only the public CA. Trusting a CA and bypassing app-specific certificate pinning are separate steps.

## Connect Android

Install Android SDK Platform Tools, enable USB debugging, and authorize your computer on the device. A rooted emulator is the first verified setup.

1. Create/select a session and start capture.
2. Open the device setup, refresh devices and select the device and installed package.
3. Choose **System proxy** or **Frida**, then start interception.
4. Use the selected app and inspect the resulting exchanges.
5. Press **Stop** while the device remains connected.

System proxy mode changes the device-wide HTTP proxy. Other apps may produce traffic in that session. Frida instruments the selected process and supports the Java APIs covered by its enabled hook profiles. Native TLS and QUIC may need different instrumentation.

For runtime installation, supported hook profiles, non-root limitations and exact cleanup behavior, see [Android interception](ANDROID.md). The included [Android demo app](../examples/android-demo) is a small local test target.

## Pause and change a message

1. Open **Traffic rules**, create a rule, and match the endpoint's host and path.
2. Choose **Pause & edit**, then request, response, or both stages. Save the rule.
3. Send a new request through Scope from your client or Android app. The paused
   message banner appears even while you stay in the rule editor.
4. Open the editor. It contains the real message: edit the request URL/method,
   headers or body, or the response's final status, headers or body.
5. Send it with your changes, continue it unchanged, or abort that transfer.
   **Both** creates a separate response pause after the request is continued.
6. Inspect **Original** to compare the captured message before the edit.

A breakpoint expires after **60 seconds**, at which point the transfer is aborted.
Closing its editor does not continue it. The global Request/Response buttons in
Traffic still pause every proxied message when enabled; an endpoint rule only
pauses its matches. Disable global breakpoints and rules matching the demo routes
before generating the automatic demo batch. **Replay** sends a new outbound
request directly and creates another exchange; it skips manual pauses. Send
traffic through the proxy to pause and edit it. [Matching, ordering and examples →](RULES.md)

## Save and move sessions

Captures are committed to the local SQLite archive as they arrive. Stopping capture or quitting Scope does not delete them.

Use the session export controls to save:

- **HAR** for interoperability with other HTTP tools.
- **Scope session** (`.scope.json`) for a lossless Scope archive, including Scope-specific metadata and original edited messages.

Import creates a separate session. Ordinary exports contain captured credentials
and body content. The separate **sanitized** option masks known credential fields,
omits binary/non-JSON bodies and session notes/tags, and marks the file as lossy.
Review URLs and application-specific fields before sharing. MCP redaction does
not change the raw archive. OpenAPI export contains observed shapes without
payload examples; its documented limits mean it is not a full API contract.

Open the session controls to rename, add notes/tags, pin or delete. Deleting
requires the confirmation shown by the app and is rejected while that session
is actively capturing. Deletion removes the local session and exchanges, not
files previously exported elsewhere.

## Automate a local mock

Open **Rules** and create a disabled mock for `GET`, host `127.0.0.1`, path prefix
`/api/products`, status `503`, JSON content type and `{"maintenance":true}` body.
Save, then enable it and send a request through the capture proxy. The response
comes from the rule without contacting the upstream endpoint; the row records
mock provenance and the applied rule. Disable or remove it to restore normal
traffic. Rules are global across sessions. [Ordering and format](RULES.md)

Use **Replay** to compose a separate request with edited method, URL, full headers
or body. Review credentials before changing destination. Replay follows current
rules and bypasses manual breakpoint pauses. **Activity** records local event
metadata without copying traffic bodies or credential headers into a second log.

The connection/settings view displays the data directory. It contains `scope.sqlite3`, Scope's public CA and private key, MCP bridge configuration, and device recovery state. The SQLite database is not encrypted by Scope. For a consistent filesystem backup, stop capture and quit the app before copying the directory. Do not publish that directory.

## If something goes wrong

| Symptom                                    | Next step                                                                                                           |
| ------------------------------------------ | ------------------------------------------------------------------------------------------------------------------- |
| App reports a busy port                    | Check for another Scope instance or a process using 17890, 17891 or 17892; do not stop unrelated processes blindly  |
| No devices                                 | Check `adb devices -l`, authorization, USB debugging and the ADB discovery paths in [Android setup](ANDROID.md#adb) |
| HTTPS fails                                | Confirm the client trusts Scope's CA; inspect pinning/native-stack compatibility separately                         |
| Paused request fails                       | Check the 60-second breakpoint expiry and the client's own timeout                                                  |
| Device disconnected during interception    | Reconnect the same device and press Stop to retry recorded cleanup                                                  |
| MCP sees no sessions                       | Share the selected session in **MCP server**; sharing resets after app restart                                      |
| UI works in a browser but capture does not | Run `npm run desktop`; `npm run dev` is only the frontend preview                                                   |

If cleanup fails, keep the recovery file and reconnect the same device. Removing the recovery file does not restore the device. See [recovery details](ANDROID.md#cleanup-and-recovery).

For a reproducible bug report, use the repository's [issue forms](https://github.com/dalkedev/scope/issues/new/choose). Include sanitized logs and the exact stage that failed, not private captures or CA keys.
