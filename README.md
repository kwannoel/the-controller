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

## Secure Env CLI

The app now includes a companion CLI for secure `.env` editing:

```bash
cd src-tauri
cargo run --bin controller-cli -- env set --project <project-name> --key <ENV_KEY>
```

Behavior:

- The Controller app must already be running.
- The target project must already be known to The Controller.
- The CLI opens a secure modal in the app instead of reading the secret in the terminal.
- The CLI prints only redacted results such as `created OPENAI_API_KEY for demo-project`.

## Demo Ideas

- **Meta-programming lightshow:** Ask the editor to turn its background blue, then red, then yellow. Then ask it to cycle through the colors on intervals like a lightshow. The controller is editing itself in real-time.
