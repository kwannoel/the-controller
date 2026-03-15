# E2E Eval Skill Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use the-controller-executing-plans to implement this plan task-by-task.

**Goal:** Enable Claude to self-validate UI changes by spawning a dedicated server pair from a worktree and running Playwright tests against it.

**Architecture:** A helper script (`e2e/eval.sh`) manages the full server lifecycle (Axum + Vite on free ports, with dynamic proxy config). A skill document instructs Claude when and how to use it. Two small code changes make the Axum server port and Vite proxy target configurable via env vars.

**Tech Stack:** Bash (eval.sh), Rust/Axum (server.rs port config), Vite (proxy config), Playwright

**Design doc:** `docs/plans/2026-03-14-e2e-eval-skill-design.md`

---

### Task 1: Make Axum server port configurable via `PORT` env var

**Files:**
- Modify: `src-tauri/src/bin/server.rs:66-68`

**Step 1: Write the failing test**

There's no test harness for the server binary directly. Instead, verify manually.

Run the current server to confirm it hardcodes 3001:
```bash
cd src-tauri && cargo run --bin server --features server &
# Observe: "Server listening on http://localhost:3001"
kill %1
```

**Step 2: Make the port configurable**

In `src-tauri/src/bin/server.rs`, replace lines 66-68:

```rust
// Before:
    println!("Server listening on http://localhost:3001");
    let listener = tokio::net::TcpListener::bind("0.0.0.0:3001").await.unwrap();
    axum::serve(listener, app).await.unwrap();

// After:
    let port = std::env::var("PORT")
        .ok()
        .and_then(|p| p.parse::<u16>().ok())
        .unwrap_or(3001);
    println!("Server listening on http://localhost:{}", port);
    let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{}", port))
        .await
        .unwrap();
    axum::serve(listener, app).await.unwrap();
```

**Step 3: Verify it works**

```bash
cd src-tauri && PORT=4001 cargo run --bin server --features server &
# Observe: "Server listening on http://localhost:4001"
curl -s -X POST http://localhost:4001/api/list_projects | head -c 100
# Should return JSON (not connection refused)
kill %1

# Also verify default still works:
cd src-tauri && cargo run --bin server --features server &
# Observe: "Server listening on http://localhost:3001"
kill %1
```

**Step 4: Commit**

```bash
git add src-tauri/src/bin/server.rs
git commit -m "feat: make axum server port configurable via PORT env var"
```

---

### Task 2: Make Vite proxy target configurable via `AXUM_PORT` env var

**Files:**
- Modify: `vite.config.ts:58-64`

**Step 1: Verify current behavior**

```bash
grep -A5 'proxy:' vite.config.ts
# Confirm: hardcoded "http://localhost:3001"
```

**Step 2: Make the proxy port configurable**

In `vite.config.ts`, replace the proxy block (lines 58-64):

```typescript
// Before:
    proxy: {
      "/api": "http://localhost:3001",
      "/ws": {
        target: "ws://localhost:3001",
        ws: true,
      },
    },

// After:
    proxy: {
      "/api": `http://localhost:${axumPort}`,
      "/ws": {
        target: `ws://localhost:${axumPort}`,
        ws: true,
      },
    },
```

And add the `axumPort` variable near the top (after line 12):

```typescript
const axumPort = process.env.AXUM_PORT || "3001";
```

**Step 3: Verify it works**

```bash
# Default (no env var) should still work:
npm run dev &
# Open http://localhost:1420 — app loads, API calls succeed
kill %1

# Custom port:
AXUM_PORT=4001 npm run dev &
# Vite starts. API calls would go to 4001 (expected to fail if nothing's there — that's correct)
kill %1
```

**Step 4: Commit**

```bash
git add vite.config.ts
git commit -m "feat: make vite proxy target configurable via AXUM_PORT env var"
```

---

### Task 3: Create the helper script `e2e/eval.sh`

**Files:**
- Create: `e2e/eval.sh`

**Step 1: Write the script**

```bash
#!/usr/bin/env bash
set -euo pipefail

# e2e/eval.sh — Run Playwright e2e tests against a fresh server pair from a worktree.
#
# Usage:
#   ./e2e/eval.sh <worktree-path> [test-file...]
#
# Examples:
#   ./e2e/eval.sh /path/to/worktree                         # all specs
#   ./e2e/eval.sh /path/to/worktree e2e/specs/smoke.spec.ts # one spec

WORKTREE="${1:?Usage: $0 <worktree-path> [test-file...]}"
shift
TEST_FILES=("$@")

# --- Cleanup trap ---
AXUM_PID=""
VITE_PID=""
cleanup() {
  echo "Cleaning up..."
  [[ -n "$VITE_PID" ]] && kill "$VITE_PID" 2>/dev/null && wait "$VITE_PID" 2>/dev/null || true
  [[ -n "$AXUM_PID" ]] && kill "$AXUM_PID" 2>/dev/null && wait "$AXUM_PID" 2>/dev/null || true
}
trap cleanup EXIT

# --- Find free ports ---
find_free_port() {
  python3 -c "
import socket
s = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
s.bind(('127.0.0.1', 0))
print(s.getsockname()[1])
s.close()
"
}

AXUM_PORT=$(find_free_port)
VITE_PORT=$(find_free_port)
echo "Eval ports: Axum=$AXUM_PORT, Vite=$VITE_PORT"

# --- Ensure node_modules ---
if [[ ! -d "$WORKTREE/node_modules" ]]; then
  echo "Installing npm dependencies in worktree..."
  (cd "$WORKTREE" && npm install --silent)
fi

# --- Start Axum server ---
echo "Starting Axum server on port $AXUM_PORT..."
(cd "$WORKTREE/src-tauri" && PORT="$AXUM_PORT" cargo run --bin server --features server) &
AXUM_PID=$!

# --- Start Vite dev server ---
echo "Starting Vite dev server on port $VITE_PORT..."
(cd "$WORKTREE" && DEV_PORT="$VITE_PORT" AXUM_PORT="$AXUM_PORT" npm run dev -- --strictPort) &
VITE_PID=$!

# --- Wait for servers to be ready ---
wait_for_port() {
  local port=$1
  local label=$2
  local timeout=$3
  local elapsed=0
  echo "Waiting for $label (port $port)..."
  while ! curl -sf "http://localhost:$port" >/dev/null 2>&1; do
    sleep 2
    elapsed=$((elapsed + 2))
    if [[ $elapsed -ge $timeout ]]; then
      echo "ERROR: $label did not start within ${timeout}s"
      exit 1
    fi
    # Check if process is still alive
    if [[ "$label" == "Axum" ]] && ! kill -0 "$AXUM_PID" 2>/dev/null; then
      echo "ERROR: Axum server process died"
      exit 1
    fi
    if [[ "$label" == "Vite" ]] && ! kill -0 "$VITE_PID" 2>/dev/null; then
      echo "ERROR: Vite dev server process died"
      exit 1
    fi
  done
  echo "$label is ready on port $port"
}

wait_for_port "$VITE_PORT" "Vite" 30
wait_for_port "$AXUM_PORT" "Axum" 180  # cargo build can be slow

# --- Run Playwright ---
echo "Running Playwright tests..."
PLAYWRIGHT_ARGS=(--project=e2e)
if [[ ${#TEST_FILES[@]} -gt 0 ]]; then
  PLAYWRIGHT_ARGS+=("${TEST_FILES[@]}")
fi

set +e
BASE_URL="http://localhost:$VITE_PORT" npx playwright test "${PLAYWRIGHT_ARGS[@]}"
EXIT_CODE=$?
set -e

if [[ $EXIT_CODE -eq 0 ]]; then
  echo "All tests passed."
else
  echo "Tests failed (exit code $EXIT_CODE). Check e2e/results/ for artifacts."
fi

exit $EXIT_CODE
```

**Step 2: Make it executable**

```bash
chmod +x e2e/eval.sh
```

**Step 3: Smoke test the script**

```bash
# Run against the current worktree with the smoke test
./e2e/eval.sh "$(pwd)" e2e/specs/smoke.spec.ts
# Should: start servers, run smoke test, pass, tear down
```

**Step 4: Commit**

```bash
git add e2e/eval.sh
git commit -m "feat: add e2e eval helper script for self-contained Playwright testing"
```

---

### Task 4: Create the skill document

**Files:**
- Create: `skills/the-controller-e2e-eval/SKILL.md`

**Step 1: Create the skill directory**

```bash
mkdir -p skills/the-controller-e2e-eval
```

**Step 2: Write the skill**

```markdown
---
name: the-controller-e2e-eval
description: Use when validating UI changes end-to-end before claiming work is complete — spawns a fresh server pair from the worktree and runs Playwright tests against it
---

# E2E Eval — Self-Validating UI Changes

## When to Use

You've made a UI change in a session's worktree and want to verify it actually works in a browser — not just "it compiles" but "the user can see and interact with it correctly."

## Prerequisites

The session must be staged (user pressed `v` in development mode). If not staged, ask the user to stage it first.

## Steps

### 1. Find the worktree path

Read project.json files to find the staged session's worktree:

```bash
# Find the project with a staged session
cat ~/.the-controller/projects/*/project.json | jq -r '
  select(.staged_session != null) |
  .staged_session.session_id as $sid |
  .sessions[] | select(.id == $sid) | .worktree_path
'
```

### 2. Write a targeted Playwright test

Create a spec file in `e2e/specs/` following existing patterns (see `e2e/specs/smoke.spec.ts` for the simplest example):

```typescript
import { test, expect } from "@playwright/test";

test("description of what the UI change does", async ({ page }) => {
  await page.goto("/");
  // Setup: navigate to the right state
  // Action: perform the user interaction
  // Assert: verify the expected outcome
});
```

Use helpers from `e2e/helpers/` if you need seeded projects or test repos.

### 3. Run the targeted test

```bash
./e2e/eval.sh <worktree-path> e2e/specs/<your-test>.spec.ts
```

The script handles everything: finds free ports, starts Axum + Vite from the worktree, runs Playwright, tears down.

### 4. Run the regression suite

```bash
./e2e/eval.sh <worktree-path>
```

This runs ALL specs to catch regressions.

### 5. Interpret results

- **All pass:** Commit the new test to the worktree. It becomes part of the regression suite.
- **Targeted test fails:** Your UI change has a bug. Fix the code, re-run.
- **Regression test fails:** Your change broke something else. Investigate.
- **Deeper investigation needed:** Use the-controller-debugging-ui-with-playwright skill for the 4-phase root cause analysis.

## Common Mistakes

- **Forgetting to stage first:** The eval needs a worktree path. Stage the session with `v`.
- **Testing against wrong servers:** Always use `eval.sh` — never manually start servers, as port conflicts with the main dev setup will cause false results.
- **Flaky waits:** Use `await expect(...).toBeVisible({ timeout: 10_000 })` instead of `waitForTimeout()` for assertions.
```

**Step 3: Commit**

```bash
git add skills/the-controller-e2e-eval/SKILL.md
git commit -m "feat: add e2e eval skill for self-validating UI changes"
```

---

### Task 5: Sync the skill and end-to-end validation

**Step 1: Sync the skill into Claude's skill directories**

Use the the-controller-synchronizing-skills skill to symlink the new skill.

**Step 2: Validate end-to-end**

Stage a session with a trivial UI change (or use the current worktree), then:

```bash
# Run smoke test through eval.sh
./e2e/eval.sh <worktree-path> e2e/specs/smoke.spec.ts
```

Verify:
- Axum starts on a random port (not 3001)
- Vite starts on a random port (not 1420)
- Vite proxies API calls to the Axum port
- Smoke test passes
- Both servers are cleaned up after

**Step 3: Commit any fixes**

If anything needed adjustment during validation, commit the fixes.
