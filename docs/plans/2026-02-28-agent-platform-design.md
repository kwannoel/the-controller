# Agent Platform — Design Document

A Tauri desktop app for managing multiple Claude Code sessions across projects, with a sidebar GUI for navigation and full embedded terminals.

## Problem

Juggling multiple Claude Code terminals across projects is hard to track. No visibility into what's running where, no structured way to spawn sessions with project-specific instructions.

## Architecture

Three layers:

1. **Svelte Frontend** — Sidebar (project tree, session status indicators) + single terminal area (xterm.js, switched via sidebar selection)
2. **Tauri IPC** — Commands for project/session management, events for streaming PTY output
3. **Rust Backend** — PTY Manager (portable-pty) and Project/Worktree Manager (git2)

```
┌─────────────────────────────────────────────────┐
│                  Tauri App                       │
│                                                  │
│  ┌──────────┐  ┌─────────────────────────────┐  │
│  │ Sidebar  │  │       Terminal Area          │  │
│  │          │  │                               │  │
│  │ Project A│  │  ┌─────────────────────────┐ │  │
│  │  ├ s1 ●  │◄─┤  │  xterm.js (active)      │ │  │
│  │  └ s2 ○  │  │  │                         │ │  │
│  │          │  │  │  $ claude ...            │ │  │
│  │ Project B│  │  │                         │ │  │
│  │  └ s1 ●  │  │  └─────────────────────────┘ │  │
│  │          │  │                               │  │
│  │ [+ New]  │  │  (single terminal, switched  │  │
│  └──────────┘  │   via sidebar selection)      │  │
│                 └─────────────────────────────┘  │
│                                                  │
│  ┌──────────────────────────────────────────┐   │
│  │          Rust Backend (Tauri)             │   │
│  │  ┌────────────┐  ┌───────────────────┐   │   │
│  │  │ PTY Manager│  │ Project/Worktree  │   │   │
│  │  │ (portable- │  │    Manager        │   │   │
│  │  │  pty)      │  │ (git2-rs)         │   │   │
│  │  └────────────┘  └───────────────────┘   │   │
│  └──────────────────────────────────────────┘   │
└─────────────────────────────────────────────────┘
```

Sidebar is the sole navigation. One terminal fills the main area; clicking a session in the sidebar swaps which PTY xterm.js renders. All PTYs stay alive in the background.

## Data Model

App config lives in `~/.the-controller/`, separate from any managed repo:

```
~/.the-controller/
├── config.json
└── projects/
    └── <project-id>/
        ├── project.json
        └── agents.md        # fallback only, repo's agents.md takes priority
```

**project.json:**

```json
{
  "id": "uuid",
  "name": "my-feature",
  "repo_path": "/path/to/repo",
  "created_at": "2026-02-28T...",
  "archived": false,
  "sessions": [
    {
      "id": "uuid",
      "label": "main",
      "worktree_path": null,
      "worktree_branch": null,
      "status": "running"
    },
    {
      "id": "uuid",
      "label": "refine-auth",
      "worktree_path": "/path/to/repo/.worktrees/refine-auth",
      "worktree_branch": "refine-auth",
      "status": "idle"
    }
  ]
}
```

- Sessions either run in the main repo or in a managed worktree
- `status` is derived from PTY process state, not manually set
- `agents.md` from the repo root is authoritative; the app-level one is a fallback for repos without one

## Core Flows

### New Project

1. User clicks `[+ New]` → "Create new"
2. Modal: project name + repo path (directory picker)
3. Backend creates project config dir with `project.json` and default `agents.md`
4. Project appears in sidebar

### Load Existing Project

1. User clicks `[+ New]` → "Load existing"
2. Directory picker → select a git repo
3. User gives it a project name
4. Backend creates project entry in `~/.the-controller/projects/<id>/`
5. If repo has `agents.md` at root → reference it directly, don't create a new one
6. If no `agents.md` → create a default in the project config dir
7. Project appears in sidebar

### New Session

1. Right-click project or click `+` next to it
2. Choose "New session" or "New refinement (worktree)"
3. **Session** — spawns `claude` in `repo_path`, passing `agents.md` content
4. **Refinement** — asks for branch name, creates worktree via `git worktree add`, spawns `claude` in worktree path with `agents.md`
5. Session appears under project in sidebar, terminal area switches to it

### Session Lifecycle

- `●` running / `○` idle (process exited, output preserved)
- User can restart idle sessions or close them (destroys PTY, optionally cleans up worktree)

### Archive Project

1. Right-click project → "Archive"
2. Running sessions get graceful shutdown
3. Worktrees cleaned up
4. `archived: true` in `project.json`
5. Hidden from sidebar (toggle "Show archived" to view)

## Error Handling

- **Claude Code crashes** — session goes `○` idle, output preserved
- **Claude Code not in PATH** — error shown in terminal area on session start
- **Worktree creation fails** — git error surfaced in toast, session not created
- **Worktree cleanup fails (uncommitted changes)** — warn user, offer force-delete or keep
- **Repo path gone** — project shown as "disconnected" with re-link option
- **`agents.md` deleted from repo** — fall back to default in config dir, notify user
- **App closes** — all PTYs killed, no background daemon
- **App opens** — loads project list, sessions start empty

## Tech Stack

### Rust Backend

- `tauri` v2 — app shell, IPC, window management
- `portable-pty` — cross-platform PTY spawning
- `git2` — worktree creation/cleanup, repo validation
- `serde` / `serde_json` — config serialization
- `uuid` — IDs
- `tokio` — async PTY I/O streaming

### Svelte Frontend

- Svelte 5 (runes)
- `xterm.js` + `xterm-addon-fit` — terminal rendering and auto-resize
- Minimal CSS, no component library

### IPC Contract

**Commands:** `create_project`, `load_project`, `archive_project`, `create_session`, `create_refinement`, `close_session`, `restart_session`, `write_to_pty`, `resize_pty`, `list_projects`, `update_agents_md`

**Events (backend → frontend):** `pty-output:{session_id}`, `session-status-changed:{session_id}`
