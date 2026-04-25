# Keyboard Shortcuts & Modes

All keyboard input flows through `HotkeyManager.svelte`. Hotkey definitions live in `src/lib/commands.ts`.

## Workspace Modes

The Controller has four workspace modes, each with its own hotkeys. Press `Space` then a key to switch:

| Key | Mode |
|-----|------|
| d | Development — manage sessions, branches, projects |
| a | Agents — toggle auto-workers and maintainers |
| k | Kanban — organize GitHub issues |
| c | Chat — use daemon-backed chat sessions |

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
| Picker (d/a/k/c)  |
+-------------------+
```

## Terminal Passthrough

**When**: Terminal (xterm) is focused.

| Key | Action |
|-----|--------|
| Esc (single) | Move focus to active session in sidebar |
| Esc (double, <300ms) | Forward Esc to terminal PTY |
| Any other key | Passes through to terminal |

## Ambient Mode — Global Keys

These work in all workspace modes when no terminal or editable element is focused.

| Key | Action |
|-----|--------|
| j / k | Next / previous item in sidebar |
| l / Enter | Expand/collapse project, focus terminal, or open panel |
| f | Fuzzy finder (find project by directory) |
| ? | Toggle help overlay |
| Space | Open workspace mode picker |
| Esc | Move focus up (note → folder, session → project, agent → project) |
| Esc Esc | Forward escape to terminal and refocus it |

## Ambient Mode — Development Keys

| Key | Action |
|-----|--------|
| c | Create session for focused project |
| n | New project |
| d | Delete focused item (session or project) |
| i | Issues — create, find, assign for focused project |
| m | Merge/finish branch for active session (creates PR) |
| v | **Stage / unstage session** (see [Staging](#staging-hot-reload--preview)) |
| p | Load a saved prompt into a new session |
| P | Save focused session's prompt |
| ⌘T | Toggle session provider (Claude ↔ Codex) |
| ⌘S | Screenshot (full window) → pick session to send to |
| ⌘D | Screenshot (cropped) → pick session to send to |
| ⌘⇧S / ⌘⇧D | Screenshot with preview before sending |
| ⌘K | Toggle keystroke visualizer |

## Ambient Mode — Agents Keys

| Key | Action |
|-----|--------|
| o | Toggle focused agent on/off |
| r | Run maintainer check for focused project |
| c | Clear maintainer reports |
| t | Toggle between Runs / Issues view |

## Ambient Mode — Kanban Keys

| Key | Action |
|-----|--------|
| Space then k | Open the Kanban board for the focused project |
| Drag issue cards | Move issues between lifecycle columns |

## Ambient Mode — Chat Keys

| Key | Action |
|-----|--------|
| Space then c | Open daemon-backed chat mode |
| j / k | Move through visible chat sessions |
| l / Enter | Select a chat session or expand/collapse a project |

## Agent Panel Keys

When an agent panel is focused (after pressing `l` on an agent):

| Key | Action |
|-----|--------|
| j / k | Navigate through items |
| l / Enter | Select item |
| o | Open issue in browser |
| Esc | Return to agent list |

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
- Same tmux sessions accessible from both instances

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
