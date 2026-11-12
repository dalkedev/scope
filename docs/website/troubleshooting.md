# Find the failing step

Separate the desktop, proxy, client trust and captured evidence. A visible window does not mean a client is using the proxy, and a running listener does not prove that an app accepts the CA.

## Start with a local control

Try **Generate test traffic**. If six loopback requests appear, inspect one response and reopen the saved session. That gives you a working desktop/proxy/archive path before adding HTTPS or a device.

| Symptom | Next check |
| --- | --- |
| Port is busy | Check for another Snurpo or old Scope instance. Default ports are 17890 for the proxy, 17891 for the local fixture and 17892 for the internal bridge. |
| Local fixture works, my app is absent | Confirm the app uses the displayed HTTP proxy. Check its networking stack and protocol. |
| HTTPS fails | Verify the CA path and client trust, then investigate pinning separately. [HTTPS guide](https.md). |
| Paused transfer fails | Check the 60-second breakpoint expiry and the client's shorter timeout. Closing the editor does not continue traffic. |
| Demo generation refuses to run | Disable global breakpoints and matching pause rules; stop an active capture in another session. |
| Structured search is empty | Check the exact lowercase `filter:` prefix and supported fields. A malformed query produces an error. |
| MCP cannot see a session | Share it in **MCP server**. Sharing resets after desktop restart. |
| MCP body is omitted | Non-JSON, oversized and original snapshots have deliberate output limits. Inspect the raw message locally. |
| Browser preview cannot capture | `npm run dev` is a frontend preview. Use the installed app or `npm run desktop`. |
| cURL preview cannot be generated | Read the displayed validation error. Try replay or export for oversized or unsupported requests. |

Do not stop unrelated processes simply because a port is occupied. **Connection settings** lets you change the proxy port while capture is stopped; update your client to match.

## Android cleanup

Use **Diagnostics** before interception. If a device disconnects during cleanup, reconnect the same device and press **Stop** again. Preserve recovery state until restoration succeeds. The [Android guide](android.md) explains mode-specific behavior and unsupported combinations.

## Report a reproducible problem

Use the [issue forms](https://github.com/dalkedev/snurpo/issues/new/choose). Include the Snurpo version, OS/architecture, exact action, expected result and observed error. If possible, reproduce it with local synthetic traffic.

Review attached captures and logs first. Leave out private CA keys, bridge tokens, account cookies and raw authorization headers. Sanitized exports mask known fields but still need review. Screenshot the relevant app area without unrelated desktop overlays or cursor highlights.

For release-specific limitations, see [releases and compatibility](releases.md).
