---
name: finishing-a-development-branch
description: Use when implementation is complete and you need to merge the branch — verifies tests, rebases, creates PR, squash merges, deletes remote branch, syncs local master, and closes the issue
---

# Finishing a Development Branch

## Step 1: Verify Tests

Run the project's test suite. If tests fail, fix them before proceeding.

## Step 2: Execute Merge Workflow

1. Ensure all changes are committed before proceeding
2. Rebase onto `master`
3. Create a PR to `master`
4. Squash merge the PR and delete the remote branch:
   ```bash
   # Use --squash and --delete-branch, but do NOT let gh try to checkout master locally
   gh pr merge --squash --delete-branch
   ```
   **Worktree note:** If this errors with "'master' is already used by worktree", the merge still succeeded on GitHub. The error is just the local checkout step — ignore it and proceed to step 5.
5. Sync local master:
   ```bash
   git fetch origin master:master
   # If master is checked out in another worktree, update that working tree too
   master_worktree=$(git worktree list | grep '\[master\]' | awk '{print $1}')
   if [ -n "$master_worktree" ]; then
     git -C "$master_worktree" reset --hard master
   fi
   ```
6. Close the associated issue with a summary of what was done

## Integration

**Called by:**
- **subagent-driven-development** (Step 7) - After all tasks complete
- **executing-plans** (Step 5) - After all batches complete

**Pairs with:**
- **using-git-worktrees** - Cleans up worktree created by that skill
