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

## Navigation & Features

### Switch Focus `esc` `l`

Move focus between the session terminal and the sidebar.

![Demo: Switch focus between terminal and sidebar](https://raw.githubusercontent.com/kwannoel/blog/main/demo-nav-esc-l.gif)

### Move Across Sessions `j/k`

Navigate up and down through sessions in the sidebar.

![Demo: Move across sessions](https://raw.githubusercontent.com/kwannoel/blog/main/demo-nav-jk.gif)

### Create & Delete Session `c` `d`

Create a new session or delete the selected one.

![Demo: Create and delete a session](https://raw.githubusercontent.com/kwannoel/blog/main/demo-nav-cd.gif)

### Create & Delete Project `n` `d`

Create a new project or delete the selected one.

![Demo: Create and delete a project](https://raw.githubusercontent.com/kwannoel/blog/main/demo-nav-nd.gif)

### Screenshot `cmd+shift+s`

One keystroke to capture the current view and save it to the project.

![Demo: Screenshot capability](https://raw.githubusercontent.com/kwannoel/blog/main/demo-screenshot.gif)

### Staging Modifications `v`

One keystroke to preview uncommitted changes.

![Demo: Staging modifications](https://raw.githubusercontent.com/kwannoel/blog/main/demo-staging.gif)

Together they close the loop: take a screenshot, have the agent inspect the UI with Playwright (`/the-controller-debugging-ui-with-playwright`), fix the issue, toggle staging to verify the fix.
