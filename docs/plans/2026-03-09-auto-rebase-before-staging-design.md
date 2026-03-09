# Auto-rebase Before Staging

## Problem

When a worktree branch is behind master, staging checks out old code in the main repo. If that old code predates the unstaging feature, pressing `v` to unstage doesn't work.

## Solution

Auto-rebase the worktree branch onto main before staging. Use Claude to resolve conflicts if needed.

## Flow

In `stage_session_inplace` (commands.rs), before calling `WorktreeManager::stage_inplace()`:

1. **Check worktree is clean.** If uncommitted changes exist, send a prompt to the session's Claude ("commit your changes") and return an error blocking staging.
2. **Check if branch is behind main.** Compare worktree branch tip against main branch tip using `merge_base`.
3. **If behind, rebase.** Run `git rebase <main>` in the worktree directory.
4. **If rebase succeeds**, proceed with normal staging.
5. **If rebase has conflicts**, send a prompt to Claude via PTY to resolve them. Return an error blocking staging ("Rebasing — resolve conflicts and try staging again").
6. **If already up to date**, proceed with normal staging (current behavior).

## Changes

- `stage_session_inplace` in `commands.rs` — add rebase logic before `WorktreeManager::stage_inplace()`, send prompts via PTY when needed.
- `worktree.rs` — add helpers: check if worktree is clean, check if branch is behind main, run rebase.
- Frontend unchanged — errors show as toasts, user retries with `v`.
