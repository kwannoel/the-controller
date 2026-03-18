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
- [Node.js](https://nodejs.org/) + pnpm
- espeak-ng (`brew install espeak-ng`) — required for voice mode TTS

Then:

```bash
pnpm install
pnpm tauri dev
```

Before considering local work finished, run:

```bash
pnpm check
cd src-tauri && cargo fmt --check
cd src-tauri && cargo clippy -- -D warnings
```

`pnpm check` is a real gate in this repo. Do not ignore warnings and do not assume Rust formatting is already clean.

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

## Server Mode (Headless)

The Controller can run as a standalone HTTP/WebSocket server for headless Linux deployments — no desktop or display required. The same Svelte UI is served as static files and accessed via a web browser.

### Build

```bash
pnpm build                                              # Vite frontend → dist/
cd src-tauri && cargo build --release --features server  # Axum server binary
```

### Run

```bash
CONTROLLER_DIST_DIR=../dist \
CONTROLLER_AUTH_TOKEN=mysecret \
./src-tauri/target/release/server
```

Then open `http://<host>:3001?token=mysecret` in a browser.

### Environment Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `CONTROLLER_PORT` | `3001` | HTTP listen port |
| `CONTROLLER_BIND` | `0.0.0.0` | Bind address |
| `CONTROLLER_AUTH_TOKEN` | *(none)* | Bearer token for API/WS auth. If unset, API/WS requests are limited to same-origin browser access or non-browser clients without an `Origin` header. |
| `CONTROLLER_DIST_DIR` | `./dist` (relative to binary) | Path to the Vite-built `dist/` directory |
| `CONTROLLER_SOCKET` | `/tmp/the-controller.sock` | Unix socket for session status hooks |

### Architecture

- `src-tauri/src/bin/server.rs` — Axum HTTP + WebSocket server with 40+ API routes
- `src/lib/backend.ts` — detects `__TAURI_INTERNALS__` and routes commands to Tauri IPC (desktop) or `fetch`/WebSocket (browser)
- `src-tauri/src/emitter.rs` — `WsBroadcastEmitter` pushes events over WebSocket instead of Tauri events
- `src/lib/platform.ts` — lazy-loads Tauri-only imports (`openUrl`, clipboard) with browser fallbacks

Desktop-only features (clipboard image copy, app screenshot, voice pipeline) return stubs in server mode.

### Deploy to a Linux Server

The server runs directly on the host (not in a container) because sessions need access to host projects and tools like `claude` and `git`.

```bash
# Build, install to ~/.the-controller, set up user systemd + optional Caddy HTTPS
./deploy/deploy.sh --host controller.example.com
```

The script handles: build, install, auth token generation, user-level systemd unit, and optional Caddy reverse proxy. No sudo needed for the service itself. See `deploy/` for the reference systemd unit and Caddyfile.

```bash
# Manage the service
systemctl --user status the-controller
journalctl --user -u the-controller -f

# Config lives at ~/.the-controller/server.env
```

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

## Claude vs Codex

Claude Code is the first-class development agent for this project. Most of the work is design — architecture, UX, interaction patterns, workflow shaping — where Claude's general reasoning, meta-thinking, and design sense make it the stronger choice. Claude also has better UX for interactive development.

Codex is useful for pushing out straightforward code at volume and handling background maintenance (dependency updates, mechanical refactors). It's a good fit for well-defined, independent tasks that can run in parallel.

Both agents are fully supported. Skills are symlinked to both on app startup, and `agents.md` serves as the shared instruction file. See [ARCHITECTURE.md](ARCHITECTURE.md) for details.

## Feature Discovery

Not sure what a feature does or how something works? Just ask Claude. The default setup has you clone the repository, so your agent already has full access to the source code and can trace through the implementation to answer your questions.

Or browse the docs directly:

- [Keyboard Shortcuts & Modes](docs/keyboard-modes.md) — all hotkeys, workspace modes, and how to stage/preview changes
- [Domain Knowledge](docs/domain-knowledge.md) — hard-won lessons about Tauri, PTY broker, session architecture, and server mode
- [Demo Recording](docs/demo.md) — how to record demos of The Controller

## Caveats

The Controller is a strongly opinionated power tool — built for efficiency, simplicity, and elegant design, maintained under a benevolent-dictator model. This is still a period of experimentation, so even the roadmap is unstable.

This project is in early stages. Some features may be overhauled or removed entirely without concern for backwards compatibility. Things will stabilize eventually, but not in the near term.

**Maintain your own fork.** This is the single best way to use The Controller without being caught off guard by breaking changes. Keep your customizations on your own branch and periodically rebase onto the latest commits from `main`. We may provide a skill (`the-controller-maintain-fork`) to automate this — PRs welcome.

Several things are still being refined, including the [contribution guide](CONTRIBUTING.md).
