# Hotkey-Driven UX Design

## Goal

Make the entire app operable by hotkeys only, using a vim/tmux-style leader key pattern.

## Leader Key

- **Single Escape** activates leader mode
- **300ms timeout** — if no follow-up key, forward Escape to terminal and return to idle
- **Escape in leader mode** — cancel, forward Escape to terminal

## Key Bindings

| Key (after Esc) | Action |
|-----------------|--------|
| `1`-`9` | Switch to session N |
| `n` | Next session |
| `p` | Previous session |
| `c` | Create new session in current project |
| `x` | Close current session |
| `f` | Open fuzzy finder |
| `N` (Shift+n) | Open new project modal |
| `h` | Focus sidebar |
| `l` | Focus terminal |
| `j` | Next project (expand) |
| `k` | Previous project (expand) |
| `Escape` | Cancel leader mode |
| `?` | Toggle key bindings help |

## Architecture

- Global `window` keydown listener in capture phase (intercepts before xterm)
- Leader state machine: idle → leader → action → idle
- Status bar at bottom shows available keys when leader is active
- Actions dispatched via Svelte stores to decouple HotkeyManager from Sidebar
