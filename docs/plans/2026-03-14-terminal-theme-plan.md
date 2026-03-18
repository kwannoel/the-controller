# Terminal Theme Loading Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use the-controller-executing-plans to implement this plan task-by-task.

**Goal:** Load terminal colors from `~/.the-controller/current-theme.conf` using kitty-style syntax and apply them to xterm.js with a safe default fallback.

**Architecture:** Add a Rust theme-loading module and Tauri command that read from the app base directory, parse kitty-style colors, and return an xterm-compatible DTO. Update the frontend terminal setup to request that theme before creating the xterm instance, while keeping the current built-in theme as the fallback.

**Tech Stack:** Rust, Tauri v2, Svelte 5, xterm.js, Vitest

**Design doc:** `docs/plans/2026-03-14-terminal-theme-design.md`

---

### Task 1: Add failing Rust tests for config-path and theme loading

**Files:**
- Modify: `src-tauri/src/storage.rs`
- Create: `src-tauri/src/terminal_theme.rs`

**Step 1: Write the failing test**

Add Rust tests for:

- `Storage::default_base_dir(Some(home))` returning `home/.the-controller`
- Missing `current-theme.conf` returning the default theme
- Valid kitty-style config mapping to expected fields
- Invalid color values returning the default theme

**Step 2: Run test to verify it fails**

Run: `cd src-tauri && cargo test terminal_theme --lib`
Expected: FAIL because `terminal_theme` module and helpers do not exist yet.

**Step 3: Write minimal implementation**

Create `src-tauri/src/terminal_theme.rs` with:

- `TerminalTheme` struct
- `default_terminal_theme()`
- `load_terminal_theme(base_dir: &Path) -> TerminalTheme`

Update `Storage::default_base_dir()` to return `~/.the-controller`.

**Step 4: Run test to verify it passes**

Run: `cd src-tauri && cargo test terminal_theme --lib`
Expected: PASS

**Step 5: Commit**

```bash
git add src-tauri/src/storage.rs src-tauri/src/terminal_theme.rs
git commit -m "feat(theme): load terminal theme from kitty-style config"
```

### Task 2: Add failing command tests for theme loading exposure

**Files:**
- Modify: `src-tauri/src/commands.rs`
- Modify: `src-tauri/src/lib.rs`

**Step 1: Write the failing test**

Add command-level tests for a new `load_terminal_theme` command or helper path that prove the command returns the default theme without a file and a parsed theme when the file exists.

**Step 2: Run test to verify it fails**

Run: `cd src-tauri && cargo test load_terminal_theme --lib`
Expected: FAIL because the command is not wired yet.

**Step 3: Write minimal implementation**

Add an async Tauri command that uses `spawn_blocking` to read `<base_dir>/current-theme.conf` through the new theme module, and register it in `lib.rs`.

**Step 4: Run test to verify it passes**

Run: `cd src-tauri && cargo test load_terminal_theme --lib`
Expected: PASS

**Step 5: Commit**

```bash
git add src-tauri/src/commands.rs src-tauri/src/lib.rs
git commit -m "feat(theme): expose terminal theme loading command"
```

### Task 3: Add failing frontend tests for terminal theme consumption

**Files:**
- Modify: `src/lib/Terminal.svelte`
- Create: `src/lib/Terminal.test.ts`

**Step 1: Write the failing test**

Add Vitest coverage that:

- Mocks the backend `command("load_terminal_theme")`
- Verifies the returned theme is passed to `new Terminal`
- Verifies terminal creation still happens with the default theme when the command rejects

**Step 2: Run test to verify it fails**

Run: `pnpm test -- Terminal.test.ts`
Expected: FAIL because `Terminal.svelte` still hardcodes the theme.

**Step 3: Write minimal implementation**

Refactor `Terminal.svelte` to use a shared default theme constant, await the backend theme load during mount, and pass the resolved theme into xterm construction.

**Step 4: Run test to verify it passes**

Run: `pnpm test -- Terminal.test.ts`
Expected: PASS

**Step 5: Commit**

```bash
git add src/lib/Terminal.svelte src/lib/Terminal.test.ts
git commit -m "feat(theme): apply loaded terminal theme in xterm"
```

### Task 4: Run focused verification

**Files:**
- Modify: none

**Step 1: Run Rust verification**

Run: `cd src-tauri && cargo test terminal_theme --lib && cargo test load_terminal_theme --lib`
Expected: PASS

**Step 2: Run frontend verification**

Run: `pnpm test -- Terminal.test.ts`
Expected: PASS

**Step 3: Run adjacent sanity checks**

Run: `pnpm test -- TerminalManager.test.ts`
Expected: PASS

**Step 4: Review diff**

Run: `git diff --stat`
Expected: only theme-loading and config-path related files changed
