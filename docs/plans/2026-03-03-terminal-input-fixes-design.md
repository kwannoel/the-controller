# Terminal Input Fixes Design

## Problems

### Cmd-V paste (#4)
Tauri's macOS webview doesn't fire the browser paste event to xterm's internal textarea. xterm.js relies on the browser `paste` event for clipboard integration, so paste silently fails.

### Shift-Enter (#5)
xterm.js sends `\r` for both Enter and Shift-Enter. Claude Code uses the kitty keyboard protocol (CSI u) where Shift-Enter is `\x1b[13;2u`. xterm.js doesn't support CSI u natively, so Claude Code can't distinguish the two keys.

## Fix

Both fixes go in `Terminal.svelte` via `attachCustomKeyEventHandler`:

**Cmd-V:** Intercept Meta+V (macOS) keydown, read clipboard via `navigator.clipboard.readText()`, write text to PTY. Return `false` to suppress xterm's default handling.

**Shift-Enter:** Intercept Shift+Enter keydown, send CSI u sequence `\x1b[13;2u` to PTY. Return `false` to suppress xterm sending `\r`.

**Risk:** tmux sits between xterm and Claude Code. If tmux strips CSI u sequences, we'll need to enable `extended-keys` in the tmux session config (`tmux.rs`).

## Files

- `src/lib/Terminal.svelte` — add `attachCustomKeyEventHandler`
- Possibly `src-tauri/src/tmux.rs` — enable `extended-keys` if needed
