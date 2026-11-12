# HAR comparison CI

`.github/workflows/har-compare.yml` runs for pushes and pull requests that change
`tools/har-compare/**` or the workflow itself. It is separate from the desktop
build and release workflows. It requests only `contents: read`, persists no Git
credentials, and uses commit-pinned official actions and exact Node.js 26.13.1.

The job runs on a clean GitHub-hosted Ubuntu 22.04 runner as its non-root user.
Only the lab's locked QA dependencies are installed with
`npm ci --ignore-scripts --prefix tools/har-compare/lab`; the desktop dependency
tree is not installed. jsdom 26.1.0 is resolved from that lab package by the DOM
checker. Playwright's locked local CLI installs its matching Chromium build and
runner libraries. No unpinned `npx` package is executed.

The required checks are:

- The offline tool and lab unit tests.
- `examples/run-demo.mjs` and `examples/sprint/run.mjs`, including their expected
  regression exit statuses and deterministic report comparisons.
- `examples/check-html-dom.mjs`, using a generated hostile synthetic report.
- `lab/browser-check.mjs chromium`, using `lab/browser-runtime.mjs` with
  `chromiumSandbox: true`. An unsafe control must execute and attempt a request;
  the escaped report must do neither. The browser must render the hash-authorized
  stylesheet, block injected inline script and unhashed CSS under the report's
  CSP, and support local navigation and details expansion.

Browser launch failures, unavailable sandboxing, missing binaries, and assertion
failures fail CI. There is no successful limitation fallback, sandbox-disabling
flag, or host security policy change. The browser installation may use the hosted
runner's ordinary package installer for required system libraries.

The artifact allowlist contains only named synthetic demo reports, hostile-report
HTML, the two hostile-report screenshots and the browser assertion receipt.
It excludes HAR files, captured traffic, raw browser profiles, detector-control
HTML, and the rest of the lab output directory. Artifacts are uploaded only after
all checks pass and retained for three days. Screenshots are produced by browser
assertions; the workflow does not claim a human visual review.

An active workflow file is not evidence of a successful hosted run. Confirm the
Actions result for the exact pushed commit before reporting remote CI as passed.
The older `examples/github-actions.yml` remains a reusable integration example.
