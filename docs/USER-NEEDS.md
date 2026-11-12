# Public issue research

Reviewed 2026-11-02. These are individual public reports and requests, not a
representative user survey, verified defects in another product, or a claim that
Snurpo already handles every case. No upstream code was copied for this research.
The links preserve context and maintainer responses.

| Observed need                                                     | Public source                                                                                                                                                          | Snurpo response and acceptance evidence                                                                                                                                                                                                               |
| ----------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Large HAR exports need predictable memory use and visible failure | [HTTP Toolkit #935](https://github.com/httptoolkit/httptoolkit/issues/935), [#936](https://github.com/httptoolkit/httptoolkit/issues/936)                              | 1.0 keeps free local archives/export and explicit per-body limits. Streaming export, cancellation, size estimates and a large-capture memory benchmark remain planned; existing in-memory export does not solve this report.                         |
| Requests from identical devices need reliable attribution         | [#932](https://github.com/httptoolkit/httptoolkit/issues/932)                                                                                                          | Plan per-capture source identity retained in native/HAR export, with tests using two identical app/device fixtures. Current device-wide proxy mode cannot prove a request's originating app or device.                                               |
| Agents should automate useful debugging tasks                     | [#922](https://github.com/httptoolkit/httptoolkit/issues/922)                                                                                                          | 1.0 adds scoped OpenAPI inference, edited replay, separately gated global rules and read-only preflight. Browser/UI driving with Playwright/uiautomator is a separate future proposal; no arbitrary-shell MCP tool is included.                      |
| Users want their own instrumentation scripts                      | [#671](https://github.com/httptoolkit/httptoolkit/issues/671)                                                                                                          | Snurpo offers explicit bundled hook profiles. A custom script registry needs trust boundaries, license/provenance checks and recovery coverage before it can be shipped.                                                                              |
| Frida compatibility failures need exact environment evidence      | [HTTP Toolkit Desktop #113](https://github.com/httptoolkit/httptoolkit-desktop/issues/113), [HTTP Toolkit #665](https://github.com/httptoolkit/httptoolkit/issues/665) | 1.0's read-only preflight reports root/ABI/runtime/server/version evidence separately. A successful preflight is not proof that a target's ART/TLS stack is supported. Physical-device and version-matrix testing remain required.                   |
| Replaying/editing headers must preserve request semantics         | [#902](https://github.com/httptoolkit/httptoolkit/issues/902)                                                                                                          | Snurpo preserves repeated header lists, normalizes Host/Content-Length and removes stale encoding/digest headers after body replacement. Real HTTP/TLS tests cover edits and binary bodies; capture is normalized, not a byte-for-byte wire recorder. |

## Next experiments

1. Generate a synthetic capture with 10,000 exchanges and mixed binary/JSON
   bodies. Measure peak RSS, export duration and output integrity. Streaming
   export must complete within a documented memory budget and support cancellation.
2. Add explicit source identity before claiming multi-device capture. Verify it
   survives session reopen, native export/import and HAR `_scope` metadata.
3. Try an MCP workflow against the local demo: find an error, cite exchange IDs,
   create a disabled mock, explicitly enable it, then remove it. Reads and each
   mutation must obey the desktop's separate access controls.
4. Record Android/Frida failures by host, Android SDK, ABI, runtime/server/bridge
   versions and cleanup outcome; share a reviewed diagnostic report without
   embedding tokens, app traffic or private paths by default.

See [roadmap](../ROADMAP.md) for ordering and [verification](VERIFICATION.md) for
checks that actually ran. The current implementation and its tests remain the
source of truth for Snurpo's behavior.
