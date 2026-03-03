# Keyboard Mode State Machine

All keyboard input flows through `HotkeyManager.svelte`. The system has three modes plus jump navigation:

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
+--------+---------+
         |
    j → Jump Project Phase
         |
    label match → Jump Session Phase
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
| j | Enter Jump mode (project phase) |
| c | Create session (in active project) |
| x | Close active session |
| d | Delete active project |
| a | Archive active project |
| f | Open fuzzy finder |
| n | New project |
| h | Focus sidebar |
| l | Focus terminal |
| s | Toggle sidebar visibility |
| ? | Toggle help overlay |

### Jump Mode — Project Phase

**When**: User pressed `j` from Ambient mode.

Labels (from `JUMP_KEYS = [z,x,c,b,n,m]`) appear next to each project in the sidebar. Single-char for <=6 projects, two-char for >6.

| Key | Action |
|-----|--------|
| Esc | Cancel jump mode |
| Label match | Enter Session Phase for that project |
| Label prefix | Buffer key, wait for next |
| Non-label key | Cancel jump mode |

### Jump Mode — Session Phase

**When**: User matched a project label in Project Phase.

Labels appear next to each session + a virtual "+ New session" entry. Also accepts `d`/`a` for project-level operations.

| Key | Action |
|-----|--------|
| Esc | Cancel jump mode |
| d | Delete the jumped-to project |
| a | Archive the jumped-to project |
| Label match (session) | Switch to that session |
| Label match (last = new) | Create new session in that project |
| Label prefix | Buffer key, wait for next |
| Non-label key | Cancel jump mode |

## Transitions

```
Terminal Passthrough
  --[Esc single]--> Ambient Mode (focusTarget → active session in sidebar)
  --[Esc double]--> Terminal Passthrough (forwards Esc to PTY)

Ambient Mode
  --[j]--> Jump Project Phase
  --[l]--> Terminal Passthrough (focusTarget → terminal)
  --[hotkey]--> Ambient Mode (executes action)

Jump Project Phase
  --[Esc]--> Ambient Mode
  --[label match]--> Jump Session Phase
  --[non-label]--> Ambient Mode

Jump Session Phase
  --[Esc]--> Ambient Mode
  --[d/a]--> Ambient Mode (dispatches project action)
  --[label match]--> Ambient Mode (switches session or creates new)
  --[non-label]--> Ambient Mode
```
