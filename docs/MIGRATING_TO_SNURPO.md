# Scope → Snurpo

Scope is now **Snurpo — A nosy little HTTP debugger.** The application name,
mascot, window title, documentation and future installer names change. Existing
sessions, certificates, integrations and data formats keep their compatibility
identifiers. This is a brand change, not a data migration.

The project repository is `https://github.com/dalkedev/snurpo`. Existing clones
can update their remote with:

```sh
git remote set-url origin https://github.com/dalkedev/snurpo.git
```

## Existing installations

The latest published release at the time of the rename is **Scope 1.4.1**. Those
release tags and downloadable assets are historical and remain unchanged. Source
version 1.4.1 is not a claim that a Snurpo-branded 1.4.1 installer was published.
The next release must use a new version and publish its own verified artifacts.

Snurpo retains the desktop application identifier `dev.scope.debugger` and
`ProjectDirs::from("dev", "scope", "Scope")`. It therefore reads the same local
data directory, `scope.sqlite3`, `scope-ca.pem`, `scope-ca-key.pem` and bridge
configuration. **Do not rename or delete these files to match the new brand.**
Your existing CA remains valid; its subject can still display
`Scope Local Debug CA` with organization `Scope`. This is intentional, including
for newly generated certificates, so device instructions and trust lookups remain
compatible. An app rename does not require reinstalling the CA.

New macOS bundles are called `Snurpo.app`. The old `Scope.app` bundle may still be
present after a manual install. Keep one installed copy and do not run both at
once: they use the same database, proxy settings and local bridge. Platform
installer upgrade behavior must be checked as part of the next release; a name
change alone is not proof of an in-place installer upgrade.

On macOS, a small template mascot in the menu bar offers **Show Snurpo** and
**Quit Snurpo**. Show brings the application window forward, including from a
minimized state.
Quit uses the existing device/proxy cleanup path. Window-close behavior, automatic
startup and capture settings are unchanged. This menu is macOS-only.

## Compatibility names that remain

| Identifier | Reason it stays |
| --- | --- |
| `scope-mcp`, MCP configuration key `scope` | Existing MCP client commands and configuration continue to work. |
| `scope-har-compare` | Existing local HAR CLI commands remain an alias; new links can use `snurpo-har-compare`. |
| `scope_command`, Rust `scope-*` packages and desktop `scope` binary | Existing IPC, developer commands and package integration remain compatible. |
| `SCOPE_*` environment variables and `scope-theme` | Existing automation, runtime configuration and theme preference remain readable. |
| `scope-session`, `.scope.json`, HAR `_scope`, OpenAPI `x-scope` | Saved archives and exported evidence retain the same versioned formats. |
| `ScopeJavaEngine`, `scope-java-v1`, `dev.scope.runtime.*`, `scope-hooks-*` | Frida bridge and runtime ownership/cleanup remain compatible. |
| Android `/data/local/tmp/scope-*` and `scope-ca.pem` | Existing device certificate/proxy cleanup continues to find the files it owns. |
| `X-Scope-Demo`, `X-Scope-Evidence` | Existing diagnostic fixtures and evidence headers retain their meaning. |

The lowercase word “scope” also remains where it describes a permission, access
boundary or filtering concept. Historical release notes, test receipts, screenshots
and their provenance retain the name they actually recorded.

The executable remains `scope-mcp`; clients do not need a new configuration just
because the desktop application is called Snurpo. See [MCP.md](MCP.md) for the
current commands and [BRAND.md](BRAND.md) for the new artwork.
