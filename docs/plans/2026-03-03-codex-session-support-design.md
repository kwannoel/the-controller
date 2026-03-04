# Codex Session Support via 'x' Key

## Goal

Press 'x' to create a session that runs `codex` instead of `claude`. No visual difference in sidebar.

## Approach

Add a `kind` field (`"claude"` | `"codex"`) that flows through the entire stack, defaulting to `"claude"` for backward compatibility.

## Changes

### Data model
- `SessionConfig` (Rust + TS): Add `kind` field, default `"claude"`

### Frontend
- `HotkeyManager.svelte`: Add 'x' key handler dispatching `create-session` with `kind: "codex"`
- `stores.ts`: Extend `HotkeyAction` and `SessionConfig` types with `kind`
- `Sidebar.svelte`: Pass `kind` through `createSession()` → `invoke("create_session", { projectId, kind })`

### Backend
- `commands.rs`: Accept `kind` param in `create_session`, save to `SessionConfig`
- `pty_manager.rs`: Pass `kind` to spawn methods, use it to select command
- `tmux.rs`: Use `kind` to spawn `claude` or `codex`

## Alternatives considered

Modal to choose session type — rejected as over-engineered since distinct hotkeys ('c' vs 'x') already disambiguate.
