# The Controller

A desktop app for orchestrating multiple Claude Code terminal sessions.

Built with Tauri v2 + Svelte 5 + Rust.

## Development Setup

### Prerequisites

- [Rust](https://rustup.rs/) + Tauri v2
- [Node.js](https://nodejs.org/) + npm
- tmux (`brew install tmux`)

### tmux Configuration

If you use Claude Code inside tmux to develop this project, add the following to `~/.tmux.conf` for a cleaner UI:

```
set -g status off
```

Reload with `tmux source-file ~/.tmux.conf`.

`status off` hides the tmux status bar for a cleaner UI.

## Demo Ideas

- **Meta-programming lightshow:** Ask the editor to turn its background blue, then red, then yellow. Then ask it to cycle through the colors on intervals like a lightshow. The controller is editing itself in real-time.
