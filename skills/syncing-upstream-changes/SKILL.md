---
name: syncing-upstream-changes
description: Use when this fork needs to keep a local mirror of kwannoel/the-controller main and merge upstream changes into the current main branch without losing track of what came from upstream
---

# Syncing Upstream Changes

Upstream remote: `upstream` → `https://github.com/kwannoel/the-controller.git`
Upstream default branch: `main`

## Workflow

1. Fetch upstream:
   ```bash
   git fetch upstream main
   ```
2. Compare local `main` against upstream before merging:
   ```bash
   git rev-list --left-right --count main...upstream/main
   git log --oneline main..upstream/main
   git diff --stat main..upstream/main
   ```
3. Review the upstream-only commits. Inspect both commit summaries and the actual diff so you know what areas changed and what follow-on breakage is plausible:
   ```bash
   git diff main..upstream/main
   ```
4. Merge upstream into `main` (MUST use `--no-ff`, NEVER squash):
   ```bash
   git checkout main
   git merge --no-ff upstream/main -m "sync: merge upstream kwannoel/the-controller up to #<highest-PR-number>"
   ```
5. Run verification — at minimum:
   ```bash
   cd src-tauri && cargo test --features server
   cd src-tauri && cargo clippy --features server -- -D warnings
   cd src-tauri && cargo fmt --check
   pnpm check
   pnpm test
   ```
6. Pay special attention to the `server_routes_cover_desktop_command_surface` test — it enforces 1:1 parity between Tauri desktop commands in `lib.rs` and HTTP routes in `server/main.rs`. If upstream adds new `#[tauri::command]` functions, you MUST add matching routes and handlers in the server.
7. Review for cross-cutting fallout. Upstream changes can merge cleanly and pass a narrow test while still causing problems elsewhere — check nearby workflows, prompts, or UI paths that depend on the touched code.
8. Push the updated `main` to `origin`:
   ```bash
   git push origin main
   ```

## Critical Rules

- **NEVER use squash merge for upstream sync.** Squash merge destroys the parent linkage that lets git track which upstream commits are already merged. Next sync will re-list all previously synced commits as "behind", making it impossible to know where to start. Always use `--no-ff` merge to preserve upstream commit history as a merge parent.
- If merge conflicts appear, resolve them on `main`. When a previous sync already resolved the same conflicts (e.g. in a squash commit), you can use `git checkout <previous-resolved-commit> -- <file>` to recover the correct resolution.
- Do not treat a clean merge or a passing narrow test as sufficient review. Always inspect what upstream changed and think about downstream effects.
- After syncing, verify the behind count: `git rev-list --left-right --count main...upstream/main` — the right side should be 0 (or only new commits added after the merge target).
