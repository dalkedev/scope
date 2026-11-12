# Android connection diagnostics

Open **Diagnostics** in the native app and choose **Run read-only checks**. With
one connected ADB device, Snurpo selects it automatically. With several devices,
choose the intended serial and rerun the checks. Snurpo never boots an emulator.
The report is a snapshot; rerun it after changing a connection or setup.

The report inspects ADB availability/version, authorization, shell access,
Android version/API/ABI, root access, optional Frida runtime/server configuration,
current proxy/mappings and Snurpo's saved recovery ownership. Each check has a
status, concrete detail and, where applicable, a suggested fix or setup command.
Commands are advice to review and run explicitly; checks do not execute them.

## Read-only boundary

Checks do not download/install a runtime, copy files to Android, launch an app or
server listener, attach scripts, change proxy/certificate settings, create/remove
ADB mappings, restore settings, or rewrite Snurpo's runtime/recovery files. ADB may
start its ordinary host daemon when queried. A read-only `su -c id` probe can show
the device's normal superuser authorization prompt. Snurpo does not grant root or
change the superuser policy.

Host probes run `adb version` and the selected Python/worker's version query. For
an authorized selected device, the ADB operations are:

```text
adb devices -l
adb -s SERIAL shell id
adb -s SERIAL shell su -c 'id'             # only if shell itself is not root
adb -s SERIAL shell getprop ro.product.cpu.abi
adb -s SERIAL shell getprop ro.build.version.release
adb -s SERIAL shell getprop ro.build.version.sdk
adb -s SERIAL shell settings get global http_proxy
adb -s SERIAL reverse --list
adb -s SERIAL forward --list
adb -s SERIAL shell ps -A -o PID,NAME,ARGS
```

For a configured existing Android server, Snurpo also checks that its validated
path is executable and invokes only `PATH --version`. A local server is inspected
as an ELF file without uploading or executing it. ADB probes have a four-second
timeout; the host runtime version probe has an eight-second timeout. Probes are
partly concurrent, so these are per-probe bounds, not a fixed total duration.
Offline/unauthorized devices are listed without shell or server probes.

## Interpreting readiness

`readyForProxy` means the selected device is authorized, its shell responds and
no active/pending Snurpo session blocks a new start. It does not test the desktop
proxy listener or an app's trust/network configuration. A rootless device can
capture HTTP with Android's device-wide proxy; HTTPS requires appropriate app
trust and can still fail because of pinning.

`readyForFrida` adds confirmed root, an available host runtime, a recognized ABI,
a usable server source and no detected version conflict. It is basic preflight,
not proof that a particular app/OS is supported or all requests will be captured.
An existing USB server also needs a recognized process to be observed.

A process name is evidence that a process exists; it does not verify its port,
permissions or running version. `serverVersionVerified` describes the binary's
actual `--version` output, not a handshake with a running server. A local ELF's
version remains unknown even if its filename contains a version. **Start** checks
its actual device version against the worker before launching Snurpo's listener.

The ABI mappings are `arm64-v8a`, `armeabi-v7a`, `x86`, `x86_64`. A recognized ABI
is not a universal compatibility claim. Frida Java hooks remain experimental;
the earlier short smoke test covered one rooted Android 16 arm64 emulator and
the included demo. ART/GC crashes were observed upstream. Native TLS, QUIC,
child processes and app-specific defenses are not guaranteed. The 1.0 diagnostics
changes were verified using deterministic fixtures, without booting an emulator
or repeating that device smoke test.

## Common fixes

| Check | Action |
| --- | --- |
| ADB unavailable | Install SDK platform-tools, set `ANDROID_HOME`/`ANDROID_SDK_ROOT`, or set `SCOPE_ADB` to its executable. |
| Unauthorized | Unlock the device, accept this computer's USB debugging prompt and rerun checks. |
| Offline / disconnected | Reconnect the same authorized debugging connection. Snurpo does not reconnect/reboot it automatically. |
| No USB permissions | Configure Android udev access on Linux, then reconnect using your normal workflow. |
| Rootless | Choose system proxy mode for HTTP, or use an authorized rooted lab device and grant Shell access. |
| Root unknown / timed out | Check the superuser prompt and connectivity, then rerun. Unknown does not mean confirmed rootless. |
| Optional runtime missing | Review the report's command for the actual runtime path and selected ABI. Run the setup helper explicitly; see [Android setup](ANDROID.md#recommended-user-side-setup). |
| ABI mismatch | Choose an Android ELF server matching the reported ABI. Start rejects a wrong local architecture before pushing it. |
| Version mismatch | Use exact matching worker/server versions. Remove conflicting `SCOPE_FRIDA_*` overrides or rerun managed setup. |
| Port 24073 occupied | Stop your own listener or select an existing default-port USB Frida server. Snurpo refuses to replace an unrelated listener. |
| Cleanup pending | Reconnect the recorded serial and press Stop. Do not delete recovery state as a substitute for restoring device settings. |
| Invalid recovery state | Inspect the reported error and recovery file before proceeding. Invalid persisted fields are rejected before cleanup commands. |

The optional runtime remains a separate explicit installation. Diagnostics does
not weaken certificate trust, disable pinning or perform the setup for you.

## Saved state and ownership

`recovery` distinguishes active state, valid saved state requiring cleanup and
invalid state. `mappings.owned` includes only exact selected-device mappings that
match recorded Snurpo ports. Other devices' forward mappings are excluded. An
unrelated reverse mapping, such as a demo-server port, is not marked owned.

Stop checks current proxy/forward/reverse values before restoring them. If a
value was changed outside Snurpo, it preserves the current value and logs that
decision. A missing mapping already restored elsewhere is harmless. Failed
inspection or cleanup retains the recovery file for a retry. These ownership
checks do not claim to identify which external process last changed a value.

If a worker exits or detaches, Snurpo stops claiming capture is running and keeps
the recorded state for Stop. Notifications from an old worker cannot overwrite
a subsequent session's status.

## CLI, JSON and MCP

From a source checkout:

```sh
cargo run -p scope-device --example device -- diagnostics
cargo run -p scope-device --example device -- diagnostics YOUR_SERIAL
```

The example uses a separate temporary smoke runtime by default. Set
`SCOPE_DEVICE_RUNTIME` to the device-runtime directory you intend to inspect if
you need its recovery/managed setup state. This is an inspection operation.

The native command and MCP tool are both `device_diagnostics`, with optional
`serial`. MCP requires Snurpo's separate **Device diagnostics** permission;
session sharing or write permission does not enable it. Results can contain
local paths, device serials, package IDs, versions and proxy addresses. Review a
copied JSON report before sharing it publicly. See [MCP access](MCP.md).

The camelCase result contains `checkedAt` (Unix milliseconds), `readOnly`,
`selectedSerial`, `readyForProxy`, `readyForFrida`, `adb`, `devices`, `device`,
`frida`, `mappings`, `recovery`, `checks`, and `notes`. Missing ordinary tools or
connections are structured checks, not top-level failures. An invalid serial is
rejected before any probe. Check status is `ok`, `warning`, `error`, or `info`.
Nullable values mean unknown/not checked, rather than a confirmed false result.

Fixtures in `crates/scope-device/tests/fixtures/diagnostics.json` drive the same
collector used by native diagnostics. Tests assert exact allowed read commands,
missing/unauthorized/offline/permission states, root denial/timeouts, ABI/version
conflicts, no host runtime writes, and exact recovery ownership. Separate stream
tests exercise worker EOF/detach and stale-session notifications; a Unix fake ADB
test checks the actual Stop command path and preservation of external changes.
