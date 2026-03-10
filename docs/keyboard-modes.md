# Keyboard Mode State Machine

All keyboard input flows through `HotkeyManager.svelte`. The active states are terminal passthrough, ambient mode, toggle mode, and workspace mode:

```
+-----------------+
|   Terminal      |
|   Passthrough   |
+--------+--------+
         |
    Esc (single) → focus moves to sidebar session
         |
         v
+------------------+
|   Ambient Mode   |
| (sidebar/no      |
|  focus on input) |
+----+--------+----+
     |        |
   o |        | Space
     v        v
+---------+  +-------------+
| Toggle  |  | Workspace   |
| Mode    |  | Mode Picker |
+---------+  +-------------+
```

## Focus Target

The `focusTarget` store tracks what's currently focused:
- `{ type: "terminal" }` — terminal panel is focused
- `{ type: "session", sessionId, projectId }` — a specific session in the sidebar
- `{ type: "project", projectId }` — a project in the sidebar
- `null` — nothing specific

Visual borders highlight the focused element:
- Blue left border on the focused project header
- Blue left border on the focused session item
- Blue left border on the terminal panel

## Modes

### Terminal Passthrough

**When**: Terminal (xterm) is focused.

| Key | Action |
|-----|--------|
| Esc (single) | Move focus to active session in sidebar (ambient mode) |
| Esc (double, <300ms) | Forward Esc to terminal PTY |
| Any other key | Passes through to terminal |

### Ambient Mode

**When**: No terminal or editable element is focused (e.g. sidebar, empty area).

Hotkeys work directly. Input/textarea/contenteditable elements are excluded.

| Key | Action |
|-----|--------|
| j / k | Move focus through visible projects and sessions |
| J / K | Move focus between projects only |
| l / Enter | Expand/collapse project, focus terminal from session, or open the focused panel |
| c | Create session for the focused project |
| d | Delete focused project or session |
| a | Archive focused project or session |
| A | Toggle archive view |
| f | Open fuzzy finder |
| n | New project |
| i | Create issue for the focused project |
| m | Finish branch for the active session |
| o | Enter toggle mode |
| Space | Open workspace mode picker |
| s | Toggle sidebar visibility |
| ? | Toggle help overlay |

### Toggle Mode

**When**: User pressed `o` from Ambient mode in development workspace.

The next key is interpreted as a maintainer/worker toggle command.

| Key | Action |
|-----|--------|
| m | Toggle maintainer |
| w | Toggle auto-worker |
| Esc | Cancel toggle mode |
| Any other key | Cancel toggle mode |

### Workspace Mode Picker

**When**: User pressed `Space` from Ambient mode.

The next key switches workspace modes.

| Key | Action |
|-----|--------|
| d | Switch to Development |
| a | Switch to Agents |
| n | Switch to Notes |
| Esc | Cancel picker |
| Any other key | Cancel picker |

## Transitions

```
Terminal Passthrough
  --[Esc single]--> Ambient Mode (focusTarget → active session in sidebar)
  --[Esc double]--> Terminal Passthrough (forwards Esc to PTY)

Ambient Mode
  --[o]--> Toggle Mode
  --[Space]--> Workspace Mode Picker
  --[l/Enter on session]--> Terminal Passthrough (focusTarget → terminal)
  --[hotkey]--> Ambient Mode (executes action)

Toggle Mode
  --[m/w]--> Ambient Mode (dispatches toggle action)
  --[Esc/other]--> Ambient Mode

Workspace Mode Picker
  --[d/a/n]--> Ambient Mode (switches workspace)
  --[Esc/other]--> Ambient Mode
```
