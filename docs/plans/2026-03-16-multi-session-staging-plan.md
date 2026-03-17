# Multi-Session Staging Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use the-controller-executing-plans to implement this plan task-by-task.

**Goal:** Allow multiple sessions to be staged simultaneously, each running on its own port with its own socket.

**Architecture:** Change `staged_session: Option<StagedSession>` to `staged_sessions: Vec<StagedSession>` across Rust and TypeScript models. Make the staged socket path per-session. Update stage/unstage logic to operate on individual sessions rather than assuming one-at-a-time.

**Tech Stack:** Rust (Tauri v2, serde), Svelte 5, TypeScript

---

### Task 1: Rust model — migrate `staged_session` to `staged_sessions`

**Files:**
- Modify: `src-tauri/src/models.rs:18-20`

**Step 1: Write the failing test**

Add a test in `src-tauri/src/models.rs` that verifies a project with multiple staged sessions serializes/deserializes correctly:

```rust
#[test]
fn test_staged_sessions_multiple_roundtrip() {
    let project = Project {
        id: Uuid::new_v4(),
        name: "test".to_string(),
        repo_path: "/tmp".to_string(),
        created_at: "2026-03-16T00:00:00Z".to_string(),
        archived: false,
        maintainer: MaintainerConfig::default(),
        auto_worker: AutoWorkerConfig::default(),
        prompts: vec![],
        sessions: vec![],
        staged_sessions: vec![
            StagedSession { session_id: Uuid::new_v4(), pid: 1001, port: 2420 },
            StagedSession { session_id: Uuid::new_v4(), pid: 1002, port: 2421 },
        ],
    };
    let json = serde_json::to_string(&project).expect("serialize");
    let deserialized: Project = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(deserialized.staged_sessions.len(), 2);
    assert_eq!(deserialized.staged_sessions[0].port, 2420);
    assert_eq!(deserialized.staged_sessions[1].port, 2421);
}
```

Also add a migration test that verifies old `staged_session` JSON loads into the new field:

```rust
#[test]
fn test_staged_session_migration_from_old_format() {
    let json = r#"{
        "id": "550e8400-e29b-41d4-a716-446655440000",
        "name": "test",
        "repo_path": "/tmp",
        "created_at": "2026-03-16T00:00:00Z",
        "archived": false,
        "sessions": [],
        "staged_session": {
            "session_id": "550e8400-e29b-41d4-a716-446655440001",
            "pid": 12345,
            "port": 2420
        }
    }"#;
    let project: Project = serde_json::from_str(json).expect("deserialize old format");
    assert_eq!(project.staged_sessions.len(), 1);
    assert_eq!(project.staged_sessions[0].pid, 12345);
    assert_eq!(project.staged_sessions[0].port, 2420);
}
```

**Step 2: Run tests to verify they fail**

Run: `cd src-tauri && cargo test test_staged_sessions_multiple_roundtrip test_staged_session_migration_from_old_format -- --nocapture`
Expected: Compilation error — `staged_sessions` field doesn't exist yet.

**Step 3: Implement the model change**

In `src-tauri/src/models.rs`, replace:

```rust
    /// When a session is staged as a separate Controller instance.
    #[serde(default)]
    pub staged_session: Option<StagedSession>,
```

with:

```rust
    /// Sessions staged as separate Controller instances.
    #[serde(default, deserialize_with = "deserialize_staged_sessions")]
    pub staged_sessions: Vec<StagedSession>,
```

Add a custom deserializer that handles migration from old format. Place it before the `#[cfg(test)]` block:

```rust
/// Deserializes `staged_sessions` (Vec) from either the new array format
/// or the old `staged_session` (Option) format for backward compatibility.
fn deserialize_staged_sessions<'de, D>(deserializer: D) -> Result<Vec<StagedSession>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::de;

    #[derive(Deserialize)]
    #[serde(untagged)]
    enum StagedField {
        Many(Vec<StagedSession>),
        One(StagedSession),
    }

    // Try to deserialize staged_sessions field first.
    // If it's present and valid, use it.
    match StagedField::deserialize(deserializer) {
        Ok(StagedField::Many(v)) => Ok(v),
        Ok(StagedField::One(s)) => Ok(vec![s]),
        Err(_) => Ok(vec![]),
    }
}
```

Wait — serde's `deserialize_with` only applies to the field it annotates. The old JSON has `staged_session` (singular) not `staged_sessions` (plural), so a custom deserializer on the new field won't see the old data. We need a different approach.

Use serde's `alias` + a helper approach. The cleanest way is to keep both fields during deserialization and merge in a post-deserialization step. Use serde's `#[serde(flatten)]` with a helper struct:

Actually the simplest approach: use `#[serde(alias = "staged_session")]` so that the old field name maps to the new one, plus custom deserialization to handle `Option<StagedSession>` → `Vec<StagedSession>`:

```rust
    /// Sessions staged as separate Controller instances.
    /// Alias handles migration from old `staged_session` field name.
    #[serde(default, alias = "staged_session", deserialize_with = "deserialize_staged_sessions")]
    pub staged_sessions: Vec<StagedSession>,
```

The custom deserializer handles both cases — if the JSON value is an object (old singular format), wrap it in a vec. If it's an array (new format), use it directly. If it's null (old `None` format), return empty vec:

```rust
fn deserialize_staged_sessions<'de, D>(deserializer: D) -> Result<Vec<StagedSession>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde_json::Value;

    let value = Option::<Value>::deserialize(deserializer)?;
    match value {
        None => Ok(vec![]),
        Some(Value::Array(arr)) => {
            let sessions: Vec<StagedSession> = arr
                .into_iter()
                .map(|v| serde_json::from_value(v).map_err(serde::de::Error::custom))
                .collect::<Result<_, _>>()?;
            Ok(sessions)
        }
        Some(obj @ Value::Object(_)) => {
            let session: StagedSession = serde_json::from_value(obj).map_err(serde::de::Error::custom)?;
            Ok(vec![session])
        }
        Some(_) => Err(serde::de::Error::custom("unexpected staged_sessions value")),
    }
}
```

**Step 4: Update all `staged_session` references in models.rs**

Update all test Project constructors from `staged_session: None` to `staged_sessions: vec![]`.

Update existing staging tests:
- `test_staged_session_defaults_to_none` → assert `staged_sessions.is_empty()`
- `test_staged_session_roundtrip` → use `staged_sessions: vec![StagedSession { ... }]`, assert `staged_sessions[0]`
- `test_staged_session_new_format_roundtrip` → same pattern

**Step 5: Run tests to verify they pass**

Run: `cd src-tauri && cargo test -- --test-threads=1`
Expected: All model tests pass, compilation errors in other files (commands.rs, lib.rs, etc.) — that's expected, we'll fix those in subsequent tasks.

**Step 6: Commit**

```bash
git add src-tauri/src/models.rs
git commit -m "feat: migrate staged_session to staged_sessions vec with backward compat"
```

---

### Task 2: Socket path — make it per-session

**Files:**
- Modify: `src-tauri/src/status_socket.rs:12-17`

**Step 1: Change `staged_socket_path` to accept a session ID**

Replace:

```rust
const DEFAULT_STAGED_SOCKET_PATH: &str = "/tmp/the-controller-staged.sock";

/// Return the socket path used by staged Controller instances.
pub fn staged_socket_path() -> &'static str {
    DEFAULT_STAGED_SOCKET_PATH
}
```

with:

```rust
/// Return the socket path for a specific staged session.
pub fn staged_socket_path(session_id: &Uuid) -> String {
    format!("/tmp/the-controller-staged-{}.sock", session_id)
}
```

Add the `use uuid::Uuid;` import if not already present (it is already imported).

**Step 2: Run cargo check**

Run: `cd src-tauri && cargo check 2>&1 | head -40`
Expected: Compilation errors in callers (commands.rs, lib.rs) — expected, will fix next.

**Step 3: Commit**

```bash
git add src-tauri/src/status_socket.rs
git commit -m "feat: make staged_socket_path per-session"
```

---

### Task 3: Backend — update commands.rs for multi-session staging

**Files:**
- Modify: `src-tauri/src/commands.rs` (stage_session_core, unstage_session, stage_session)

**Step 1: Update `stage_session_core` (lines 854-1113)**

In the early validation block (lines 867-893), replace the single-session check:

```rust
        if let Some(staged) = &project.staged_session {
            // Check if the staged process is still alive
            #[cfg(unix)]
            let alive = unsafe { libc::kill(staged.pid as i32, 0) } == 0;
            #[cfg(not(unix))]
            let alive = false;
            if alive {
                return Err("A session is already staged — unstage it first".to_string());
            }
            // Stale record — kill orphaned children (e.g. Vite, esbuild that outlived
            // the process leader), clean up the socket, then clear the record.
            kill_process_group(staged.pid);
            let _ = std::fs::remove_file(crate::status_socket::staged_socket_path());
            let mut p = project.clone();
            p.staged_session = None;
            storage.save_project(&p).map_err(|e| e.to_string())?;
        }
```

with:

```rust
        // Check if this specific session is already staged
        if let Some(existing) = project.staged_sessions.iter().find(|s| s.session_id == session_id) {
            #[cfg(unix)]
            let alive = unsafe { libc::kill(existing.pid as i32, 0) } == 0;
            #[cfg(not(unix))]
            let alive = false;
            if alive {
                return Err("This session is already staged — unstage it first".to_string());
            }
            // Stale record — clean up
            kill_process_group(existing.pid);
            let stale_socket = crate::status_socket::staged_socket_path(&session_id);
            let _ = std::fs::remove_file(&stale_socket);
            let mut p = project.clone();
            p.staged_sessions.retain(|s| s.session_id != session_id);
            storage.save_project(&p).map_err(|e| e.to_string())?;
        }
```

In the socket env var (line 1073-1076), change:

```rust
        .env(
            "CONTROLLER_SOCKET",
            crate::status_socket::staged_socket_path(),
        )
```

to:

```rust
        .env(
            "CONTROLLER_SOCKET",
            crate::status_socket::staged_socket_path(&session_id),
        )
```

In the save block (lines 1091-1110), replace:

```rust
        project.staged_session = Some(StagedSession {
            session_id,
            pid,
            port,
        });
```

with:

```rust
        project.staged_sessions.push(StagedSession {
            session_id,
            pid,
            port,
        });
```

**Step 2: Update `unstage_session` (lines 1129-1150)**

Change the function signature to accept `session_id`:

```rust
#[tauri::command]
pub fn unstage_session(state: State<AppState>, project_id: String, session_id: String) -> Result<(), String> {
    let project_uuid = Uuid::parse_str(&project_id).map_err(|e| e.to_string())?;
    let session_uuid = Uuid::parse_str(&session_id).map_err(|e| e.to_string())?;

    let storage = state.storage.lock().map_err(|e| e.to_string())?;
    let mut project = storage
        .load_project(project_uuid)
        .map_err(|e| e.to_string())?;

    let idx = project
        .staged_sessions
        .iter()
        .position(|s| s.session_id == session_uuid)
        .ok_or("This session is not currently staged")?;

    let staged = project.staged_sessions.remove(idx);

    // Kill the staged Controller process group
    kill_process_group(staged.pid);

    // Clean up this session's socket
    let socket = crate::status_socket::staged_socket_path(&session_uuid);
    let _ = std::fs::remove_file(&socket);

    storage.save_project(&project).map_err(|e| e.to_string())?;
    Ok(())
}
```

**Step 3: Update any `staged_session: None` in test helpers**

Search for `staged_session: None` in commands.rs and replace with `staged_sessions: vec![]`.

**Step 4: Run cargo check**

Run: `cd src-tauri && cargo check 2>&1 | head -40`
Expected: May still have errors in lib.rs, status_socket.rs, storage.rs — will fix next.

**Step 5: Commit**

```bash
git add src-tauri/src/commands.rs
git commit -m "feat: stage_session_core and unstage_session support multiple staged sessions"
```

---

### Task 4: Backend — update lib.rs cleanup and remaining Rust files

**Files:**
- Modify: `src-tauri/src/lib.rs:155-168`
- Modify: `src-tauri/src/storage.rs` (any `staged_session: None` constructors)
- Modify: `src-tauri/src/status_socket.rs` (any `staged_session: None` constructors)
- Modify: `src-tauri/src/secure_env.rs` (any `staged_session: None` constructors)

**Step 1: Update app exit cleanup in lib.rs**

Replace the cleanup loop (lines 155-168):

```rust
                            for project in &inventory.projects {
                                if let Some(staged) = &project.staged_session {
                                    commands::kill_process_group(staged.pid);
                                    let _ =
                                        std::fs::remove_file(status_socket::staged_socket_path());
                                    // Clear stale staged_session record
                                    let mut p = project.clone();
                                    p.staged_session = None;
                                    let _ = storage.save_project(&p);
                                }
                            }
```

with:

```rust
                            for project in &inventory.projects {
                                if !project.staged_sessions.is_empty() {
                                    let mut p = project.clone();
                                    for staged in &project.staged_sessions {
                                        commands::kill_process_group(staged.pid);
                                        let _ = std::fs::remove_file(
                                            status_socket::staged_socket_path(&staged.session_id),
                                        );
                                    }
                                    p.staged_sessions.clear();
                                    let _ = storage.save_project(&p);
                                }
                            }
```

**Step 2: Update all `staged_session: None` in other Rust files**

In `storage.rs`, `status_socket.rs`, `secure_env.rs`: replace `staged_session: None` with `staged_sessions: vec![]` in any Project constructors (test helpers, etc.).

**Step 3: Run full Rust test suite**

Run: `cd src-tauri && cargo test -- --test-threads=1`
Expected: All tests pass.

**Step 4: Commit**

```bash
git add src-tauri/src/lib.rs src-tauri/src/storage.rs src-tauri/src/status_socket.rs src-tauri/src/secure_env.rs
git commit -m "feat: update all Rust references from staged_session to staged_sessions"
```

---

### Task 5: Frontend — update TypeScript model and stores

**Files:**
- Modify: `src/lib/stores.ts:113` and `274-275`

**Step 1: Update the Project interface**

Change:

```typescript
  staged_session: StagedSession | null;
```

to:

```typescript
  staged_sessions: StagedSession[];
```

**Step 2: Update the HotkeyAction type**

Change:

```typescript
  | { type: "unstage-session"; projectId: string }
```

to:

```typescript
  | { type: "unstage-session"; projectId: string; sessionId: string }
```

**Step 3: Commit**

```bash
git add src/lib/stores.ts
git commit -m "feat: update TypeScript Project interface for staged_sessions"
```

---

### Task 6: Frontend — update HotkeyManager

**Files:**
- Modify: `src/lib/HotkeyManager.svelte:339-349`

**Step 1: Update hotkey toggle logic**

Replace:

```typescript
      case "stage": {
        const stageProj = projectList.find((p) => p.staged_session !== null);
        if (stageProj) {
          dispatchHotkeyAction({ type: "unstage-session", projectId: stageProj.id });
        } else if (activeId) {
          const proj2 = projectList.find((p) => p.sessions.some((s) => s.id === activeId));
          if (proj2 && proj2.name === "the-controller") {
            dispatchHotkeyAction({ type: "stage-session", sessionId: activeId, projectId: proj2.id });
          }
        }
        return true;
      }
```

with:

```typescript
      case "stage": {
        if (!activeId) return true;
        const proj = projectList.find((p) => p.sessions.some((s) => s.id === activeId));
        if (!proj || proj.name !== "the-controller") return true;
        const isStaged = proj.staged_sessions.some((s) => s.session_id === activeId);
        if (isStaged) {
          dispatchHotkeyAction({ type: "unstage-session", projectId: proj.id, sessionId: activeId });
        } else {
          dispatchHotkeyAction({ type: "stage-session", sessionId: activeId, projectId: proj.id });
        }
        return true;
      }
```

**Step 2: Commit**

```bash
git add src/lib/HotkeyManager.svelte
git commit -m "feat: hotkey toggles staging on focused session"
```

---

### Task 7: Frontend — update Sidebar and ProjectTree

**Files:**
- Modify: `src/lib/Sidebar.svelte:198-199` and `473-481`
- Modify: `src/lib/sidebar/ProjectTree.svelte:79`

**Step 1: Update Sidebar unstageSession**

Change `unstageSession` to accept and pass `sessionId`:

```typescript
  async function unstageSession(projectId: string, sessionId: string) {
    try {
      await command("unstage_session", { projectId, sessionId });
      await loadProjects();
      showToast("Unstaged — stopped separate instance", "info");
    } catch (e) {
      showToast(String(e), "error");
    }
  }
```

Update the action handler:

```typescript
        case "unstage-session": {
          unstageSession(action.projectId, action.sessionId);
          break;
        }
```

**Step 2: Update ProjectTree badge**

Change:

```svelte
{#if project.staged_session?.session_id === session.id}
```

to:

```svelte
{#if project.staged_sessions?.some((s) => s.session_id === session.id)}
```

**Step 3: Commit**

```bash
git add src/lib/Sidebar.svelte src/lib/sidebar/ProjectTree.svelte
git commit -m "feat: sidebar and project tree support multiple staged sessions"
```

---

### Task 8: Fix test files

**Files:**
- Modify: `src/lib/Sidebar.test.ts`
- Modify: `src/lib/AgentDashboard.test.ts`
- Modify: `src/lib/sidebar/ProjectTree.test.ts`
- Modify: `src/lib/project-listing.test.ts`
- Modify: `src/lib/focus-helpers.test.ts`
- Modify: `src/lib/TerminalManager.test.ts`
- Modify: `src/lib/HotkeyManager.test.ts`
- Modify: `src/App.test.ts`

**Step 1: Replace all `staged_session: null` with `staged_sessions: []` in test files**

Search-and-replace across all test files.

For `src/App.test.ts`, also update the test that creates a `staged_session` object (around line 399):

```typescript
staged_session: {
    session_id: "...",
    pid: 12345,
    port: 2420,
}
```

becomes:

```typescript
staged_sessions: [{
    session_id: "...",
    pid: 12345,
    port: 2420,
}],
```

**Step 2: Run frontend tests**

Run: `pnpm test`
Expected: All tests pass.

**Step 3: Commit**

```bash
git add src/lib/Sidebar.test.ts src/lib/AgentDashboard.test.ts src/lib/sidebar/ProjectTree.test.ts src/lib/project-listing.test.ts src/lib/focus-helpers.test.ts src/lib/TerminalManager.test.ts src/lib/HotkeyManager.test.ts src/App.test.ts
git commit -m "test: update all test files for staged_sessions migration"
```

---

### Task 9: Full verification

**Step 1: Run Rust tests**

Run: `cd src-tauri && cargo test -- --test-threads=1`
Expected: All pass.

**Step 2: Run frontend tests**

Run: `pnpm test`
Expected: All pass.

**Step 3: Verify no remaining references to old field**

Run: `grep -r "staged_session[^s]" src/ src-tauri/src/ --include="*.rs" --include="*.ts" --include="*.svelte" | grep -v node_modules | grep -v target`
Expected: No matches (all references should now be `staged_sessions`).
