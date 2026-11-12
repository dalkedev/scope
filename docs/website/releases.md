# Releases and compatibility

These guides describe **Snurpo 1.5.0**, published on 12 November 2026. Use the [release page](https://github.com/dalkedev/snurpo/releases/tag/v1.5.0) for downloads and checksums, or [all releases](https://github.com/dalkedev/snurpo/releases) for the full history.

## What 1.5.0 adds

**Copy as cURL** carries an effective captured request into a POSIX terminal. You preview it before copying; neither action sends traffic. Repeated headers and text or binary bodies are supported within the documented limits. [Usage and boundaries](replay-curl.md).

The installed desktop app needs no Node runtime. Source tooling requires Node 26.13.1 or newer. The separate HAR comparison CLI keeps its own `0.1.0` version.

## Download availability and validation

| Target | Current evidence |
| --- | --- |
| macOS Apple Silicon | Native interaction has been exercised. Public ZIP is ad-hoc signed, not Apple-notarized. |
| Windows x64 | Installer available; CI build/package and headless MCP/capture coverage. Interactive use remains unverified. |
| Linux x64 | AppImage and Debian package available; CI build/package and headless MCP/capture coverage. Interactive use remains unverified. |
| Intel macOS / Linux arm64 | No package in the 1.5.0 release. |
| Android / Frida | Experimental coverage for the specific setup described in the [Android guide](android.md). |

The gallery's existing native screenshots record **1.4.2**. They are not screenshots of the 1.5.0 cURL feature. Detailed [verification](https://github.com/dalkedev/snurpo/blob/v1.5.0/docs/VERIFICATION.md) and [dependency records](https://github.com/dalkedev/snurpo/blob/v1.5.0/docs/DEPENDENCIES.md) retain their own dates and limits.

## Scope became Snurpo in 1.4.2

The name and mascot changed; saved-data and integration identifiers remain compatible. Existing sessions, CA trust and MCP configuration do not need a branding migration.

Keep existing `scope.sqlite3`, `scope-ca.pem`, `scope-ca-key.pem`, `.scope.json`, `SCOPE_*` variables and `scope-mcp` configurations. The certificate subject can still say Scope. Do not rename or remove these files merely to update the branding.

Quit old Scope before opening Snurpo: both use the same local archive and ports. Keep a backup of important work before changing installations. Historical releases and checksums remain unchanged.

On macOS, the mascot menu offers **Show Snurpo** and **Quit Snurpo**. Quit uses the application's cleanup path. This does not introduce automatic startup or change capture settings.

Read the [migration reference](https://github.com/dalkedev/snurpo/blob/v1.5.0/docs/MIGRATING_TO_SNURPO.md) for all retained compatibility names.
