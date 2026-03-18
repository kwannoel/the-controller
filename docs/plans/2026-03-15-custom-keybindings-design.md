# Custom Keybindings Design

## Overview

Allow users to customize keyboard shortcuts via a config file at `~/.the-controller/keybindings`. Sparse overrides — only changed bindings are stored. File is hot-reloaded while the app is running.

## Config File Format

Path: `~/.the-controller/keybindings`

Vim-like format, one binding per line: `command-name key`. Lines starting with `#` are comments. On first launch, a template is generated with all bindings commented out so users can see available commands.

```
# Navigation
# navigate-next j
# navigate-prev k

# Sessions (development)
# create-session c
# screenshot Meta+s
```

Modifier keys use `Meta+` prefix (e.g. `Meta+s`, `Meta+t`).

## Architecture

### Backend (Rust)

- `src-tauri/src/keybindings.rs` — parse, validate, resolve overrides
- File watcher via `notify` crate, debounced 200ms
- On change: parse → validate → emit `keybindings-changed` Tauri event
- Tauri command: `load_keybindings`
- Template auto-generated on first launch via `ensure_keybindings_file`

### Frontend (Svelte)

- `commands.ts` keeps hardcoded defaults as source of truth for command metadata
- `applyOverrides(defaults, userBindings)` merges user overrides
- `buildKeyMap()` uses resolved commands
- `HotkeyManager.svelte` rebuilds keymap on `keybindings-changed` event
- `HotkeyHelp.svelte` shows resolved keys
- Toast for conflicts/warnings

### Data Flow

```
App start → Rust load_keybindings → parse → resolve → send to frontend
File change → notify → debounce → re-parse → emit event → frontend rebuilds keymap
```

### Graceful Degradation

- File missing → generate template, use defaults
- Parse error → keep last valid bindings, warning toast
- Duplicate key in same mode → last-wins (no detection in v1)
- Unknown command → skip line, warning toast

## Not in v1

- No in-app UI for editing keybindings
- No per-project overrides
- No multi-key sequences beyond existing Esc Esc
