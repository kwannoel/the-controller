# Changelog

## v0.5.0

### Features

- **Notes workspace** — full notes editor with Notion-like live preview, vim keybindings, visual mode AI chat, image support, note duplication (`y`), and git version control (#251, #364, #400, #407, #426, #431, #437)
- **Notes decoupled from projects** — notes are independent entities stored in their own folders, not tied to projects (#430)
- **Staging system** — stage branches via a separate controller instance with single-action commit + rebase + stage, in-place staging for worktree branches, auto-rebase before staging (#272, #296, #319, #324, #417)
- **Workspace modes** — Development and Agents modes with `l` key agent panel focus, expandable agent sub-items, and focus target translation between modes (#217, #228, #231, #234)
- **Auto-worker** — background coding agent with eligible issue panel, work policy display, restart-safe completion, and issue reports with merged PR verification (#211, #276, #313, #355, #377)
- **Retheme** — cold dominant B&W palette with Geist fonts, CSS custom properties for all colors (#405, #419)
- **Deployment workspace** — Hetzner + Cloudflare + Coolify deployment support (#406)
- **Architecture explorer** — interactive codebase architecture visualization (#393, #396)
- **Maintainer improvements** — issue browser with closed issue awareness, actionable reports via GitHub issues, reports wired to correct project (#240, #304)
- **Session token metrics** — token usage chart in summary pane (#382)
- **Prompt library** — prompt extraction and project prompt library (#263)
- **Consolidated issue modal** — single keyboard-driven modal for all issue operations (#433)
- **Command registry** — single-source-of-truth hotkey system (#188)
- **Summary pane for archived sessions** — view initial prompt and git progress for archived sessions (#209)
- **Issue creation complexity** — complexity prompt added to issue creation flow (#213)
- **Assigned issues panel** — shows assigned-but-uncompleted issues (#242)
- **Controller CLI** — install controller-cli to stable PATH location (#401)
- **Secure env modal** — CLI for managing project credentials (#392)
- **Frontend error forwarding** — JS errors forwarded to staging log via Tauri IPC (#423)
- **Browser-mode backend** — Playwright e2e test infrastructure (#363)
- **Background agents switched to Codex** — background agents use Codex instead of Claude (#291)
- **Symlink CLAUDE.md to agents.md** — automatic agents.md setup on project creation (#226)
- **Symlink .env into worktrees** — `.env` symlinked instead of copied (#404)

### Bug Fixes

- Fix staging race condition, zombie leak, and IPv6 port mismatch (#421)
- Clean up orphaned staging processes and log staging output (#420)
- Fix AI chat panel positioned off-screen when selection starts above viewport (#418)
- Normalize Shift+letter key events for vim plugin in WKWebView (#415)
- Prevent UUID stacking when duplicating notes and auto-open rename (#413)
- Fix `ga` trigger for AI chat when entire note is selected (#412)
- Clear native selection on visual mode exit to prevent WebKit highlight artifact (#402)
- Fix CodeMirror view destruction on every keystroke in vim insert mode (#390)
- Pass vim entry keys through to notes editor, stop HotkeyManager interception (#389)
- Prevent token chart from being clipped by long commit lists (#386)
- Route screenshot sessions to controller project (#384)
- Enable terminal scrolling via SGR mouse wheel sequences (#312)
- Prevent triage panel issue body from overflowing horizontally (#311)
- Fix AgentDashboard reactivity loop and undefined variable
- Translate focus target when switching workspace modes (#234)
- Filter escape sequences from prompt capture buffer (#218)
- Fuzzy finder arrow key double-fire and vim-style navigation (#216)
- Fix thinking level PTY writes and flush pending changes on idle (#214)
- Remove CLAUDECODE env var in maintainer health check (#201)
- Group issues by priority in issue picker modal (#199)
- Prevent duplicate labels in issue cache (#196)
- Scroll terminal to bottom on resize so cursor stays visible (#222)
- Prevent session status getting stuck on working (#221)
- Stop staging info from appearing in current window title (#427)
- Standardize issue label format to prevent triage mismatch (#388)
- Roll back failed session spawns (#347)
- Label completed worker issues on kill and restart recovery (#348)
- Recover auto-worker state on restart (#344)
- Recover partial worktree path migration (#352)
- Route codex merge Enter through raw PTY (#351)
- Handle startup storage init failures (#349)
- Resolve svelte-check CI failures (#434, #439)
- Resolve all clippy warnings for Rust 1.94 CI (#436)
- Default session provider to claude instead of codex (#391)
- Increase cleanup signal retry window to survive dev server restarts (#397)
- Various text color, triage UI, and label standardization fixes

### Refactoring

- Open-source readiness: MIT license, README cleanup (#398, #432, #435, #438)
- Simplify dev mode keys and restructure help overlay (#428)
- Simplify FuzzyFinder to single-phase selection (#229)
- Remove GlobalChat component and `g` keybinding (#395)
- Remove thinking mode toggles (#215)
- Remove archiving and obsolete jump mode

## v0.4.0

### Features

- **Maintainer background agent** — periodic code health checks with live countdown timer, `b` hotkey, and `r` to run checks (#132, #144, #146, #151)
- **Issue triage panel** — swiping-style triage with priority buckets, j/k hotkeys, complexity step, triaged labels, and category support (#147, #155, #172, #174, #181)
- **Screenshot-to-session** — Shift+S shortcut for screenshot sessions, cropped screenshots with optional preview (#170, #153)
- **Keystroke visualizer** — Cmd+K to toggle on-screen keystroke display (#185)
- **Global skills injection** — inject skills globally via home dir symlinks on startup (#186)
- **Clickable terminal links** — WebLinksAddon integration for clickable URLs in terminal (#137)
- **Issue creation flow** — two-stage keyboard flow with high/low priority labels and colored dot indicators (#157, #145, #165)
- **Merge hotkey improvements** — confirmation popup, squash-only merge for new projects, finishing-a-development-branch skill (#136, #113, #116)
- **Auto-cleanup worktrees** — automatic worktree cleanup after finishing a development branch (#126)
- **Keyboard shortcuts help** — categorized into sections (#173)
- **Sort issues by priority** — issue picker sorted by priority with colored indicators (#165)

### Bug Fixes

- Fix deadlock in handle_cleanup due to reversed lock ordering (#179)
- Fix scroll terminal to bottom when navigating to a session (#180)
- Fix triage hotkeys firing when modifier keys are held (#183)
- Show individually archived sessions in archive view (#175)
- Restore focus and active session after backend-initiated cleanup (#167)
- Handle session cleanup directly in backend instead of frontend (#162)
- Fix screenshot paste race conditions (#125, #130, #164)
- Fix PTY scroll race by registering output listener before connect_session (#134, #148)
- Fix terminal garbling by matching local PTY size to tmux on reattach
- Unarchive project when loading by repo_path (#133)
- Fix session branch name collisions with UUID suffix (#128)
- Fix codex merge hotkey and paste handling (#123, #127, #129)
- Pass THE_CONTROLLER_SESSION_ID via tmux -e flag for reliable env propagation (#131)
- Fix worktree-compatible finishing-a-development-branch skill (#119)
- Fix garbled terminal output when xterm.js opens in hidden container
- Eliminate extra newlines in sessions after restart

### Refactoring

- Modularize tauri command domains (#110)
- Simplify frontend architecture and remove obsolete UI glue
- Move triage ranking controls to right panel (#184)
- Move maintainer status from sidebar dot to panel text label (#171)
- Remove dead code and consolidate duplicate type definitions (#117)

## v0.3.0

### Features

- **GitHub issue integration** — assign issues to sessions, issue picker modal, issue context injection, in-progress labels (#22, #43)
- **Issue creation** — create GitHub issues via `i` hotkey with modal and toast notifications (#21, #31)
- **Task panel** — GitHub task list panel with `t` hotkey and auto-refetch (#12)
- **Session status hooks** — hook-based status events replacing PTY-output debounce (#28)
- **Merge hotkey** — merge session branch via `m` key with rebase + PR flow (#9, #20)
- **Drag-and-drop images** — handle drag-and-drop and paste images in terminal (#34, #36)
- **Codex sessions** — `x` for codex with issue picker, `X` for raw codex session
- **Session summary pane** — display initial prompt and git commit progress
- **Skill discovery** — Codex skill discovery via symlinks, project-local superpowers skills
- **Project scaffolding** — scaffold with template agents.md, GitHub remote creation (#25, #38)
- **Worktree naming** — use project name in worktree paths with migration (#14)
- **Background workers** — C/X hotkeys for autonomous background worker sessions
- **Persist session commits** — DONE state survives merge/rebase (#93)
- **GitHub issue cache** — in-memory GitHub issue cache (#89)
- **Image paste support** — paste images in Claude sessions (#30)
- **Three-state status indicator** — session status indicator with three states (#8)
- **Auto-start on issue** — auto-start assistant work when session has GitHub issue
- **.env copying** — copy .env from repo into new session worktrees

### Bug Fixes

- Fix merge conflict prompt sent to Claude Code session (#29)
- Fix idle transition flickering between tool calls (#28)
- Use carriage return instead of newline for PTY merge command
- Scope drag-drop image paste to active session only
- Gate terminal input during initialization to prevent spurious newlines
- Force terminal repaint and PTY resize when navigating back to session
- Use --append-system-prompt for issue context
- Fix issue picker keyboard nav and hooks format compatibility
- Show sidebar when escaping from terminal to session

### Testing

- Add 45 new Rust unit tests across all modules (#73)

## v0.2.0

### Features

- **Tmux-backed sessions** — sessions now run inside tmux for persistence across app restarts
- **Session archival** — archive/unarchive sessions with confirmation modals
- **Vim-style navigation** — `j`/`k` navigation, `l`/Enter to focus, fuzzy finder
- **Hotkey system** — Escape leader key, `HotkeyManager` state machine, status bar hints
- **Terminal focus management** — Esc→h for sidebar, Esc→l for terminal, double-Escape to unfocus
- **Sidebar redesign** — collapse/expand, create session hotkey, focus indicators
- **Focus-after-delete** — redirect focus after session or project deletion (#10)
- **Auto-focus terminal** — auto-focus terminal after 2s sidebar dwell
- **Session continuation** — use `claude --continue` for restored/unarchived sessions
- **Session kind field** — thread session kind through `create_session` and PTY spawning
- **Duplicate project rejection** — reject duplicate project names in create/load/scaffold
- **Keyboard shortcut help** — `HotkeyHelp` modal showing all keyboard shortcuts

### Bug Fixes

- Fix duplicate paste by dropping custom Cmd-V handler
- Block Shift+Enter on all event types to prevent double-send
- Pass `--continue` flag when unarchiving project sessions
- Route Shift-Enter through tmux `send-keys` to bypass outer terminal parser
- Clear dwell timer on single-Escape from session focus
- Fix Cmd-V paste popup and Shift-Enter tmux format mismatch
- Restore sessions on reboot instead of destroying them
- Use `project.archived` as source of truth for archive filtering
- Reset leader timeout on modifier keypress to allow shifted keys
- Guard modifier-only keys in leader mode

## v0.1.0

Initial release.
