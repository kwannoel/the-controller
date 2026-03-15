# E2E Eval Skill Design

**Date:** 2026-03-14

## Problem

Claude needs a way to self-validate UI changes end-to-end — not just "does the code compile" but "does the UI actually work in a browser." Today, UI verification is manual (stage with `v`, eyeball it) or requires the developer to invoke Playwright debugging. There's no skill for Claude to autonomously run e2e evals.

## Design

### Architecture: Fully Self-Contained Server Pair

The skill never reuses the staged Tauri app's servers. Instead:

1. **Detect the worktree path** — read `~/.the-controller/projects/*/project.json`, find `staged_session`, look up the session's `worktree_path`. If nothing is staged, tell Claude to ask the user to stage first.
2. **Start a dedicated server pair** from that worktree — Axum on a free port, Vite on a free port, with the Vite proxy dynamically pointed at the Axum port.
3. **Run Playwright** against the Vite port.
4. **Tear down** both servers via `trap`.

The staged Tauri app and the eval servers are completely independent. No port conflicts, no shared state assumptions.

### Components

#### 1. Helper Script: `e2e/eval.sh`

```
./e2e/eval.sh <worktree-path> [test-file...]
```

Steps:
1. Accept worktree path and optional test file(s)
2. `npm install` in worktree if `node_modules` missing
3. Find two free ports (Axum, Vite) via bind-and-release
4. Start Axum: `cd <worktree> && cargo run --bin server --features server` with `PORT=<axum_port>`
5. Start Vite: `cd <worktree> && DEV_PORT=<vite_port> AXUM_PORT=<axum_port> npm run dev`
6. Poll both until ready (curl, max 120s for Axum cargo build)
7. Run: `BASE_URL=http://localhost:<vite_port> npx playwright test [test-files] --project=e2e`
8. Capture exit code
9. `trap` kills both server PIDs on exit (success or failure)
10. Exit with Playwright's exit code

#### 2. Skill: `the-controller-e2e-eval/SKILL.md`

Trigger: Claude wants to validate a UI change it made.

The skill instructs Claude to:
1. Verify session is staged (ask user to press `v` if not)
2. Find the worktree path from project.json → staged_session → session → worktree_path
3. Write a targeted Playwright test in `e2e/specs/` following existing patterns
4. Run targeted test: `./e2e/eval.sh <worktree-path> e2e/specs/<new-test>.spec.ts`
5. Run regression suite: `./e2e/eval.sh <worktree-path>` (all specs)
6. Interpret results — fix code if failures, commit test if passes
7. Cross-references `the-controller-debugging-ui-with-playwright` for deeper investigation

### Files to Modify

- `vite.config.ts` — Read `AXUM_PORT` env var for proxy target (fallback 3001)
- `src-tauri/src/bin/server.rs` — Read `PORT` env var for listen port (fallback 3001)

### Files to Create

- `skills/the-controller-e2e-eval/SKILL.md`
- `e2e/eval.sh`

### No Changes To

- `playwright.config.ts` — `BASE_URL` overrides `baseURL`; script starts its own servers
- `dev.sh` — Staging unchanged
- Existing e2e specs — Work with either setup
