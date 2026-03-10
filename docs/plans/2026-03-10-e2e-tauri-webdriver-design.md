# E2E Tests via Tauri WebDriver

## Goal

Set up end-to-end tests using Tauri WebDriver + WebdriverIO to test real user workflows with real processes and real GitHub integration. First test: merging a Codex session's branch (rebase, push, PR creation).

## Architecture

```
Test Runner (WebdriverIO)
    ↓ WebDriver protocol
tauri-driver
    ↓ native webview debug protocol
The Controller app (built binary)
    ↓ real PTY, real git
Codex CLI + GitHub (e2e-test-sandbox repo)
```

### Components

- **`tauri-driver`** — Installed via `cargo install tauri-driver`. WebDriver server wrapping the native webview.
- **WebdriverIO** — Node.js test client. Config in `wdio.conf.ts` at project root.
- **Test specs** — Live in `e2e/specs/`, TypeScript.
- **Test fixtures** — Helpers to clone sandbox repo, create branches, clean up.

## Test: Merge a Codex Session Branch

### Setup (beforeAll)

1. Clone `noel/e2e-test-sandbox` to a temp directory
2. Create a feature branch with a trivial commit
3. Push the branch to remote
4. Start `tauri-driver` process

### Test Steps

1. App launches via WebDriver
2. Create a new project pointing at the cloned repo
3. Toggle provider to Codex (`Cmd+T`)
4. Start a session (`c` key) — real Codex spawns in the PTY
5. Trigger "finish branch" merge action
6. Wait for merge-status events / PR URL to appear in the UI
7. Assert: PR exists on GitHub via `gh pr view`

### Teardown (afterAll)

1. Close PR via `gh pr close`
2. Delete remote branch via `git push origin --delete`
3. Remove temp directory
4. Kill `tauri-driver`

## File Structure

```
e2e/
  wdio.conf.ts          # WebdriverIO config
  specs/
    merge-codex.spec.ts  # The merge workflow test
  helpers/
    repo-setup.ts        # Clone, branch, commit, cleanup
    app.ts               # App interaction helpers (create project, start session)
```

## Dependencies

Dev dependencies to add:

```json
{
  "@wdio/cli": "^9",
  "@wdio/local-runner": "^9",
  "@wdio/mocha-framework": "^9",
  "@wdio/spec-reporter": "^9",
  "webdriverio": "^9"
}
```

System prerequisites:
- `cargo install tauri-driver`
- Codex CLI installed and authenticated
- `gh` CLI authenticated with sandbox repo access
- App built via `npm run tauri build`

## Constraints

- macOS only (local dev environment)
- Generous timeouts — Codex sessions and merges take time
- No CI for now — local-only execution
- Sandbox repo (`noel/e2e-test-sandbox`) must exist on GitHub
- Runs against compiled binary, not dev mode
