# Codex Session Support Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Press 'x' to create a session running `codex` instead of `claude`.

**Architecture:** Add a `kind` field (defaulting to `"claude"`) to `SessionConfig` that flows from keybinding → Tauri command → PTY spawn. The `kind` determines which binary to execute.

**Tech Stack:** Rust (Tauri), Svelte 5, serde, portable-pty, tmux

---

### Task 1: Add `kind` field to Rust `SessionConfig`

**Files:**
- Modify: `src-tauri/src/models.rs:14-22`

**Step 1: Write the failing test**

Add to the existing `tests` module in `models.rs`:

```rust
#[test]
fn test_session_config_kind_defaults_to_claude() {
    // Simulate loading a persisted session that has no "kind" field (backward compat)
    let json = r#"{"id":"550e8400-e29b-41d4-a716-446655440000","label":"session-1","worktree_path":null,"worktree_branch":null,"archived":false}"#;
    let session: SessionConfig = serde_json::from_str(json).expect("deserialize");
    assert_eq!(session.kind, "claude");
}

#[test]
fn test_session_config_kind_codex() {
    let json = r#"{"id":"550e8400-e29b-41d4-a716-446655440000","label":"session-1","worktree_path":null,"worktree_branch":null,"archived":false,"kind":"codex"}"#;
    let session: SessionConfig = serde_json::from_str(json).expect("deserialize");
    assert_eq!(session.kind, "codex");
}
```

**Step 2: Run test to verify it fails**

Run: `cd src-tauri && cargo test test_session_config_kind`
Expected: FAIL — `SessionConfig` has no field `kind`

**Step 3: Write minimal implementation**

Add `kind` field to `SessionConfig` in `models.rs`:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionConfig {
    pub id: Uuid,
    pub label: String,
    pub worktree_path: Option<String>,
    pub worktree_branch: Option<String>,
    #[serde(default)]
    pub archived: bool,
    #[serde(default = "default_kind")]
    pub kind: String,
}

fn default_kind() -> String {
    "claude".to_string()
}
```

Also update the existing tests that construct `SessionConfig` to include `kind: "claude".to_string()`.

**Step 4: Run test to verify it passes**

Run: `cd src-tauri && cargo test`
Expected: ALL PASS

**Step 5: Commit**

```bash
git add src-tauri/src/models.rs
git commit -m "feat: add kind field to SessionConfig with claude default"
```

---

### Task 2: Thread `kind` through `create_session` command and PTY spawning

**Files:**
- Modify: `src-tauri/src/commands.rs:324-385` (accept `kind` param, save to config, pass to spawn)
- Modify: `src-tauri/src/pty_manager.rs:32-49` (accept `kind` param, pass to spawn methods)
- Modify: `src-tauri/src/pty_manager.rs:52-75` (use `kind` to select command in direct spawn)
- Modify: `src-tauri/src/tmux.rs:28-58` (accept `kind` param, use as tmux command)

**Step 1: Update `TmuxManager::create_session` to accept command**

In `src-tauri/src/tmux.rs`, change signature and usage:

```rust
pub fn create_session(session_id: Uuid, working_dir: &str, command: &str) -> Result<(), String> {
    let name = Self::session_name(session_id);
    let output = Command::new(TMUX_BIN)
        .args([
            "new-session",
            "-d",
            "-s",
            &name,
            "-c",
            working_dir,
            "-x",
            "80",
            "-y",
            "24",
            command,
        ])
        .env_remove("CLAUDECODE")
        .output()
        .map_err(|e| format!("failed to run tmux: {}", e))?;
    // ... rest unchanged
```

**Step 2: Update `PtyManager::spawn_session` to accept `kind`**

In `src-tauri/src/pty_manager.rs`:

```rust
pub fn spawn_session(
    &mut self,
    session_id: Uuid,
    working_dir: &str,
    kind: &str,
    app_handle: AppHandle,
) -> Result<(), String> {
    let command = match kind {
        "codex" => "codex",
        _ => "claude",
    };
    if TmuxManager::is_available() {
        if !TmuxManager::has_session(session_id) {
            TmuxManager::create_session(session_id, working_dir, command)?;
        }
        self.attach_tmux_session(session_id, app_handle)
    } else {
        self.spawn_direct_session(session_id, working_dir, command, app_handle)
    }
}
```

Update `spawn_direct_session` to accept `command: &str` and use it instead of hardcoded `"claude"`:

```rust
fn spawn_direct_session(
    &mut self,
    session_id: Uuid,
    working_dir: &str,
    command: &str,
    app_handle: AppHandle,
) -> Result<(), String> {
    // ...
    let mut cmd = CommandBuilder::new(command);
    cmd.cwd(working_dir);
    cmd.env_remove("CLAUDECODE");
    // ... rest unchanged
```

**Step 3: Update `create_session` command to accept and pass `kind`**

In `src-tauri/src/commands.rs`:

```rust
#[tauri::command]
pub fn create_session(
    state: State<AppState>,
    app_handle: AppHandle,
    project_id: String,
    kind: Option<String>,
) -> Result<String, String> {
    let kind = kind.unwrap_or_else(|| "claude".to_string());
    // ... existing code ...

    // In SessionConfig construction:
    let session_config = SessionConfig {
        id: session_id,
        label: label.clone(),
        worktree_path: wt_path,
        worktree_branch: wt_branch,
        archived: false,
        kind: kind.clone(),
    };

    // In spawn call:
    pty_manager.spawn_session(session_id, &session_dir, &kind, app_handle)?;
    // ...
```

**Step 4: Update all callers of `spawn_session` to pass `kind`**

In `commands.rs`, update `restore_sessions` (line 68):
```rust
pty_manager.spawn_session(session.id, &session_dir, &session.kind, app_handle.clone())
```

In `commands.rs`, update `unarchive_session` (line 472):
```rust
pty_manager.spawn_session(session_uuid, &session_dir, &kind, app_handle)?;
```
(Extract `kind` from the session before dropping storage lock.)

In `commands.rs`, update `unarchive_project` (line 296-297):
```rust
// Collect kind along with session info
let to_restore: Vec<(Uuid, String, String)> = project.sessions.iter()
    .filter(|s| s.archived)
    .map(|s| {
        let dir = s.worktree_path.clone().unwrap_or_else(|| project.repo_path.clone());
        (s.id, dir, s.kind.clone())
    })
    .collect();
// ...
for (session_id, session_dir, kind) in to_restore {
    pty_manager.spawn_session(session_id, &session_dir, &kind, app_handle.clone())?;
}
```

**Step 5: Run all tests**

Run: `cd src-tauri && cargo test`
Expected: ALL PASS (cargo build also verifies compilation)

**Step 6: Commit**

```bash
git add src-tauri/src/commands.rs src-tauri/src/pty_manager.rs src-tauri/src/tmux.rs
git commit -m "feat: thread session kind through create_session, PTY spawn, and tmux"
```

---

### Task 3: Add 'x' keybinding and frontend `kind` plumbing

**Files:**
- Modify: `src/lib/stores.ts:3-9` (add `kind` to `SessionConfig`)
- Modify: `src/lib/stores.ts:34` (add `kind` to `create-session` action)
- Modify: `src/lib/HotkeyManager.svelte:315-321` (add 'x' handler)
- Modify: `src/lib/Sidebar.svelte:142-149` (pass `kind` to `createSession`)
- Modify: `src/lib/Sidebar.svelte:281-305` (pass `kind` to `invoke`)

**Step 1: Update TypeScript types**

In `src/lib/stores.ts`, add `kind` to `SessionConfig`:

```typescript
export interface SessionConfig {
  id: string;
  label: string;
  worktree_path: string | null;
  worktree_branch: string | null;
  archived: boolean;
  kind: string;
}
```

Add `kind` to the `create-session` action:

```typescript
| { type: "create-session"; projectId?: string; kind?: string }
```

**Step 2: Add 'x' key handler in `HotkeyManager.svelte`**

After the `case "c":` block (around line 321), add:

```typescript
case "x":
  if (currentFocus?.type === "project") {
    dispatchAction({ type: "create-session", projectId: currentFocus.projectId, kind: "codex" });
  } else if (currentFocus?.type === "session") {
    dispatchAction({ type: "create-session", projectId: currentFocus.projectId, kind: "codex" });
  }
  return true;
```

**Step 3: Update Sidebar to pass `kind` through**

In `Sidebar.svelte`, update the `create-session` handler (around line 142-149):

```typescript
case "create-session": {
  const project = action.projectId
    ? projectList.find((p) => p.id === action.projectId)
    : (projectList.find((p) =>
        p.sessions.some((s) => s.id === activeSession),
      ) ?? projectList[0]);
  if (project) createSession(project.id, action.kind);
  break;
}
```

Update `createSession` function (around line 281):

```typescript
async function createSession(projectId: string, kind?: string) {
  try {
    const sessionId: string = await invoke("create_session", {
      projectId,
      kind: kind ?? "claude",
    });
    // ... rest unchanged
```

**Step 4: Build to verify**

Run: `npm run check` (or `npx svelte-check`)
Expected: No type errors

**Step 5: Commit**

```bash
git add src/lib/stores.ts src/lib/HotkeyManager.svelte src/lib/Sidebar.svelte
git commit -m "feat: add 'x' keybinding for codex sessions"
```

---

### Task 4: Manual smoke test

**Step 1: Run the app**

Run: `npm run tauri dev`

**Step 2: Test 'c' key**

- Focus a project in sidebar, press 'c'
- Verify a Claude session spawns as before

**Step 3: Test 'x' key**

- Focus a project in sidebar, press 'x'
- Verify a session spawns running `codex` (will error if codex isn't installed — that's expected and fine)

**Step 4: Test backward compatibility**

- Verify existing sessions (no `kind` field in JSON) load correctly and default to `claude`

**Step 5: Commit final**

```bash
git commit --allow-empty -m "feat: codex session support via 'x' key (closes #7)"
```
