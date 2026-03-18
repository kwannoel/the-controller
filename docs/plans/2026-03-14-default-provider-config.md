# Default Provider Config Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use the-controller-executing-plans to implement this plan task-by-task.

**Goal:** Add a persisted `default_provider` app-config option so startup can default the foreground provider to Codex or Claude Code.

**Architecture:** Extend the Rust app config model with a serde-defaulted provider enum that preserves backward compatibility for existing `config.json` files. Return that field through onboarding/config reads, then map the persisted config value into the frontend's in-memory `selectedSessionProvider` store during app startup while leaving `Cmd+T` runtime-only.

**Tech Stack:** Rust, serde, Tauri commands, Svelte 5, Vitest

---

### Task 1: Add failing backend config coverage

**Files:**
- Modify: `src-tauri/src/config.rs`

**Step 1: Write the failing test**

Add tests that assert:
- saving/loading a config preserves `default_provider`
- loading legacy config JSON without `default_provider` defaults to Claude Code

**Step 2: Run test to verify it fails**

Run: `cd src-tauri && cargo test config`
Expected: FAIL because `Config` does not yet expose `default_provider`.

**Step 3: Write minimal implementation**

Add a config provider enum with serde support and a default value, then thread it through `Config`.

**Step 4: Run test to verify it passes**

Run: `cd src-tauri && cargo test config`
Expected: PASS.

### Task 2: Add failing frontend startup coverage

**Files:**
- Modify: `src/App.test.ts`
- Modify: `src/lib/stores.ts`

**Step 1: Write the failing test**

Add tests that assert:
- startup config `{ default_provider: "codex" }` initializes `selectedSessionProvider` to `"codex"`
- startup config without the field leaves it at `"claude"`

**Step 2: Run test to verify it fails**

Run: `pnpm test -- src/App.test.ts`
Expected: FAIL because startup only stores `projects_root`.

**Step 3: Write minimal implementation**

Extend the frontend `Config` type and map `default_provider` into the runtime provider store during startup/onboarding completion.

**Step 4: Run test to verify it passes**

Run: `pnpm test -- src/App.test.ts`
Expected: PASS.

### Task 3: Verify targeted regression surface

**Files:**
- Modify as needed: `src/App.svelte`, `src/lib/Onboarding.svelte`, `src-tauri/src/commands.rs`

**Step 1: Run targeted validation**

Run:
- `cd src-tauri && cargo test config`
- `pnpm test -- src/App.test.ts`

**Step 2: Confirm behavior**

Expected:
- legacy configs remain readable
- `codex` config initializes foreground provider to Codex
- `Cmd+T` behavior remains runtime-only
