# GitHub Actions maintenance

This repo locks every GitHub Action its workflows use, so CI runs only code that someone reviewed
in a PR. This guide covers what that means, and what to do when you add an action, review a
Dependabot PR, or see the lockfile check fail.

## Why

Workflows run third-party actions with access to the repo's token, and release workflows also
have publish credentials. A tag like `v4` is a pointer that the action's owner, or anyone who takes
over their account, can move to different code. `.github/workflows/actions.lock` records the exact
commit behind every action, including the actions those actions call. GitHub runs those commits
and nothing else, and any change to them shows up as a diff you can review.

The lockfile doesn't cover:

- reusable workflows called at job level (`jobs.<id>.uses`). Keep those pinned to a full commit
  SHA with a `# vX.Y.Z` comment.
- container images. Pin those by `@sha256:` digest.
- anything an action downloads while it runs.
- the runner image.

`joshka/betamax-action` stays pinned to a full commit SHA because no release tag points at the
commit CI uses. The lockfile still records and verifies that commit.

## How CI gates merges

`.github/workflows/ci.yml` ends with one job named `required`, and that is the only
check that branch protection requires. It depends on every job that must pass, and it fails if
any of them failed, was cancelled, or was skipped. To make a new job block merges, add it to
`required`'s `needs:`. You don't need to change any repo settings.

The `lockfile` job runs `gh actions-lock --verify`. It fails when the lockfile doesn't match the
workflows, or when a locked commit no longer checks out upstream.

## Setup

```sh
gh extension install github/gh-actions-lock --pin v0.4.1
```

Use the same version that the `lockfile` job pins, because a newer or older version may write a
different lockfile.

## Common tasks

### Add or change an action

1. Edit the `uses:` line. Use an exact release tag (`owner/action@v1.2.3`) when the action
   publishes one. Use a branch only when the action's README says to (for example
   `dtolnay/rust-toolchain@master` together with its `toolchain:` input). Don't use major-only tags
   like `@v2`.
2. Run `gh actions-lock`. It writes the new entry and doesn't change the existing ones.
3. Commit the workflow and `actions.lock` together. In the PR, say why you chose that ref.

### Review a Dependabot PR

Dependabot updates the workflow and `actions.lock` in the same PR. Before merging, check:

- the release notes for the new version.
- that only the commit for the action being bumped changed in `actions.lock`.
- that `owner_id` and `repo_id` didn't change. If they did, the repository behind that name is a
  different one, so investigate before merging.

### Refresh a branch ref

Branch refs such as `@master` never move on their own. To update one:

```sh
gh actions-lock --relock .github/workflows/<file>.yml
```

`--relock` re-resolves every ref in the files you name. Review every commit that changes, not
just the one you meant to update.

### The lockfile check failed

Run `gh actions-lock --verify` locally. Each finding links to its fix.

| Finding                                              | Do                                                                                      |
| ---------------------------------------------------- | --------------------------------------------------------------------------------------- |
| `not-pinned`, `stale`, `ref-changed`, `repo-renamed` | Run `gh actions-lock` and commit.                                                       |
| `tag-commit-mismatch`, `incomplete-lockfile`         | Read the upstream diff, then run `--relock` on the affected workflow.                   |
| `reachability-unknown`, `ancestry-unknown`           | GitHub didn't respond. Re-run the job.                                                  |
| `unreachable-pin`, `misleading-sha`, `repo-replaced` | **Stop.** Either upstream history was rewritten or this is an attack. Ask a maintainer. |

Warnings such as `ref-moved` don't fail the check.

## Never

- Edit `actions.lock` by hand.
- Delete it to make an error go away. Regenerating it from scratch accepts whatever every tag
  points at today, and the record of what you reviewed is lost.
- Run `--accept-moved` in CI or a script. Run it only after a person has checked why the commit
  moved.

To undo a bad lockfile change, restore the file from the default branch and run
`gh actions-lock` again:

```sh
jj restore --from 'trunk()' .github/workflows/actions.lock     # jj
git checkout origin/HEAD -- .github/workflows/actions.lock     # git
```

## More

- [gh-actions-lock](https://github.com/github/gh-actions-lock) and its
  [troubleshooting guide](https://github.com/github/gh-actions-lock/blob/main/docs/troubleshooting.md)
- [Workflow dependency lockfiles](https://docs.github.com/en/actions/concepts/security/workflow-dependency-lockfiles)
