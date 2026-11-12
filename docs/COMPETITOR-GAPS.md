# Debugging workflow gaps

Reviewed 2026-11-09 against main `312802c` (then Scope 1.4.1, now Snurpo). These are documented
workflows, not a benchmark or an exhaustive product comparison. Existing replay,
HAR/native export, filtering, rules and session comparison were inspected before
selecting new work. No upstream implementation was copied.

| Workflow                                | Primary evidence                                                                                                                                                                                                                                    | Application at the reviewed baseline                                                                                        | Decision                                                                                                                         |
| --------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------- |
| Move one captured request to a terminal | [HTTP Toolkit snippets](https://httptoolkit.com/docs/reference/view-page/), [Proxyman Copy as](https://proxyman.com/docs/basic-features/copy-as), [mitmproxy exporter](https://github.com/mitmproxy/mitmproxy/blob/main/mitmproxy/addons/export.py) | Inspector offered Compose replay and Copy request URL, without a complete terminal command.                           | Implement cURL (POSIX shell) preview and explicit copy in this increment.                                                        |
| Compare two specific responses          | [Proxyman Diff](https://proxyman.com/docs/advanced-features/diff) documents request/response comparison with platform limits.                                                                                                                       | Session comparison aggregates endpoints, status, JSON shape and timings; it is not a detailed two-message value diff. | Follow-up candidate: pin an exchange as baseline and compare status/headers/body with evidence IDs.                              |
| Transfer reusable rule sets             | [Proxyman Import / Export Settings](https://proxyman.com/docs/basic-features/import-export-settings)                                                                                                                                                | Persistent ordered rule CRUD exists; no versioned collection import/export.                                           | Follow-up candidate: validate the full collection before writing, import disabled, preserve order and specify conflict handling. |
| Replay a captured request               | [Charles Repeat](https://www.charlesproxy.com/documentation/tools/repeat/), [HTTP Toolkit Send](https://httptoolkit.com/docs/reference/send-page/)                                                                                                  | Already delivered with edited replay and a new exchange.                                                              | Preserve; do not represent existing behavior as a new feature.                                                                   |

Large-session streaming (#12), WebSocket capture (#13) and packaged Windows/Linux
acceptance (#10) remain more substantial maturity work. A cURL export does not
close these issues or certify release readiness. The older roadmap's 1.0 language is historical planning context.

The command generator follows [cURL's documented option semantics](https://curl.se/docs/manpage.html)
for `--disable`, `--globoff`, `--path-as-is`, `--header`, `--head` and
`--data-binary`. Its exact limits and normalization are in
[the user guide](GETTING_STARTED.md#copy-a-request-to-your-terminal).
