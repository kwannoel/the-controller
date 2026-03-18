# Keyboard Shortcuts & Modes

All keyboard input flows through `HotkeyManager.svelte`. Hotkey definitions live in `src/lib/commands.ts`.

When you change hotkeys, overlays, or other UI behavior in this area, finish by running `pnpm check`, `cd src-tauri && cargo fmt --check`, and `cd src-tauri && cargo clippy -- -D warnings`. The frontend check fails on warnings, not just errors.

## Workspace Modes

The Controller has six workspace modes, each with its own hotkeys. Press `Space` then a key to switch:

| Key | Mode                                              |
| --- | ------------------------------------------------- |
| d   | Development — manage sessions, branches, projects |
| a   | Agents — toggle auto-workers and maintainers      |
| r   | Architecture — generate project architecture docs |
| n   | Notes — markdown notes organized by folder        |
| i   | Infrastructure — deploy and rollback projects     |
| v   | Voice — voice interaction mode                    |

## Keyboard State Machine

```
+-----------------+
|   Terminal      |
|   Passthrough   |
+--------+--------+
         |
    Esc (single) → focus moves to sidebar
         |
         v
+------------------+
|   Ambient Mode   |
| (sidebar/no      |
|  focus on input) |
+----+-------------+
     |
     | Space
     v
+-------------------+
| Workspace Mode    |
| Picker (d/a/r/n/  |
|          i/v)     |
+-------------------+
```

## Terminal Passthrough

**When**: Terminal (xterm) is focused.

| Key                  | Action                                  |
| -------------------- | --------------------------------------- |
| Esc (single)         | Move focus to active session in sidebar |
| Esc (double, <300ms) | Forward Esc to terminal PTY             |
| Any other key        | Passes through to terminal              |

## Ambient Mode — Global Keys

These work in all workspace modes when no terminal or editable element is focused.

| Key       | Action                                                            |
| --------- | ----------------------------------------------------------------- |
| j / k     | Next / previous item in sidebar                                   |
| l / Enter | Expand/collapse project, focus terminal, or open panel            |
| f         | Fuzzy finder (find project by directory)                          |
| ?         | Toggle help overlay                                               |
| Space     | Open workspace mode picker                                        |
| Esc       | Move focus up (note → folder, session → project, agent → project) |
| Esc Esc   | Forward escape to terminal and refocus it                         |

## Ambient Mode — Development Keys

| Key       | Action                                                                    |
| --------- | ------------------------------------------------------------------------- |
| c         | Create session for focused project                                        |
| n         | New project                                                               |
| d         | Delete focused item (session or project)                                  |
| i         | Issues — create, find, assign for focused project                         |
| m         | Merge/finish branch for active session (creates PR)                       |
| v         | **Stage / unstage session** (see [Staging](#staging-hot-reload--preview)) |
| p         | Load a saved prompt into a new session                                    |
| P         | Save focused session's prompt                                             |
| ⌘T        | Cycle session provider (Claude → Codex → Cursor)                          |
| ⌘S        | Screenshot (full window) → pick session to send to                        |
| ⌘D        | Screenshot (cropped) → pick session to send to                            |
| ⌘⇧S / ⌘⇧D | Screenshot with preview before sending                                    |
| ⌘K        | Toggle keystroke visualizer                                               |

## Ambient Mode — Agents Keys

| Key | Action                                   |
| --- | ---------------------------------------- |
| o   | Toggle focused agent on/off              |
| r   | Run maintainer check for focused project |
| c   | Clear maintainer reports                 |
| t   | Toggle between Runs / Issues view        |

## Ambient Mode — Architecture Keys

| Key | Action                                                 |
| --- | ------------------------------------------------------ |
| r   | Generate / regenerate architecture for focused project |

## Ambient Mode — Notes Keys

| Key       | Action                                           |
| --------- | ------------------------------------------------ |
| n         | Create new note                                  |
| d         | Delete focused note or folder                    |
| r         | Rename focused note or folder                    |
| y         | Duplicate focused note                           |
| p         | Cycle note preview mode (edit / preview / split) |
| o / i / a | Open note for editing (vim-style)                |

## Ambient Mode — Infrastructure Keys

| Key | Action                   |
| --- | ------------------------ |
| d   | Deploy focused project   |
| r   | Rollback last deployment |

## Ambient Mode — Voice Keys

| Key | Action                  |
| --- | ----------------------- |
| d   | Toggle debug panel      |
| t   | Toggle transcript panel |

## Agent Panel Keys

When an agent panel is focused (after pressing `l` on an agent):

| Key       | Action                 |
| --------- | ---------------------- |
| j / k     | Navigate through items |
| l / Enter | Select item            |
| o         | Open issue in browser  |
| Esc       | Return to agent list   |

## Staging (Hot Reload / Preview)

**This is how you preview or hot-reload changes from a session's branch.**

Press `v` in development mode to stage the active session. This launches a **separate Controller instance** from the session's git worktree, running on a different port (base port + 1000, e.g. 2420). The staged instance is a full Controller with its own Vite HMR and Rust backend — it picks up both frontend and backend changes from that branch.

Press `v` again to unstage (kills the staged instance).

**What happens when you stage:**
1. Worktree is committed (prompts Claude to commit if dirty)
2. Branch is rebased onto main if behind
3. `pnpm install` runs in the worktree if needed
4. `./dev.sh <port>` launches a separate Controller instance
5. Main Controller title bar shows "staging: session-label"

**What the staged instance gives you:**
- Full Vite HMR for frontend changes
- Full cargo rebuild for backend changes
- Independent process — doesn't affect your main Controller
- PTY broker sessions persist across both instances

See `docs/plans/2026-03-11-staging-separate-instance-design.md` for architecture details.

## Focus Target

The `focusTarget` store tracks what's currently focused:
- `{ type: "session", sessionId, projectId }` — a session in the sidebar
- `{ type: "project", projectId }` — a project header
- `{ type: "agent", agentKind, projectId }` — an agent in agents mode
- `{ type: "agent-panel", agentKind, projectId }` — an agent's detail panel
- `{ type: "folder", folder }` — a note folder
- `{ type: "note", filename, folder }` — a note entry
- `{ type: "notes-editor", folder }` — the note editor

Visual borders highlight the focused element (blue left border).
