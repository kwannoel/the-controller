# Notes Remote Sync Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use the-controller-executing-plans to implement this plan task-by-task.

**Goal:** Push notes to a remote GitHub repository after each local commit, with error events surfaced to the frontend.

**Architecture:** Add `push_to_remote()` to `notes.rs` that shells out to `git push origin HEAD` if an `origin` remote exists. Integrate into `commands/notes.rs` so every `try_commit` and `commit_notes` call triggers a background push. On failure, emit a `notes-sync-error` Tauri event that the frontend displays as a toast.

**Tech Stack:** Rust (git2 for remote detection, `std::process::Command` for push), Svelte 5 (listen + showToast)

---

### Task 1: `push_to_remote` — no remote configured returns Ok

**Files:**
- Modify: `src-tauri/src/notes.rs`

**Step 1: Write the failing test**

Add to the `#[cfg(test)] mod tests` block in `src-tauri/src/notes.rs`:

```rust
#[test]
fn test_push_to_remote_noop_when_no_remote() {
    let tmp = TempDir::new().unwrap();
    let base = tmp.path();
    create_note(base, "proj", "hello").unwrap();
    commit_notes(base, "init").unwrap();

    // No remote configured — should succeed silently
    let result = push_to_remote(base);
    assert!(result.is_ok());
}
```

**Step 2: Run test to verify it fails**

Run: `(cd src-tauri && cargo test test_push_to_remote_noop_when_no_remote -- --nocapture)`
Expected: FAIL — `push_to_remote` not found

**Step 3: Write minimal implementation**

Add after `commit_notes` in `src-tauri/src/notes.rs`:

```rust
/// Push the notes repo to `origin` if a remote is configured.
/// Returns Ok(()) silently if no remote exists.
/// Shells out to `git push` to inherit the user's SSH agent and credential helpers.
pub fn push_to_remote(base: &Path) -> Result<(), String> {
    let repo = open_or_init_repo(base).map_err(|e| e.to_string())?;

    // Check if "origin" remote exists
    if repo.find_remote("origin").is_err() {
        return Ok(());
    }

    let notes_dir = notes_root(base);
    let output = std::process::Command::new("git")
        .args(["push", "origin", "HEAD"])
        .current_dir(&notes_dir)
        .output()
        .map_err(|e| format!("failed to run git push: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("git push failed: {}", stderr.trim()));
    }

    Ok(())
}
```

**Step 4: Run test to verify it passes**

Run: `(cd src-tauri && cargo test test_push_to_remote_noop_when_no_remote -- --nocapture)`
Expected: PASS

**Step 5: Commit**

```bash
git add src-tauri/src/notes.rs
git commit -m "feat: add push_to_remote for notes sync (no-op when no remote)"
```

---

### Task 2: `push_to_remote` — succeeds when remote exists

**Files:**
- Modify: `src-tauri/src/notes.rs`

**Step 1: Write the failing test**

Add to the test module:

```rust
#[test]
fn test_push_to_remote_pushes_when_remote_exists() {
    let tmp = TempDir::new().unwrap();
    let base = tmp.path();

    // Create a bare repo to act as the remote
    let remote_dir = tmp.path().join("remote.git");
    Repository::init_bare(&remote_dir).unwrap();

    // Create notes and commit
    create_note(base, "proj", "hello").unwrap();
    commit_notes(base, "init").unwrap();

    // Add remote
    let repo = Repository::open(notes_root(base)).unwrap();
    repo.remote("origin", remote_dir.to_str().unwrap()).unwrap();

    // Push should succeed
    let result = push_to_remote(base);
    assert!(result.is_ok(), "push failed: {:?}", result.err());

    // Verify remote received the commit
    let remote_repo = Repository::open_bare(&remote_dir).unwrap();
    let head = remote_repo.head().unwrap().peel_to_commit().unwrap();
    assert_eq!(head.message().unwrap(), "init");
}
```

**Step 2: Run test to verify it passes** (should already pass with the implementation from Task 1)

Run: `(cd src-tauri && cargo test test_push_to_remote_pushes_when_remote_exists -- --nocapture)`
Expected: PASS

**Step 3: Commit**

```bash
git add src-tauri/src/notes.rs
git commit -m "test: verify push_to_remote works with a local bare remote"
```

---

### Task 3: Integrate push into `commands/notes.rs`

**Files:**
- Modify: `src-tauri/src/commands/notes.rs`

**Step 1: Update `try_commit` to accept an emitter and push in background**

Replace the existing `try_commit` function and `commit_notes` function:

```rust
use std::sync::Arc;
use crate::emitter::EventEmitter;

/// Best-effort git commit + background push. Logs errors but doesn't fail the operation.
fn try_commit(base_dir: &std::path::Path, message: &str, emitter: &Arc<dyn EventEmitter>) {
    if let Err(e) = notes::commit_notes(base_dir, message) {
        eprintln!("notes git commit failed: {}", e);
        return;
    }
    spawn_push(base_dir, emitter);
}

/// Spawn a background thread to push notes to remote.
fn spawn_push(base_dir: &std::path::Path, emitter: &Arc<dyn EventEmitter>) {
    let base = base_dir.to_path_buf();
    let emitter = Arc::clone(emitter);
    std::thread::spawn(move || {
        if let Err(e) = notes::push_to_remote(&base) {
            eprintln!("notes sync failed: {}", e);
            let _ = emitter.emit("notes-sync-error", &e);
        }
    });
}
```

**Step 2: Update all callers of `try_commit` to pass the emitter**

Every function in `commands/notes.rs` that calls `try_commit` needs to extract the emitter from `AppState`. The `AppState` already has `pub emitter: Arc<dyn EventEmitter>`.

Update `create_note`:
```rust
pub(crate) fn create_note(
    state: State<'_, AppState>,
    folder: String,
    title: String,
) -> Result<String, String> {
    let base_dir = state.storage.lock().map_err(|e| e.to_string())?.base_dir();
    let filename = notes::create_note(&base_dir, &folder, &title).map_err(|e| e.to_string())?;
    try_commit(&base_dir, &format!("create {}/{}", folder, filename), &state.emitter);
    Ok(filename)
}
```

Apply the same pattern to: `rename_note`, `duplicate_note`, `delete_note`, `create_folder`, `rename_folder`, `delete_folder`.

Update `commit_notes`:
```rust
pub(crate) fn commit_notes(state: State<'_, AppState>) -> Result<bool, String> {
    let base_dir = state.storage.lock().map_err(|e| e.to_string())?.base_dir();
    let committed = notes::commit_notes(&base_dir, "update notes").map_err(|e| e.to_string())?;
    if committed {
        spawn_push(&base_dir, &state.emitter);
    }
    Ok(committed)
}
```

**Step 3: Verify it compiles**

Run: `(cd src-tauri && cargo test -- --nocapture 2>&1 | tail -5)`
Expected: All tests pass

**Step 4: Commit**

```bash
git add src-tauri/src/commands/notes.rs
git commit -m "feat: push notes to remote after every commit"
```

---

### Task 4: Frontend toast on sync error

**Files:**
- Modify: `src/lib/NotesEditor.svelte`

**Step 1: Add listener for `notes-sync-error`**

At the top of the `<script>` block in `NotesEditor.svelte`, add the import and listener:

```typescript
import { listen } from "$lib/backend";
import { showToast } from "./toast";
import { onDestroy } from "svelte";

const unlistenSyncError = listen<string>("notes-sync-error", (payload) => {
  showToast(`Notes sync failed: ${payload}`, "error");
});
onDestroy(() => unlistenSyncError());
```

**Step 2: Verify it compiles**

Run: `pnpm check`
Expected: No errors

**Step 3: Commit**

```bash
git add src/lib/NotesEditor.svelte
git commit -m "feat: show toast on notes remote sync failure"
```

---

### Task 5: Run all tests

**Step 1: Run Rust tests**

Run: `(cd src-tauri && cargo test -- --nocapture)`
Expected: All pass

**Step 2: Run frontend tests**

Run: `pnpm test`
Expected: All pass

**Step 3: Final commit if any fixes needed**
