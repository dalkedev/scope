# Install Snurpo

Download Snurpo 1.5.0 from the [published GitHub release](https://github.com/dalkedev/snurpo/releases/tag/v1.5.0). The desktop packages include the capture engine and companion MCP executable. Node.js, Rust and a model account are not required to run them.

## Choose your download

| Computer | Package |
| --- | --- |
| macOS, Apple Silicon | [Snurpo-v1.5.0-macOS-arm64.zip](https://github.com/dalkedev/snurpo/releases/download/v1.5.0/Snurpo-v1.5.0-macOS-arm64.zip) |
| Windows, x64 | [Snurpo-v1.5.0-Windows-x64-setup.exe](https://github.com/dalkedev/snurpo/releases/download/v1.5.0/Snurpo-v1.5.0-Windows-x64-setup.exe) |
| Linux, x64 | [AppImage](https://github.com/dalkedev/snurpo/releases/download/v1.5.0/Snurpo-v1.5.0-Linux-x64.AppImage) or [Debian package](https://github.com/dalkedev/snurpo/releases/download/v1.5.0/Snurpo-v1.5.0-Linux-x64.deb) |

There is no Intel macOS or Linux arm64 package in this release. Windows and Linux packages have CI build and headless protocol coverage; interactive installers, desktop use and device behavior on those hosts remain unverified. Native interaction has been exercised on macOS Apple Silicon.

## Verify and open

Compare your file's SHA-256 with [SHA256SUMS.txt](https://github.com/dalkedev/snurpo/releases/download/v1.5.0/SHA256SUMS.txt). For example, on macOS:

```sh
shasum -a 256 Snurpo-v1.5.0-macOS-arm64.zip
```

On Windows, use `Get-FileHash -Algorithm SHA256` in PowerShell; on Linux, use `sha256sum`.

On macOS, extract the ZIP, move **Snurpo.app** into Applications and open it. The app is ad-hoc signed, not Apple-notarized. After verifying the download, use **System Settings > Privacy & Security > Open Anyway** if macOS blocks the first launch. Keep Gatekeeper enabled.

On Windows, run the x64 installer. On Linux, use the package appropriate for your distribution. An AppImage needs executable permission:

```sh
chmod +x Snurpo-v1.5.0-Linux-x64.AppImage
./Snurpo-v1.5.0-Linux-x64.AppImage
```

Linux has a tracked glib dependency advisory; see the [dependency record](https://github.com/dalkedev/snurpo/blob/v1.5.0/docs/DEPENDENCIES.md) for its status and boundaries.

## Updating an existing installation

Quit the old app before opening the new one. Scope and Snurpo share the same archive and CA; do not run both at once or rename their data files. Back up important captures before an upgrade. [Compatibility details](releases.md).

Optional Frida binaries are not bundled. You only need them for the [Frida Android workflow](android.md), not ordinary desktop capture.

Continue with [your first capture](first-capture.md).
