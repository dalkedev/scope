# Maintainer review and releases

Snurpo is maintained by [dalkedev](https://github.com/dalkedev).
Every source change goes through a pull request. The default branch requires
one approving review, dismisses stale approvals after new commits, requires
resolved review conversations and successful checks on macOS, Windows and Linux.
The rule also applies to administrators. Force pushes and branch deletion are
disabled. `.github/CODEOWNERS` assigns the maintainer to every path.

## Reviewing a release proposal

Use a `release/<version>` branch, such as `release/1.4.1`. Dispatch
**Prepare review or draft release** on that branch with `mode: proposal` and
the matching `tag: v<version>` to open
a draft PR authored by `github-actions[bot]`. The workflow only opens a PR; it
never submits an approving review, merges, tags or publishes. The maintainer
can review the bot-authored PR using GitHub's normal **Approve** action.
The tag is validated before selecting the committed
`docs/releases/<version>-review.md` proposal body. Proposal mode does not require
or create the future version tag.

The repository's Actions setting permits PR creation by the workflow token.
The token has read access by default; only the proposal job receives
`pull-requests: write`. This setting does not authorize automated approval in
Snurpo's workflow. Commits retain their actual Git authorship.

CI also runs on `release/**` and `docs/**` pushes: events generated with `GITHUB_TOKEN` do not
trigger ordinary PR workflows, so the proposal relies on checks from that exact
branch commit. `require_last_push_approval` is disabled because the maintainer
pushes the code and must still be able to review the bot-authored PR. Stale
approvals are dismissed after new commits.

CODEOWNERS is evaluated from the PR's base branch. On the first proposal that
adds it, the required ordinary approval and the repository's sole write-capable
maintainer provide the review gate. Subsequent PRs also use CODEOWNERS from main.

After review, the maintainer chooses when to merge and create a version tag.
The draft mode builds an existing tag into a private draft prerelease. Publishing
that draft is a separate maintainer action. A version number in source is not
evidence of a published or platform-validated release.

## Reviewing website documentation

Use a `docs/<topic>` branch and commit the review summary in
`docs/website/REVIEW.md`. Dispatch **Prepare review or draft release** on that
branch with `mode: documentation`. The tag input is unused in this mode.
The workflow validates the branch and opens a draft PR as `github-actions[bot]`,
using the committed review summary. If an open PR already exists for that
branch, it reports that PR instead of creating another one.

Documentation mode does not run release builds, create tags or releases,
approve, or merge. The maintainer reviews the changes, marks the PR ready and
approves it normally. The same code-owner, resolved-conversation and exact-commit
macOS, Windows and Linux check requirements apply before merge. A new commit
dismisses the approval and requires review again.

See [GitHub protected branch behavior](https://docs.github.com/en/repositories/configuring-branches-and-merges-in-your-repository/managing-protected-branches/about-protected-branches),
[CODEOWNERS behavior](https://docs.github.com/en/repositories/managing-your-repositorys-settings-and-features/customizing-your-repository/about-code-owners)
and [workflow event restrictions](https://docs.github.com/en/actions/how-tos/write-workflows/choose-when-workflows-run/trigger-a-workflow).
