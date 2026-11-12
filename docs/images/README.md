# Screenshot provenance

## Current Snurpo 1.4.2 captures: 12 November 2026

These are original JPEG captures of the running native macOS arm64 Snurpo 1.4.2
source build. The application rendered its real interface and reopened the eight
persisted localhost exchanges from the earlier native walkthrough. No browser
mockup, seeded interface rows, image generation, cropping or retouching was used.
The original 3600 × 2260 JPEG capture bytes are preserved without transcoding.

Snurpo was placed in native fullscreen mode to exclude the system capture/title
strip. Before each final capture, the pointer was parked outside the captured content
using the native system menu area. Each saved image was visually checked for the
absence of the pointer and system sharing indicator. The redundant `local` logo
badge is removed in the application itself.

The packaged application was copied to a separate native-review location. Only
that copy's bundle identifier was changed to `dev.snurpo.review` and it was
ad-hoc signed again, so the automation could distinguish it from an already
installed app. Application source, frontend, runtime resources and UI were not
changed for the capture. The actual product remains `dev.scope.debugger` for
installation/data compatibility.

The existing isolated `.runtime/native-review` archive was reopened using
bridge port `17994`. Capture remained stopped during this screenshot session. Displayed traffic uses only the fixture on `127.0.0.1:17891` and proxy
on `127.0.0.1:17890`. No external request or Android-device operation was part
of the capture. The interface displays synthetic product/profile data and a
synthetic maintenance response, not personal traffic or credentials.

| Image | Recorded state |
| --- | --- |
| [Traffic inspector](snurpo/traffic-inspector.jpg) | Eight saved exchanges: the six built-in fixture requests, one independently issued proxied curl and one edited POST replay. An earlier successful products response is selected, showing HTTP 200 and its original JSON. |
| [Traffic rules](snurpo/traffic-rules.jpg) | After the replay, with eight saved exchanges in the session, the saved maintenance rule is selected. Its host/path matcher, priority 10, enabled state, status 503 and JSON body are visible. |
| [Mocked response](snurpo/mocked-response.jpg) | The same eight-exchange session, selecting the saved independent products request. It returned HTTP 503 and the maintenance body; the inspector identifies **Mock response** and **Product API maintenance**. |
| [Replay composer](snurpo/replay-composer.jpg) | Eight exchanges after sending an edited POST to `/api/profile?scenario=composed`. It returned HTTP 200 and was stored separately. The screenshot reopens that saved request on the Body tab; the reopened composer was then canceled. |
| [Session analysis](snurpo/session-analysis.jpg) | The same eight exchanges: two error responses (401 and local 503), three products calls, one locally generated response and six observed method/path combinations. The displayed 840 B, 48 ms average and 382 ms slow request describe this capture, not a benchmark. |

The persisted maintenance rule is enabled with priority 10, host `127.0.0.1`,
path prefix `/api/products`, local status 503 and the JSON body
`{"error":"maintenance","retryAfter":30}`. The independent curl received that
status/body. The replay changes the method to POST, uses the profile URL above,
and sends `{"name":"Snurpo","scenario":"edited-replay"}`. It deliberately targets
an endpoint outside the maintenance matcher.

### Reproduction

1. Launch an isolated native source build and choose **Generate test traffic**.
   This creates six real localhost requests, including the intentional 401 and
   slower route. Leave global request/response breakpoints off.
2. In **Traffic rules**, create and save the maintenance mock described above.
3. Send one independent request through the running proxy:

   ```sh
   curl --noproxy '' --proxy http://127.0.0.1:17890 http://127.0.0.1:17891/api/products
   ```

4. Inspect both an earlier successful products exchange and the new local 503.
   The selected 503 must show its applied rule and local provenance.
5. Select the profile request, choose **Compose replay**, change its method, URL
   and body as described, and explicitly send it. Inspect the saved HTTP 200,
   then reopen its composer on **Body**; cancel after capture.
6. Open **Analysis** on the same session. Additional requests alter its counts
   and timings; do not present them as fixed product performance.

[Machine-readable hashes](snurpo/provenance.json) identify the preserved JPEGs.
The native application title/mascot and normal application-menu Quit were
verified separately. The macOS status-item menu's Show/Quit actions were not
interactively verified by the available automation; these screenshots are not
proof of those actions or of Windows/Linux/Android behavior.

---

# Historical Scope 1.0.0 capture provenance

These are original, unedited captures of the running native macOS Scope 1.0.0 application, taken on November 2, 2026. They show actual engine results and persisted rules, not seeded interface rows or a browser mockup.

Scope was placed in native fullscreen mode to hide the system capture/title strip. Before each capture, the pointer was moved to Finder's native menu outside Scope's captured window and the menu was dismissed. The documented native application screenshot API then saved the original image bytes. No cropping, cursor removal, image generation or retouching was applied.

All displayed traffic is synthetic and local: Scope's fixture on `127.0.0.1:17891`, reached through its proxy on `127.0.0.1:17890`, plus a replay sent by Scope's engine. A fresh QA archive was used. No private app traffic, account credentials, access tokens or private filesystem paths are shown. The Android emulator stayed stopped throughout this capture session.

| Image                                      | What it shows                                                                                                                                                                                                                                                                                |
| ------------------------------------------ | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| [Traffic inspector](traffic-inspector.jpg) | A 15-exchange session from two six-request fixture runs, two separately issued curl requests, and one composed replay. The selected products response is HTTP 503 with its local mock provenance.                                                                                            |
| [Traffic rules](traffic-rules.jpg)         | The persisted, enabled **Product API maintenance** rule: priority 10, host `127.0.0.1`, path prefix `/api/products`, local HTTP 503 response, JSON content type and maintenance body.                                                                                                        |
| [Replay composer](replay-composer.jpg)     | A successfully replayed request reopened in the composer: POST `/api/profile?scenario=composed` with a synthetic JSON body. The prior send returned HTTP 200; the request inspector confirmed `x-scope-lab: composed` and the edited body. The reopened composer was canceled after capture. |
| [Analysis](analysis.jpg)                   | Aggregates for the same 15 exchanges: six observed method/path combinations, intentional HTTP 401 responses, four local mock responses and a deliberately slower endpoint. These timings describe this capture only.                                                                         |

To reproduce:

1. Launch the native desktop application and select **Generate real test traffic**. This starts the localhost fixture and captures real HTTP requests.
2. In **Traffic rules**, save an enabled mock response rule for host `127.0.0.1` and path prefix `/api/products`, status `503`, header `content-type: application/json`, and a synthetic maintenance body.
3. Issue a separate request through the proxy:

   ```sh
   curl --noproxy '' --proxy http://127.0.0.1:17890 http://127.0.0.1:17891/api/products
   ```

4. Inspect the saved exchange and its rule provenance. Select **Compose replay**, change method, URL, headers or body, and explicitly send it. The example POST targets `/api/profile?scenario=composed`, outside the mock's path matcher.
5. Open **Analysis** for the same saved session. Additional fixture runs and proxied requests change the counts and timings.

The screenshot QA also verified session name/notes/tags/pin persistence, rule duplication and priority changes, the metadata-only activity trail, read-only diagnostics with no device connected, and independent MCP scope/permission toggles. All MCP access was restored to zero shared sessions with changes, rules and diagnostics disabled. Capture was stopped and Scope was quit after verification.
