# Scaffold Rollback On GitHub Failure Design

## Summary

`scaffold_project` creates the repo directory, initializes git, writes template files, and commits them before it publishes to GitHub. Without rollback, a failed GitHub publish leaves partial scaffold state behind. That can be a local repo directory after `gh repo create` fails, or both a local directory and an orphaned remote if the GitHub repo is created but the initial push fails.

## Goals

- Make `scaffold_project` transactional with respect to local and remote scaffold state created by the command.
- Preserve the current success path: template files, initial commit, pushed GitHub repo, then saved project entry.
- Keep rollback narrow so we only delete the directory and remote this command created, never a pre-existing user directory or unrelated remote.

## Approach Options

### Option 1: Roll back local and remote state on publish failure

Split publishing into two steps:
- `gh repo create --remote=origin` to create the remote and wire up `origin`
- `git push --set-upstream origin HEAD` to publish the initial commit

If `gh repo create` fails, remove the local directory. If the push fails after the remote exists, delete the remote with `gh repo delete --yes` and then remove the local directory.

Pros:
- Matches the current UX expectation: failed scaffold means no partially created project is left behind.
- Covers both failure points in the publish flow.
- Keeps retry behavior simple.

Cons:
- Requires extra cleanup logic for the remote-delete path.

### Option 2: Persist a recoverable partial project entry

Save a project record before the GitHub step and mark it as incomplete so the app can resume or repair it later.

Pros:
- Preserves work for recovery.

Cons:
- Requires new persisted state, new UI handling, and follow-up repair flows that do not exist today.
- Larger surface area than the issue asks for.

## Recommended Design

Use Option 1. Keep project persistence after the GitHub publish succeeds so failed scaffolds do not write project metadata. On `gh repo create` failure, best-effort delete the newly created local repo directory. On initial push failure, best-effort delete the created GitHub remote first, then remove the local repo directory. Return the original failure message and append cleanup failures when rollback is incomplete.

## Error Handling

- Preserve the original failure message from the step that failed.
- If rollback also fails, return an error that includes both the original failure and the cleanup failure so the user still knows manual cleanup may be required.
- Only attempt remote deletion when a remote was created and can be identified from `origin`.
- Do not attempt rollback for directories that existed before the command; those are rejected before creation.

## Testing

- Add a command-level regression test that forces `gh repo create` to fail and verifies the repo directory is removed and retry works.
- Add a command-level regression test that lets `gh repo create` succeed but forces the initial push to fail and verifies both the local repo and created remote are deleted.
- Verify no project metadata is persisted after either failure mode.
