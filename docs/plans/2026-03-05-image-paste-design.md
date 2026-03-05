# Image Paste Support — Design

> **Issue:** #30 — Cannot paste images in the Claude chat

## Problem

When the clipboard contains an image (screenshot, copied image), pressing Cmd+V does nothing. xterm.js only handles text paste. Claude Code's TUI detects paste via bracket paste mode (`\x1b[200~...\x1b[201~`) and then reads the system clipboard for images — but if xterm.js never sends the paste sequence to the PTY, Claude Code never triggers its image-from-clipboard check.

## Approach A (this attempt)

Intercept Cmd+V in the terminal key handler. When the clipboard contains an image, send an empty bracket paste sequence to the PTY to trigger Claude Code's paste handler, which reads the system clipboard directly.

## Flow

```
User presses Cmd+V
  -> Custom key handler detects Meta+V on keydown
  -> Async: read clipboard image via Tauri plugin readImage()
  -> If image found: send \x1b[200~\x1b[201~ to PTY via write_to_pty
  -> Claude Code detects bracket paste, reads system clipboard, finds image
  -> If no image: let xterm.js handle natively (text paste)
```

## Risk

Claude Code may not be able to read the system clipboard from within a PTY/tmux session. If this doesn't work, pivot to approach B: save image to temp file and send path.
