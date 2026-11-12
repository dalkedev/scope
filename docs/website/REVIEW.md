# Canonical Snurpo website documentation

Add 14 source-backed guides and a navigation manifest for
[Snurpo documentation](https://snurpo.dev/docs/), plus a Docs link in the
repository README. The guides cover installation, capture and HTTPS, search,
rules, replay/cURL, sessions, analysis, stdio MCP, experimental Android support,
HAR comparison and release compatibility.

The copy uses current GUI labels, verified 1.5.0 download assets and explicit
platform and permission boundaries. No application code, dependency or version
changes are included.

Extend the existing bot-authored review workflow with a documentation-only
proposal mode and exact-commit CI on `docs/**` pushes. This mode only opens a
draft PR. It does not approve, merge, create tags, run release builds or publish.
Existing code-owner approval and all required checks remain in force.

## Validation

- Checked the 14-page navigation manifest and all internal links, balanced code
  fences and absence of raw HTML or U+2014.
- The website renderer passed 14-page and 56-viewport checks, including search
  and copy controls.
- Installation, releases and MCP received source and rendered-page review.
  Independent review also checked MCP, Android, analysis and replay/cURL against
  the implementation and current GUI labels, with no blocking findings.
- Exercised the workflow's proposal shell in isolated fixtures, including
  release and documentation modes, invalid inputs and existing-PR handling.
- Required macOS, Windows and Linux checks must pass on the reviewed commit
  before merge.
