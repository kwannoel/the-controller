# Staging via Separate Controller Instance

## Problem

Current staging checks out a `staging/<branch>` in the main repo so HMR picks up changes. This modifies the running Controller's codebase in-place, which is disruptive and only surfaces frontend changes via HMR (backend changes require the same cargo rebuild either way).

## Solution

Launch a **separate Controller instance** from the session's worktree directory using `npm run tauri dev` on a different port. The new instance is a full Controller with its own compiled backend, its own frontend, and access to the same tmux-backed sessions.

## Staging Flow

1. **Commit phase** (unchanged): ensure worktree is clean, prompt Claude to commit if dirty
2. **Rebase phase** (unchanged): rebase onto main if behind
3. **Launch phase** (new):
   - Run `npm install` in worktree if `node_modules` is missing
   - Pick port: base port + 1000 (e.g., 2420), verify free
   - Spawn `./dev.sh <port>` from worktree as child process
   - Store PID and port in `StagedSession`
   - Update main Controller title bar: "The Controller (...) — staging: session-label"

## Unstaging

Press `v` again:
1. Kill the child process tree (`kill -- -<pgid>`)
2. Clear `StagedSession` from project storage
3. Restore main Controller title bar

## Session Access

Sessions are backed by tmux (`ctrl-{uuid}`). Tmux supports multiple clients attaching simultaneously — each gets its own PTY with broadcast output. The staged Controller instance reads session metadata from the same `~/.the-controller/projects/` directory and attaches to the same tmux sessions independently.

## Status Socket

The main instance binds `/tmp/the-controller.sock`. The staged instance uses a different path, passed via env var: `CONTROLLER_SOCKET=/tmp/the-controller-staged.sock`.

## Port Selection

Base port + 1000 offset (e.g., 1420 → 2420). Verify the port is free before launching. If occupied, increment until a free port is found.

## Process Lifecycle

- **Spawn**: `dev.sh <port>` from worktree directory, creating a process group (shell → cargo-tauri → vite + rustc)
- **Kill**: Process group kill on unstage (`kill -- -<pgid>`)
- **App exit cleanup**: Kill staged process if main Controller quits
- **Crash handling**: Detect child process death, clear `StagedSession`, show error toast
- **Progress**: Emit staging-status events ("Installing dependencies...", "Compiling...", etc.) — non-blocking, user keeps using main Controller

## Model Changes

```rust
pub struct StagedSession {
    pub session_id: Uuid,
    pub pid: u32,        // child process PID (replaces original_branch)
    pub port: u16,       // dev server port (replaces staging_branch)
}
```

## What's Removed

- `stage_inplace` / `unstage_inplace` in `worktree.rs`
- `git checkout staging/<branch>` logic
- File-touching for HMR triggers

## What's New (Backend)

- `stage_session` command: commit/rebase + npm install + spawn dev.sh
- `unstage_session` command: kill process tree, clear state
- Port selection helper
- Process tree management (spawn, kill, crash detection)
- Configurable status socket path via env var

## What's Modified (Frontend)

- Hotkey handler: call `stage_session` / `unstage_session`
- Title bar: append "— staging: session-label" when staged
- Staging status toasts: npm install, compilation messages
- Sidebar badge: unchanged (shows which session is staged)

## What's Unchanged

The staged Controller instance is a regular Controller — no special code. It reads the same project data, attaches to the same tmux sessions, and gets its title bar from normal vite defines (`__BUILD_BRANCH__`, `__DEV_PORT__`).
