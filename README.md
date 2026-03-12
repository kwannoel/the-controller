# The Controller

A shapeable, personal desktop system — starting with terminal multiplexing.

Built with Tauri v2 + Svelte 5 + Rust.

> grug have many claude terminal. alt-tab alt-tab alt-tab. where thing go. brain so smooth. so tired.
> then grug find controller. all terminal one place. grug not lose thing no more.
> thing not right? grug grab club. beat to shape. grug tool now.
> complexity demon quiet. grug recommend.

## Running

Install prerequisites:

- [Rust](https://rustup.rs/) + Tauri v2
- [Node.js](https://nodejs.org/) + npm
- tmux (`brew install tmux`)

Then:

```bash
npm install
npm run tauri dev
```

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
