# The Controller

Tauri v2 + Svelte 5 desktop app for orchestrating multiple Claude Code terminal sessions.

## Task Workflow (CRITICAL)

For every new task, follow this workflow:

1. **File a GitHub issue** — Create an issue in this repository describing the task before starting work.
2. **Update with design plan** — Once the design is complete, update the GitHub issue with the design plan.
3. **Update with implementation plan** — Once the implementation plan is ready, update the GitHub issue with it.
4. **Close the issue** — Close the GitHub issue once the task is fully completed and verified.
5. **Update the merge commit/PR** — After merging, update the merge note (PR description or commit message) to summarize what work was done.

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

## Dev Commands

- `npm run tauri dev` — Run the app in development mode
- `cd src-tauri && cargo test` — Run Rust tests
- `npx vitest run` — Run frontend tests
