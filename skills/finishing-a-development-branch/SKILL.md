---
name: finishing-a-development-branch
description: Use when implementation is complete and you need to merge the branch — verifies tests, rebases, creates PR, squash merges, deletes remote branch, syncs local main, and closes the issue
---

# Finishing a Development Branch

## Step 1: Verify Tests

Run the project's test suite. If tests fail, fix them before proceeding.

## Step 2: Execute Merge Workflow

1. Ensure all changes are committed before proceeding
2. Rebase onto `main`
3. Create a PR to `main`
4. Squash merge the PR (without `--delete-branch` to avoid worktree checkout errors):
   ```bash
   gh pr merge --squash
   ```
5. Delete the remote branch:
   ```bash
   git push origin --delete "$(git branch --show-current)"
   ```
6. Sync local main:
   ```bash
   # Find where main is checked out and pull there directly
   main_worktree=$(git worktree list | grep '\[main\]' | awk '{print $1}')
   if [ -n "$main_worktree" ]; then
     git -C "$main_worktree" pull origin main
   else
     git fetch origin main:main
   fi
   ```
7. Close the associated issue with a summary of what was done
8. If running inside The Controller (i.e. `$THE_CONTROLLER_SESSION_ID` is set), signal it to clean up this session's worktree. Syncing main (step 6) may trigger a dev server restart which temporarily kills the socket, so retry for up to 60 seconds:
   ```bash
   if [ -z "$THE_CONTROLLER_SESSION_ID" ]; then
     echo "ERROR: THE_CONTROLLER_SESSION_ID is not set, cannot signal cleanup"
   else
     for i in $(seq 1 30); do
       echo "cleanup:$THE_CONTROLLER_SESSION_ID" | nc -U -w 2 /tmp/the-controller.sock && break
       echo "Waiting for controller socket (attempt $i/30)..."
       sleep 2
     done
   fi
   ```

## Integration

**Called by:**
- **subagent-driven-development** (Step 7) - After all tasks complete
- **executing-plans** (Step 5) - After all batches complete

**Pairs with:**
- **using-git-worktrees** - Cleans up worktree created by that skill
