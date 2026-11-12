# Keep the evidence

Snurpo saves captured exchanges to its local SQLite archive. Stopping capture or reopening the app does not erase the session. **Connection settings > Session archive** shows the actual data directory.

## Organize a session

Open **Session details** beside the session name. Give the investigation a name, add notes and comma-separated tags, or pin it for quick access.

Deleting a session is a separate confirmation that requires its name. It removes captured messages and notes. Export a copy first if you need to keep the evidence; an active capture cannot be deleted.

## Choose an export

Open **Export**, select the content policy, then the format:

| Format | Use it for |
| --- | --- |
| **HAR archive** | Moving HTTP evidence to other tools or the offline comparison CLI. |
| **Snurpo session** | Preserving native session information, including originals and edits when retained by the content policy. |
| **Observed OpenAPI** | Exploring the API shape visible in this session. |

**Sanitized** is selected by default in the desktop export menu. It masks recognized credential fields and creates a lossy sharing copy. **Raw capture** retains captured headers and bodies, including credentials. Review custom fields in either case before sharing.

The native format still uses `.scope.json` and compatibility identifiers from earlier releases. Do not rename internal schema keys to match the product name.

## Import and compare

Choose **Import session or HAR** in the sidebar and select the archive. Snurpo opens the imported session so you can inspect it or select it under **Compare sessions**.

HAR tools differ in what they capture. Missing bodies, timings or original-message details cannot be reconstructed by importing the file. Compare observations with that limitation in mind.

## Back up useful work

Use a raw native export when you need to preserve a private investigation, and keep that file private. For a complete application-data backup, stop capture and quit Snurpo first; the directory also contains sensitive CA and bridge material.

Records currently load into memory. Large-capture streaming import and export are not implemented. Bodies are limited to 16 MiB each. Keep investigations focused instead of assuming an unlimited archive can be opened cheaply.

Next: [understand a session](analysis.md) or [compare HARs in CI](har-compare-ci.md).
