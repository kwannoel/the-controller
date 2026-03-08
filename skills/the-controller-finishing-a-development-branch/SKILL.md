---
name: the-controller-finishing-a-development-branch
description: Use when implementation is complete and you need to merge the branch — verifies tests, rebases, creates PR, squash merges, deletes remote branch, syncs local master, and closes the issue
---

# Finishing a Development Branch

## Step 1: Verify Tests

Run the project's test suite. If tests fail, fix them before proceeding.

## Step 2: Execute Merge Workflow

1. Ensure all changes are committed before proceeding
2. Rebase onto `master`
3. Create a PR to `master`
4. Squash merge the PR (without `--delete-branch` to avoid worktree checkout errors):
   ```bash
   gh pr merge --squash
   ```
5. Delete the remote branch:
   ```bash
   git push origin --delete "$(git branch --show-current)"
   ```
6. Sync local master:
   ```bash
   # Find where master is checked out and pull there directly
   master_worktree=$(git worktree list | grep '\[master\]' | awk '{print $1}')
   if [ -n "$master_worktree" ]; then
     git -C "$master_worktree" pull origin master
   else
     git fetch origin master:master
   fi
   ```
7. Close the associated issue with a summary of what was done
8. If running inside The Controller (i.e. `$THE_CONTROLLER_SESSION_ID` is set), signal it to clean up this session's worktree. Retry up to 3 times if the signal fails:
   ```bash
   if [ -z "$THE_CONTROLLER_SESSION_ID" ]; then
     echo "ERROR: THE_CONTROLLER_SESSION_ID is not set, cannot signal cleanup"
   else
     for i in 1 2 3; do
       echo "cleanup:$THE_CONTROLLER_SESSION_ID" | nc -U -w 2 /tmp/the-controller.sock && break
       echo "Cleanup signal failed (attempt $i/3), retrying in 2s..."
       sleep 2
     done
   fi
   ```

## Integration

**Called by:**
- **the-controller-subagent-driven-development** (Step 7) - After all tasks complete
- **the-controller-executing-plans** (Step 5) - After all batches complete

**Pairs with:**
- **the-controller-using-git-worktrees** - Cleans up worktree created by that skill
