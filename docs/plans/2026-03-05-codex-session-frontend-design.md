# Codex Session Frontend Support + Jump Mode Simplification

## Goal

Complete issue #7: press 'x' to create a codex session. Simplify jump mode to project-level only.

## Context

Backend already implements `kind` field on `SessionConfig` (Rust), threaded through `create_session` command, PTY spawn, and tmux. Frontend needs to pass `kind` through and add the 'x' keybinding.

## Changes

### 1. Jump Mode Simplification

**`HotkeyManager.svelte`** — `handleJumpKey()`:
- Remove `jumpPhase`, `jumpProjectId` states
- On match: focus the matched project and exit jump mode (no session phase)
- Remove `d`/`a` key handling that operated on jumped-to project

**`stores.ts`** — `JumpPhase` type:
- Simplify to `{ phase: "project" } | null` (remove session variant)

### 2. 'x' Keybinding

**`HotkeyManager.svelte`** — `handleHotkey()`:
- Add `case "x":` mirroring `case "c":` but dispatching `kind: "codex"`

### 3. Frontend `kind` Plumbing

**`stores.ts`**:
- Add `kind: string` to `SessionConfig`
- Add `kind?: string` to `create-session` action

**`Sidebar.svelte`**:
- `create-session` handler passes `action.kind` to `createSession()`
- `createSession(projectId, kind?)` passes `kind` to `invoke("create_session", { projectId, kind })`

## Not changing

- Backend (already done)
- `JUMP_KEYS` array (still used for label generation; jump mode intercepts keys before `handleHotkey`)
