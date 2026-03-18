# The Controller

Tauri v2 + Svelte 5 desktop app for orchestrating multiple Claude Code terminal sessions.

## Task Structure (CRITICAL — NEVER SKIP)

**This is the most important rule. Every task, no matter how small, MUST follow this structure before writing any code. No exceptions.**

1. **Definition**: What's the task? Why are we doing it? How will we approach it?
2. **Constraints**: What are the design constraints — from the user prompt, codebase conventions, or what can be inferred?
3. **Validation**: How do I know for sure it was implemented as expected? Can I enforce it with flexible and non-brittle tests? I must validate before I consider a task complete. For semantic changes (bug fixes, feature refinements): if I revert my implementation, the test must still fail. After the implementation, the test must pass.

**If you catch yourself writing code without having stated all three above, STOP and state them first.**

## Key Docs

- `docs/domain-knowledge.md` — Hard-won lessons (Tauri main thread blocking, CLAUDECODE env var). **Read this before modifying Tauri commands or spawning processes.**
- `docs/plans/` — Design and implementation plans.

## Tech Stack

- **Frontend:** Svelte 5 (runes: `$state`, `$derived`, `$props`, `$effect`), xterm.js
- **Backend:** Rust (Tauri v2), portable-pty, git2
- **Theme:** Catppuccin Mocha

## Branch Completion Rules

When merging a feature branch back to the base branch (e.g. main), you MUST push to remote after merging. Never leave the base branch out of sync with the remote.

```bash
git checkout <base-branch>
git pull
git merge <feature-branch>
# verify tests
git push              # <- REQUIRED, do not skip
git branch -d <feature-branch>
```

## Dependencies

- **New worktrees:** Always run `pnpm install` immediately after creating a worktree. Worktrees don't share `node_modules` — without this, `pnpm test` and the dev server will fail.
- **After merging:** Run `pnpm install` in the project root (not the worktree) if `package.json` was modified. The dev server won't auto-install — missing packages cause a white screen.

## Dev Commands

- `pnpm tauri dev` — Run the app in development mode
- `cd src-tauri && cargo test` — Run Rust tests
- `pnpm test` — Run frontend tests

## Format And Lint (CRITICAL — ALWAYS RUN)

After any code change, before claiming the task is complete, you MUST run the repo's format/lint checks and read the actual output.

- `pnpm check` — frontend typecheck/lint gate; warnings count, so fix them instead of ignoring them
- `cd src-tauri && cargo fmt --check` — Rust formatting gate
- `cd src-tauri && cargo clippy -- -D warnings` — Rust lint gate

Do not claim success based on expectation. If any of these fail, report the failure and keep working or explicitly surface the blocker.
