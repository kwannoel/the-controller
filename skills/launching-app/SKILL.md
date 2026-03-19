---
name: launching-app
description: Use when the user asks to launch, relaunch, or restart the-controller app
---

# Launching the App

## Launch

```bash
cd "$(git rev-parse --show-toplevel)" && pnpm tauri dev
```

Run as a background task. This starts Vite (frontend HMR) and the Tauri file watcher (Rust rebuild + restart on changes).

## Relaunch

Kill the **process group** to avoid orphaning the Tauri binary:

```bash
# Kill existing tauri dev process group
tauri_pid=$(ps aux | grep "node.*tauri.*dev" | grep -v grep | head -1 | awk '{print $2}')
if [ -n "$tauri_pid" ]; then
  kill -- -$(ps -o pgid= -p "$tauri_pid" | tr -d ' ') 2>/dev/null
fi

# Also kill any orphaned Tauri binaries and Vite on port 1420
ps aux | grep "target/debug/the-controller" | grep -v grep | awk '{print $2}' | xargs kill -9 2>/dev/null
lsof -i :1420 -t 2>/dev/null | xargs kill -9 2>/dev/null

sleep 1
cd "$(git rev-parse --show-toplevel)" && pnpm tauri dev
```

## Rules

- **Never kill individual PIDs** — SIGTERM doesn't propagate to grandchildren, orphaning the Tauri binary
- **Always use `cd "$(git rev-parse --show-toplevel)"`** — works in worktrees and prevents cwd drift
- **Run as background task** — the process stays alive across the session
