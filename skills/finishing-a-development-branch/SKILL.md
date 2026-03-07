---
name: finishing-a-development-branch
description: Use when implementation is complete and you need to merge the branch — verifies tests, rebases, creates PR, squash merges, deletes remote branch, syncs local master, and closes the issue
---

# Finishing a Development Branch

## Step 1: Verify Tests

Run the project's test suite. If tests fail, fix them before proceeding.

## Step 2: Execute Merge Workflow

1. Rebase onto `master`
2. Create a PR to `master`
3. Squash merge the PR (include commit messages inline in the squash message)
4. Delete the remote branch
5. Sync local master: `git checkout master && git pull`
6. Close the associated issue with a summary of what was done

## Integration

**Called by:**
- **subagent-driven-development** (Step 7) - After all tasks complete
- **executing-plans** (Step 5) - After all batches complete

**Pairs with:**
- **using-git-worktrees** - Cleans up worktree created by that skill
