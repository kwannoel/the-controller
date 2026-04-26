# Keyboard Shortcuts & Modes

All ambient keyboard input flows through `HotkeyManager.svelte`. Hotkey
definitions live in `src/lib/commands.ts`.

## Workspace Modes

The Controller has three workspace modes. Press `Space` then a key to switch:

| Key | Mode |
|-----|------|
| a | Agents — toggle auto-workers and maintainers |
| k | Kanban — organize GitHub issues |
| c | Chat — use daemon-backed chat sessions |

Chat is the default workspace.

## Keyboard State Machine

```
+------------------+
|   Ambient Mode   |
| (sidebar or main |
|  workspace)      |
+----+-------------+
     |
     | Space
     v
+-------------------+
| Workspace Mode    |
| Picker (a/k/c)    |
+-------------------+
```

Ambient shortcuts are ignored while an input, textarea, content-editable
element, or dialog is active.

## Ambient Mode — Global Keys

These work across all workspace modes.

| Key | Action |
|-----|--------|
| j / k | Next / previous visible sidebar item |
| l / Enter | Expand/collapse a project, select a chat session, or open an agent panel |
| f | Fuzzy finder (find project by directory) |
| ? | Toggle help overlay |
| Space | Open workspace mode picker |
| Esc | Move focus up (session → project, agent → project, agent panel → agent list) |
| ⌘K | Toggle keystroke visualizer |

## Ambient Mode — Agents Keys

| Key | Action |
|-----|--------|
| o | Toggle focused auto-worker or maintainer on/off |
| r | Run maintainer check for the focused project |
| c | Clear maintainer reports |
| t | Toggle between Runs / Issues view |

## Ambient Mode — Kanban Keys

| Key | Action |
|-----|--------|
| Space then k | Open the Kanban board |
| Drag issue cards | Move issues between lifecycle columns |

Kanban sidebar navigation walks project rows only.

## Ambient Mode — Chat Keys

| Key | Action |
|-----|--------|
| Space then c | Open daemon-backed chat mode |
| j / k | Move through visible project and chat session rows |
| l / Enter | Select a chat session or expand/collapse a project |

## Agent Panel Keys

When an agent panel is focused after pressing `l` on an agent:

| Key | Action |
|-----|--------|
| j / k | Navigate through panel items |
| l / Enter | Select the highlighted panel item |
| o | Open issue in browser |
| Esc | Return to the agent list |

## Focus Target

The `focusTarget` store tracks the focused navigable item:

- `{ type: "session", sessionId, projectId }` — a daemon chat session in chat mode
- `{ type: "project", projectId }` — a project row
- `{ type: "agent", agentKind, projectId }` — an agent row in agents mode
- `{ type: "agent-panel", agentKind, projectId }` — an agent detail panel

Visual focus is shown with a left border on the focused row or panel.
