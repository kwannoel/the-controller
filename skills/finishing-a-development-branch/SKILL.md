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
   # Find where master is checked out and pull there directly
   master_worktree=$(git worktree list | grep '\[master\]' | awk '{print $1}')
   if [ -n "$master_worktree" ]; then
     git -C "$master_worktree" pull origin master
   else
     git fetch origin master:master
   fi
   ```
6. Close the associated issue with a summary of what was done
7. If running inside The Controller (i.e. `$THE_CONTROLLER_SESSION_ID` is set), signal it to clean up this session's worktree:
   ```bash
   echo "cleanup:$THE_CONTROLLER_SESSION_ID" | nc -U -w 2 /tmp/the-controller.sock 2>/dev/null; true
   ```

## Integration

**Called by:**
- **subagent-driven-development** (Step 7) - After all tasks complete
- **executing-plans** (Step 5) - After all batches complete

**Pairs with:**
- **using-git-worktrees** - Cleans up worktree created by that skill
