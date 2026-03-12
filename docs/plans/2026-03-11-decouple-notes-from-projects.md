# Decouple Notes from Projects — Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use the-controller-executing-plans to implement this plan task-by-task.

**Goal:** Make notes fully independent from projects, organized by user-defined folders.

**Architecture:** Rename `project_name` → `folder` throughout the notes backend and frontend. Add folder CRUD commands. Rewrite NotesTree to list folders instead of projects. No filesystem migration — existing `~/.the-controller/notes/{name}/` directories become folders.

**Tech Stack:** Rust (Tauri v2), Svelte 5, xterm.js

---

### Task 1: Rename `project_name` → `folder` in `notes.rs` (backend core)

**Files:**
- Modify: `src-tauri/src/notes.rs`

**Step 1: Write the failing test for `list_folders`**

Add to the `#[cfg(test)] mod tests` block in `src-tauri/src/notes.rs`:

```rust
#[test]
fn test_list_folders() {
    let tmp = TempDir::new().unwrap();
    let base = tmp.path();

    // No folders yet
    let folders = list_folders(base).unwrap();
    assert!(folders.is_empty());

    // Create some folders by creating notes in them
    create_note(base, "work", "task1").unwrap();
    create_note(base, "personal", "diary").unwrap();

    let mut folders = list_folders(base).unwrap();
    folders.sort();
    assert_eq!(folders, vec!["personal", "work"]);
}

#[test]
fn test_create_folder() {
    let tmp = TempDir::new().unwrap();
    let base = tmp.path();

    create_folder(base, "my-folder").unwrap();
    let folders = list_folders(base).unwrap();
    assert_eq!(folders, vec!["my-folder"]);
}

#[test]
fn test_create_folder_already_exists() {
    let tmp = TempDir::new().unwrap();
    let base = tmp.path();

    create_folder(base, "dup").unwrap();
    let result = create_folder(base, "dup");
    assert!(result.is_err());
    assert_eq!(result.unwrap_err().kind(), std::io::ErrorKind::AlreadyExists);
}

#[test]
fn test_rename_folder() {
    let tmp = TempDir::new().unwrap();
    let base = tmp.path();

    create_note(base, "old-name", "note1").unwrap();
    rename_folder(base, "old-name", "new-name").unwrap();

    let folders = list_folders(base).unwrap();
    assert_eq!(folders, vec!["new-name"]);

    // Note should still be readable under new folder
    let content = read_note(base, "new-name", "note1.md").unwrap();
    assert_eq!(content, "# note1\n");
}

#[test]
fn test_rename_folder_target_exists() {
    let tmp = TempDir::new().unwrap();
    let base = tmp.path();

    create_folder(base, "a").unwrap();
    create_folder(base, "b").unwrap();
    let result = rename_folder(base, "a", "b");
    assert!(result.is_err());
    assert_eq!(result.unwrap_err().kind(), std::io::ErrorKind::AlreadyExists);
}

#[test]
fn test_delete_folder_empty() {
    let tmp = TempDir::new().unwrap();
    let base = tmp.path();

    create_folder(base, "empty").unwrap();
    delete_folder(base, "empty", false).unwrap();
    let folders = list_folders(base).unwrap();
    assert!(folders.is_empty());
}

#[test]
fn test_delete_folder_nonempty_without_force() {
    let tmp = TempDir::new().unwrap();
    let base = tmp.path();

    create_note(base, "has-notes", "note1").unwrap();
    let result = delete_folder(base, "has-notes", false);
    assert!(result.is_err());
}

#[test]
fn test_delete_folder_nonempty_with_force() {
    let tmp = TempDir::new().unwrap();
    let base = tmp.path();

    create_note(base, "has-notes", "note1").unwrap();
    delete_folder(base, "has-notes", true).unwrap();
    let folders = list_folders(base).unwrap();
    assert!(folders.is_empty());
}

#[test]
fn test_delete_folder_nonexistent_is_ok() {
    let tmp = TempDir::new().unwrap();
    let result = delete_folder(tmp.path(), "nope", false);
    assert!(result.is_ok());
}
```

**Step 2: Run tests to verify they fail**

Run: `cd src-tauri && cargo test notes::tests -- --nocapture 2>&1 | head -30`
Expected: Compilation errors — `list_folders`, `create_folder`, `rename_folder`, `delete_folder` don't exist yet.

**Step 3: Rename `project_name` → `folder` and implement new functions**

In `src-tauri/src/notes.rs`:

1. Rename all `project_name` parameters to `folder` across every function (`notes_dir`, `notes_dir_with_base`, `list_notes`, `read_note`, `note_exists`, `write_note`, `create_note`, `rename_note`, `duplicate_note`, `delete_note`). Update doc comments to say "folder" instead of "project".

2. Add a `validate_folder_name` function (reuse same logic as `validate_filename`).

3. Add the four new public functions:

```rust
/// Returns the root notes directory under a custom base path.
pub fn notes_root_with_base(base: &std::path::Path) -> PathBuf {
    base.join("notes")
}

/// List all folder names (subdirectories) under the notes root, sorted alphabetically.
pub fn list_folders(base: &std::path::Path) -> std::io::Result<Vec<String>> {
    let root = notes_root_with_base(base);
    if !root.exists() {
        return Ok(Vec::new());
    }
    let mut folders = Vec::new();
    for entry in fs::read_dir(&root)? {
        let entry = entry?;
        if entry.file_type()?.is_dir() {
            if let Some(name) = entry.file_name().to_str() {
                folders.push(name.to_string());
            }
        }
    }
    folders.sort();
    Ok(folders)
}

/// Create an empty folder. Returns error if it already exists.
pub fn create_folder(base: &std::path::Path, name: &str) -> std::io::Result<()> {
    validate_folder_name(name)?;
    let dir = notes_dir_with_base(base, name);
    if dir.exists() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::AlreadyExists,
            format!("folder '{}' already exists", name),
        ));
    }
    fs::create_dir_all(&dir)
}

/// Rename a folder. Returns error if target already exists.
pub fn rename_folder(
    base: &std::path::Path,
    old_name: &str,
    new_name: &str,
) -> std::io::Result<()> {
    validate_folder_name(old_name)?;
    validate_folder_name(new_name)?;
    let old_dir = notes_dir_with_base(base, old_name);
    let new_dir = notes_dir_with_base(base, new_name);
    if !old_dir.exists() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("folder '{}' not found", old_name),
        ));
    }
    if new_dir.exists() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::AlreadyExists,
            format!("folder '{}' already exists", new_name),
        ));
    }
    fs::rename(old_dir, new_dir)
}

/// Delete a folder. If `force` is false, fails when the folder is non-empty.
/// Returns Ok(()) if the folder doesn't exist (idempotent).
pub fn delete_folder(base: &std::path::Path, name: &str, force: bool) -> std::io::Result<()> {
    validate_folder_name(name)?;
    let dir = notes_dir_with_base(base, name);
    if !dir.exists() {
        return Ok(());
    }
    if force {
        fs::remove_dir_all(&dir)
    } else {
        fs::remove_dir(&dir) // fails if non-empty
    }
}
```

4. Add `validate_folder_name`:

```rust
fn validate_folder_name(name: &str) -> std::io::Result<()> {
    if name.contains('/') || name.contains('\\') || name.contains("..") || name.is_empty() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!("invalid folder name: {}", name),
        ));
    }
    Ok(())
}
```

5. Update existing test names/comments: `test_notes_are_project_scoped` → `test_notes_are_folder_scoped`, etc.

**Step 4: Run tests to verify they pass**

Run: `cd src-tauri && cargo test notes::tests -- --nocapture`
Expected: All tests pass.

**Step 5: Commit**

```bash
git add src-tauri/src/notes.rs
git commit -m "refactor: rename project_name to folder in notes.rs, add folder CRUD"
```

---

### Task 2: Update Tauri command layer

**Files:**
- Modify: `src-tauri/src/commands/notes.rs`
- Modify: `src-tauri/src/commands.rs` (lines 1348-1410)
- Modify: `src-tauri/src/lib.rs` (lines 109-115)

**Step 1: Update `commands/notes.rs`**

Rename all `project_name: String` parameters to `folder: String` in every function. Update the calls to `notes::*` to pass `&folder` instead of `&project_name`.

Add three new command handler functions:

```rust
pub(crate) fn list_folders(
    state: State<'_, AppState>,
) -> Result<Vec<String>, String> {
    let base_dir = state
        .storage
        .lock()
        .map_err(|e| e.to_string())?
        .base_dir();
    notes::list_folders(&base_dir).map_err(|e| e.to_string())
}

pub(crate) fn create_folder(
    state: State<'_, AppState>,
    name: String,
) -> Result<(), String> {
    let base_dir = state
        .storage
        .lock()
        .map_err(|e| e.to_string())?
        .base_dir();
    notes::create_folder(&base_dir, &name).map_err(|e| e.to_string())
}

pub(crate) fn rename_folder(
    state: State<'_, AppState>,
    old_name: String,
    new_name: String,
) -> Result<(), String> {
    let base_dir = state
        .storage
        .lock()
        .map_err(|e| e.to_string())?
        .base_dir();
    notes::rename_folder(&base_dir, &old_name, &new_name).map_err(|e| e.to_string())
}

pub(crate) fn delete_folder(
    state: State<'_, AppState>,
    name: String,
    force: bool,
) -> Result<(), String> {
    let base_dir = state
        .storage
        .lock()
        .map_err(|e| e.to_string())?
        .base_dir();
    notes::delete_folder(&base_dir, &name, force).map_err(|e| e.to_string())
}
```

**Step 2: Update `commands.rs` Tauri wrappers**

Rename `project_name` → `folder` in all existing note commands (lines 1348-1410). Add new Tauri command wrappers:

```rust
#[tauri::command]
pub fn list_folders(
    state: State<'_, AppState>,
) -> Result<Vec<String>, String> {
    notes::list_folders(state)
}

#[tauri::command]
pub fn create_folder(
    state: State<'_, AppState>,
    name: String,
) -> Result<(), String> {
    notes::create_folder(state, name)
}

#[tauri::command]
pub fn rename_folder(
    state: State<'_, AppState>,
    old_name: String,
    new_name: String,
) -> Result<(), String> {
    notes::rename_folder(state, old_name, new_name)
}

#[tauri::command]
pub fn delete_folder(
    state: State<'_, AppState>,
    name: String,
    force: bool,
) -> Result<(), String> {
    notes::delete_folder(state, name, force)
}
```

**Step 3: Register new commands in `lib.rs`**

Add to the `.invoke_handler(tauri::generate_handler![...])` block (after the existing note commands around line 115):

```rust
commands::list_folders,
commands::create_folder,
commands::rename_folder,
commands::delete_folder,
```

**Step 4: Run Rust compilation check**

Run: `cd src-tauri && cargo check`
Expected: Compiles successfully.

**Step 5: Commit**

```bash
git add src-tauri/src/commands/notes.rs src-tauri/src/commands.rs src-tauri/src/lib.rs
git commit -m "refactor: update Tauri note commands to use folder, add folder CRUD commands"
```

---

### Task 3: Update server.rs API endpoints

**Files:**
- Modify: `src-tauri/src/bin/server.rs` (lines 39-59 routes, lines 478-578 handlers)

**Step 1: Rename `projectName` → `folder` in all note API handlers**

In every `api_*_note` function, change:
- `args["projectName"]` → `args["folder"]`
- `"missing projectName"` → `"missing folder"`
- `let project_name` → `let folder`
- Pass `&folder` to `notes::*` calls

**Step 2: Add new API endpoints for folder CRUD**

Add routes (around line 59):

```rust
.route("/api/list_folders", post(api_list_folders))
.route("/api/create_folder", post(api_create_folder))
.route("/api/rename_folder", post(api_rename_folder))
.route("/api/delete_folder", post(api_delete_folder))
```

Add handler functions:

```rust
async fn api_list_folders(
    AxumState(state): AxumState<Arc<ServerState>>,
    Json(_args): Json<Value>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let base_dir = state.app.storage.lock()
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .base_dir();
    let folders = notes::list_folders(&base_dir)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(serde_json::to_value(folders).unwrap()))
}

async fn api_create_folder(
    AxumState(state): AxumState<Arc<ServerState>>,
    Json(args): Json<Value>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let name = args["name"].as_str()
        .ok_or_else(|| (StatusCode::BAD_REQUEST, "missing name".to_string()))?.to_string();
    let base_dir = state.app.storage.lock()
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .base_dir();
    notes::create_folder(&base_dir, &name)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(Value::Null))
}

async fn api_rename_folder(
    AxumState(state): AxumState<Arc<ServerState>>,
    Json(args): Json<Value>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let old_name = args["oldName"].as_str()
        .ok_or_else(|| (StatusCode::BAD_REQUEST, "missing oldName".to_string()))?.to_string();
    let new_name = args["newName"].as_str()
        .ok_or_else(|| (StatusCode::BAD_REQUEST, "missing newName".to_string()))?.to_string();
    let base_dir = state.app.storage.lock()
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .base_dir();
    notes::rename_folder(&base_dir, &old_name, &new_name)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(Value::Null))
}

async fn api_delete_folder(
    AxumState(state): AxumState<Arc<ServerState>>,
    Json(args): Json<Value>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let name = args["name"].as_str()
        .ok_or_else(|| (StatusCode::BAD_REQUEST, "missing name".to_string()))?.to_string();
    let force = args["force"].as_bool().unwrap_or(false);
    let base_dir = state.app.storage.lock()
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .base_dir();
    notes::delete_folder(&base_dir, &name, force)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(Value::Null))
}
```

**Step 3: Run compilation check**

Run: `cd src-tauri && cargo check`
Expected: Compiles successfully.

**Step 4: Commit**

```bash
git add src-tauri/src/bin/server.rs
git commit -m "refactor: update server.rs note APIs to use folder, add folder CRUD endpoints"
```

---

### Task 4: Update frontend stores and types

**Files:**
- Modify: `src/lib/stores.ts`

**Step 1: Update store types**

Make these changes in `src/lib/stores.ts`:

1. Change `activeNote` type from `{ projectId: string; filename: string }` to `{ folder: string; filename: string }`:

```typescript
export const activeNote = writable<{
  folder: string;
  filename: string;
} | null>(null);
```

2. Add `noteFolders` store:

```typescript
export const noteFolders = writable<string[]>([]);
```

3. Update `FocusTarget` — change `note` and `notes-editor` variants:

```typescript
export type FocusTarget =
  | { type: "terminal"; projectId: string }
  | { type: "session"; sessionId: string; projectId: string }
  | { type: "project"; projectId: string }
  | { type: "agent"; agentKind: AgentKind; projectId: string }
  | { type: "agent-panel"; agentKind: AgentKind; projectId: string }
  | { type: "note"; filename: string; folder: string }
  | { type: "notes-editor"; folder: string; entryKey?: string }
  | null;
```

4. Update `HotkeyAction` — change note-related actions:

```typescript
  | { type: "create-note" }
  | { type: "delete-note"; folder: string; filename: string }
  | { type: "rename-note"; folder: string; filename: string }
  | { type: "duplicate-note"; folder: string; filename: string }
  | { type: "rename-folder"; folder: string }
  | { type: "delete-folder"; folder: string }
```

**Step 2: Verify no TypeScript errors introduced yet (will have downstream errors)**

Run: `npx tsc --noEmit 2>&1 | head -40`
Expected: Errors in downstream files that reference the old types — this is expected and will be fixed in subsequent tasks.

**Step 3: Commit**

```bash
git add src/lib/stores.ts
git commit -m "refactor: update stores to decouple notes from projects"
```

---

### Task 5: Update NotesEditor.svelte

**Files:**
- Modify: `src/lib/NotesEditor.svelte`

**Step 1: Replace project lookups with folder**

In `src/lib/NotesEditor.svelte`:

1. Remove the `projectsState`, `projectList` derivation (lines 25-26). Notes no longer need the project list.

2. Replace `projectName` derived variable (lines 34-38) with:

```typescript
let folderName = $derived(currentNote?.folder ?? null);
```

3. Update `loadNote` call (line 74) to use `folderName`:

```typescript
if (currentNote && folderName && key) {
    loadNote(folderName, currentNote.filename, key);
```

4. Update `prevNoteKey` (line 53) — the key format changes from `${projectId}:${filename}` to `${folder}:${filename}`:

```typescript
const key = currentNote ? `${currentNote.folder}:${currentNote.filename}` : null;
```

5. Update the flush logic (lines 64-69) — find folder name from prev key instead of projectId:

```typescript
const [prevFolder, ...rest] = prev.split(":");
const prevFilename = rest.join(":");
if (prevFolder && prevFilename) {
    command("write_note", { folder: prevFolder, filename: prevFilename, content: prevContent }).catch(() => {});
}
```

6. Update `loadNote` function — swap `projectName` → `folder` in command call:

```typescript
async function loadNote(folder: string, filename: string, requestKey: string) {
    loading = true;
    try {
        const text = await command<string>("read_note", { folder, filename });
```

7. Update `saveNow` — swap `projectName` → `folderName` in command call:

```typescript
async function saveNow() {
    if (saveTimer) { clearTimeout(saveTimer); saveTimer = null; }
    if (!currentNote || !folderName || content === savedContent) return;
    try {
        await command("write_note", { folder: folderName, filename: currentNote.filename, content });
```

8. Update `handleEditorEscape` — swap `projectId` → `folder`:

```typescript
if (currentNote) {
    focusTarget.set({ type: "note", filename: currentNote.filename, folder: currentNote.folder });
}
```

9. Remove unused imports: `projects`, `type Project` from the import line.

**Step 2: Verify TypeScript compiles for this file**

Run: `npx tsc --noEmit 2>&1 | grep NotesEditor`
Expected: No errors for NotesEditor.svelte specifically (other files may still have errors).

**Step 3: Commit**

```bash
git add src/lib/NotesEditor.svelte
git commit -m "refactor: decouple NotesEditor from projects, use folder directly"
```

---

### Task 6: Rewrite NotesTree.svelte

**Files:**
- Modify: `src/lib/sidebar/NotesTree.svelte`

**Step 1: Rewrite the component**

Replace the entire `NotesTree.svelte` with a folder-based implementation:

```svelte
<script lang="ts">
  import { command } from "$lib/backend";
  import { fromStore } from "svelte/store";
  import { noteEntries, type NoteEntry, type FocusTarget } from "../stores";

  interface Props {
    folders: string[];
    expandedFolderSet: Set<string>;
    currentFocus: FocusTarget;
    onToggleFolder: (folder: string) => void;
    onFolderFocus: (folder: string) => void;
    onNoteFocus: (filename: string, folder: string) => void;
    onNoteSelect: (filename: string, folder: string) => void;
  }

  let { folders, expandedFolderSet, currentFocus, onToggleFolder, onFolderFocus, onNoteFocus, onNoteSelect }: Props = $props();

  const noteEntriesState = fromStore(noteEntries);
  let noteMap: Map<string, NoteEntry[]> = $derived(noteEntriesState.current);

  function isFolderFocused(folder: string): boolean {
    return currentFocus?.type === "folder" && currentFocus.folder === folder;
  }

  function isNoteFocused(folder: string, filename: string): boolean {
    if (!currentFocus) return false;
    if (currentFocus.type === "note" && currentFocus.folder === folder && currentFocus.filename === filename) return true;
    return false;
  }

  function getNotesForFolder(folder: string): NoteEntry[] {
    return noteMap.get(folder) ?? [];
  }

  function noteCount(folder: string): number {
    return getNotesForFolder(folder).length;
  }

  function displayName(filename: string): string {
    return filename.endsWith(".md") ? filename.slice(0, -3) : filename;
  }

  function fetchNotes(folder: string) {
    command<NoteEntry[]>("list_notes", { folder }).then((entries) => {
      noteEntries.update((map) => {
        const next = new Map(map);
        next.set(folder, entries);
        return next;
      });
    });
  }

  $effect(() => {
    for (const folder of folders) {
      if (expandedFolderSet.has(folder)) {
        fetchNotes(folder);
      }
    }
  });
</script>

{#each folders as folder (folder)}
  <div class="folder-item">
    <!-- svelte-ignore a11y_no_noninteractive_tabindex -->
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <div
      class="folder-header"
      class:focus-target={isFolderFocused(folder)}
      tabindex="0"
      data-folder-id={folder}
      onfocusin={(e: FocusEvent) => {
        if (e.target === e.currentTarget) onFolderFocus(folder);
      }}
    >
      <button class="btn-expand" onclick={() => onToggleFolder(folder)}>
        {expandedFolderSet.has(folder) ? "\u25BC" : "\u25B6"}
      </button>
      <span class="folder-name">{folder}</span>
      <span class="note-count">{noteCount(folder)}</span>
    </div>

    {#if expandedFolderSet.has(folder)}
      <div class="note-list">
        {#each getNotesForFolder(folder) as note (note.filename)}
          <!-- svelte-ignore a11y_no_noninteractive_tabindex -->
          <div
            class="note-item"
            class:focus-target={isNoteFocused(folder, note.filename)}
            data-note-id="{folder}:{note.filename}"
            tabindex="0"
            onfocusin={() => onNoteFocus(note.filename, folder)}
            ondblclick={() => onNoteSelect(note.filename, folder)}
          >
            <span class="note-name">{displayName(note.filename)}</span>
          </div>
        {/each}

        {#if getNotesForFolder(folder).length === 0}
          <div class="empty-notes">No notes yet — press <kbd>n</kbd></div>
        {/if}
      </div>
    {/if}
  </div>
{/each}

{#if folders.length === 0}
  <div class="empty">No folders — press <kbd>n</kbd> to create a note</div>
{/if}
```

Keep the existing `<style>` block but rename CSS classes: `.project-item` → `.folder-item`, `.project-header` → `.folder-header`, `.project-name` → `.folder-name`. The styles themselves are identical.

**Important:** The `FocusTarget` type needs a `"folder"` variant for sidebar folder focus. Add to `stores.ts`:

```typescript
| { type: "folder"; folder: string }
```

**Step 2: Commit**

```bash
git add src/lib/sidebar/NotesTree.svelte src/lib/stores.ts
git commit -m "refactor: rewrite NotesTree to use folders instead of projects"
```

---

### Task 7: Update NewNoteModal with folder picker

**Files:**
- Modify: `src/lib/NewNoteModal.svelte`

**Step 1: Add folder selector**

Update `NewNoteModal.svelte` to accept `folders` prop and add a folder picker:

- Add `folders: string[]` to the Props interface
- Add a `folder` state variable
- Change `onSubmit` from `(title: string) => void` to `(title: string, folder: string) => void`
- Add a combo input: a `<select>` dropdown of existing folders + an `<input>` for typing a new folder name
- Disable Create button until both title and folder are non-empty
- Call `onSubmit(title.trim(), folder.trim())` on submit

The UI should have:
- A "Folder" label with a `<select>` of existing folders plus a "New folder..." option
- When "New folder..." is selected, show a text input for the new folder name
- A "Title" label with text input (existing)
- Cancel / Create buttons

**Step 2: Commit**

```bash
git add src/lib/NewNoteModal.svelte
git commit -m "feat: add folder picker to NewNoteModal"
```

---

### Task 8: Update Sidebar.svelte (note action handlers)

**Files:**
- Modify: `src/lib/Sidebar.svelte`

**Step 1: Update note-related state and handlers**

1. Import `noteFolders` from stores. Remove note-related dependencies on `projects`/`projectList`.

2. Change `deleteNoteTarget` type from `{ projectId: string; filename: string }` to `{ folder: string; filename: string }`.

3. Change `renameNoteTarget` type similarly.

4. Replace `newNoteProjectId` with no project dependency — the modal now handles folder selection.

5. Update `handleCreateNote` to accept `(title: string, folder: string)`:

```typescript
async function handleCreateNote(title: string, folder: string) {
    showNewNoteModal = false;
    try {
        const filename: string = await command("create_note", { folder, title });
        const notes = await command<NoteEntry[]>("list_notes", { folder });
        noteEntries.update(m => { const next = new Map(m); next.set(folder, notes); return next; });
        // Expand folder and open note
        expandedProjects.update(s => { const next = new Set(s); next.add(folder); return next; });
        activeNote.set({ folder, filename });
        focusTarget.set({ type: "notes-editor", folder });
        // Refresh folder list in case a new folder was created
        const folders = await command<string[]>("list_folders", {});
        noteFolders.set(folders);
    } catch (e) {
        showToast(String(e), "error");
    }
}
```

6. Update `handleDeleteNote`, `handleRenameNote`, `handleDuplicateNote` — replace all `projectId` / `project.name` references with `folder`:

```typescript
async function handleDeleteNote(folder: string, filename: string) {
    try {
        await command("delete_note", { folder, filename });
        const notes = await command<NoteEntry[]>("list_notes", { folder });
        noteEntries.update(m => { const next = new Map(m); next.set(folder, notes); return next; });
        const an = activeNoteState.current;
        if (an?.folder === folder && an?.filename === filename) {
            activeNote.set(null);
        }
        focusTarget.set({ type: "folder", folder });
        showToast("Note deleted", "info");
    } catch (e) {
        showToast(String(e), "error");
    }
}
```

Similarly for `handleRenameNote` and `handleDuplicateNote`.

7. Add folder action handlers:

```typescript
async function handleRenameFolder(oldName: string, newName: string) {
    try {
        await command("rename_folder", { oldName, newName });
        const folders = await command<string[]>("list_folders", {});
        noteFolders.set(folders);
        // Update expanded set
        expandedProjects.update(s => {
            const next = new Set(s);
            if (next.has(oldName)) { next.delete(oldName); next.add(newName); }
            return next;
        });
        // Update noteEntries key
        noteEntries.update(m => {
            const next = new Map(m);
            const entries = next.get(oldName);
            if (entries) { next.delete(oldName); next.set(newName, entries); }
            return next;
        });
        // Update activeNote if in renamed folder
        const an = activeNoteState.current;
        if (an?.folder === oldName) {
            activeNote.set({ folder: newName, filename: an.filename });
        }
        focusTarget.set({ type: "folder", folder: newName });
        showToast("Folder renamed", "info");
    } catch (e) {
        showToast(String(e), "error");
    }
}

async function handleDeleteFolder(folder: string) {
    try {
        await command("delete_folder", { name: folder, force: true });
        const folders = await command<string[]>("list_folders", {});
        noteFolders.set(folders);
        const an = activeNoteState.current;
        if (an?.folder === folder) {
            activeNote.set(null);
        }
        showToast("Folder deleted", "info");
    } catch (e) {
        showToast(String(e), "error");
    }
}
```

8. Update the `NotesTree` usage in the template — pass folders instead of projects:

```svelte
{:else if currentMode === "notes"}
  <NotesTree
    folders={folderList}
    expandedFolderSet={expandedProjectSet}
    {currentFocus}
    onToggleFolder={toggleProject}
    onFolderFocus={(folder) => {
      focusTarget.set({ type: "folder", folder });
    }}
    onNoteFocus={(filename, folder) => {
      focusTarget.set({ type: "note", filename, folder });
    }}
    onNoteSelect={(filename, folder) => {
      activeNote.set({ folder, filename });
      focusTarget.set({ type: "notes-editor", folder });
    }}
  />
```

9. Add folder list loading: fetch folders on mount and derive `folderList`.

```typescript
const noteFoldersState = fromStore(noteFolders);
let folderList: string[] = $derived(noteFoldersState.current);
```

Load folders in the `loadProjects` effect or a separate effect:

```typescript
$effect(() => {
    command<string[]>("list_folders", {}).then(folders => {
        noteFolders.set(folders);
    });
});
```

10. Update the `create-note` hotkey handler — no longer needs to infer project:

```typescript
case "create-note": {
    showNewNoteModal = true;
    break;
}
```

11. Update `NewNoteModal` usage to pass folders and the new `onSubmit` signature:

```svelte
{#if showNewNoteModal}
  <NewNoteModal
    folders={folderList}
    onSubmit={handleCreateNote}
    onClose={() => { showNewNoteModal = false; }}
  />
{/if}
```

12. Add folder rename/delete action handling in the hotkey subscriber. Add `RenameFolderModal` (or reuse `RenameNoteModal` pattern).

**Step 2: Commit**

```bash
git add src/lib/Sidebar.svelte
git commit -m "refactor: update Sidebar note/folder handlers to use folder instead of project"
```

---

### Task 9: Update HotkeyManager.svelte

**Files:**
- Modify: `src/lib/HotkeyManager.svelte`

**Step 1: Update imports and stores**

Add `noteFolders` to imports from stores.

```typescript
import {
    ...
    noteFolders,
    ...
} from "./stores";
```

Add:

```typescript
const noteFoldersState = fromStore(noteFolders);
let noteFolderList = $derived(noteFoldersState.current);
```

**Step 2: Update `SidebarItem` type and `getVisibleItems`**

Add `"folder"` variant to `SidebarItem`:

```typescript
type SidebarItem =
    | { type: "project"; projectId: string }
    | { type: "session"; sessionId: string; projectId: string }
    | { type: "agent"; agentKind: "auto-worker" | "maintainer"; projectId: string }
    | { type: "folder"; folder: string }
    | { type: "note"; filename: string; folder: string };
```

Update the `notes` branch of `getVisibleItems()`:

```typescript
if (currentMode === "notes") {
    const result: SidebarItem[] = [];
    for (const folder of noteFolderList) {
        result.push({ type: "folder", folder });
        if (!expandedSet.has(folder)) continue;
        const notes = noteEntriesMap.get(folder) ?? [];
        for (const n of notes) {
            result.push({ type: "note", filename: n.filename, folder });
        }
    }
    return result;
}
```

**Step 3: Update `navigateItem` to handle folder/note types**

In `navigateItem`, update the index finding and focus setting:

```typescript
} else if (currentFocus?.type === "folder") {
    idx = items.findIndex(it => it.type === "folder" && it.folder === currentFocus.folder);
} else if (currentFocus?.type === "note") {
    idx = items.findIndex(it => it.type === "note" && it.folder === currentFocus.folder && it.filename === currentFocus.filename);
}
```

And in the focus setting:

```typescript
} else if (next.type === "folder") {
    focusTarget.set({ type: "folder", folder: next.folder });
} else if (next.type === "note") {
    focusTarget.set({ type: "note", filename: next.filename, folder: next.folder });
}
```

**Step 4: Update `navigateProject`**

In notes mode, this should navigate between folders instead of projects:

```typescript
function navigateProject(direction: 1 | -1) {
    if (currentMode === "notes") {
        if (noteFolderList.length === 0) return;
        const focusedFolder = currentFocus?.type === "folder" ? currentFocus.folder
            : currentFocus?.type === "note" ? currentFocus.folder
            : currentFocus?.type === "notes-editor" ? currentFocus.folder
            : null;
        let idx = -1;
        if (focusedFolder) idx = noteFolderList.indexOf(focusedFolder);
        const len = noteFolderList.length;
        const next = noteFolderList[((idx + direction) % len + len) % len];
        focusTarget.set({ type: "folder", folder: next });
        return;
    }
    // ... existing project navigation
}
```

**Step 5: Update `getFocusedProject`**

Add folder/note types to the check (they no longer have `projectId`):

```typescript
function getFocusedProject(): Project | null {
    if (currentFocus?.type === "project" || currentFocus?.type === "session" || currentFocus?.type === "agent" || currentFocus?.type === "agent-panel") {
        return projectList.find((p) => p.id === currentFocus.projectId) ?? null;
    }
    return null;
}
```

Remove `note` and `notes-editor` from this function since they no longer have `projectId`.

**Step 6: Update hotkey action dispatch**

Update note-related hotkey actions to pass `folder` instead of `projectId`:

```typescript
case "delete-note":
    if (currentFocus?.type === "note") {
        dispatchAction({ type: "delete-note", folder: currentFocus.folder, filename: currentFocus.filename });
    }
    return true;
case "rename-note":
    if (currentFocus?.type === "note") {
        dispatchAction({ type: "rename-note", folder: currentFocus.folder, filename: currentFocus.filename });
    }
    return true;
case "duplicate-note":
    if (currentFocus?.type === "note") {
        dispatchAction({ type: "duplicate-note", folder: currentFocus.folder, filename: currentFocus.filename });
    }
    return true;
```

**Step 7: Update `expand-collapse` for notes**

```typescript
} else if (currentFocus?.type === "folder") {
    const next = new Set(expandedSet);
    if (next.has(currentFocus.folder)) {
        next.delete(currentFocus.folder);
    } else {
        next.add(currentFocus.folder);
    }
    expandedProjects.set(next);
} else if (currentFocus?.type === "note") {
    activeNote.set({ folder: currentFocus.folder, filename: currentFocus.filename });
    const vimKeys = ["o", "i", "a"];
    focusTarget.set({ type: "notes-editor", folder: currentFocus.folder, entryKey: vimKeys.includes(key) ? key : undefined });
}
```

**Step 8: Update Escape handler**

Change the `note` escape case to navigate to folder:

```typescript
} else if (currentFocus?.type === "note") {
    focusTarget.set({ type: "folder", folder: currentFocus.folder });
```

**Step 9: Update `delete` handling for folder context**

In `dispatchDeleteAction`, add folder handling:

```typescript
if (currentFocus?.type === "folder") {
    dispatchAction({ type: "delete-folder", folder: currentFocus.folder });
    return;
}
```

**Step 10: Commit**

```bash
git add src/lib/HotkeyManager.svelte
git commit -m "refactor: update HotkeyManager to navigate folders instead of projects in notes mode"
```

---

### Task 10: Update focus-helpers.ts and commands.ts

**Files:**
- Modify: `src/lib/focus-helpers.ts`
- Modify: `src/lib/commands.ts`

**Step 1: Update focus-helpers.ts**

Update `focusForModeSwitch` to handle the new `folder` focus type:

1. In the `"development"` branch, add `"folder"`:

```typescript
if (current.type === "agent" || current.type === "agent-panel" || current.type === "note" || current.type === "notes-editor" || current.type === "folder") {
```

For `folder` and `note`/`notes-editor` types (which no longer have `projectId`), return `null` instead of trying to use `current.projectId`:

```typescript
if (newMode === "development") {
    if (current.type === "folder" || current.type === "note" || current.type === "notes-editor") {
        // Notes are no longer project-scoped, fall back to active session or null
        if (activeSessionId) {
            const project = projectList.find(p => p.sessions.some(s => s.id === activeSessionId && !s.auto_worker_session));
            if (project) {
                return { type: "session", sessionId: activeSessionId, projectId: project.id };
            }
        }
        return projectList[0] ? { type: "project", projectId: projectList[0].id } : null;
    }
    if (current.type === "agent" || current.type === "agent-panel") {
        if (activeSessionId) {
            const project = projectList.find(p => p.id === current.projectId);
            if (project?.sessions.some(s => s.id === activeSessionId && !s.auto_worker_session)) {
                return { type: "session", sessionId: activeSessionId, projectId: current.projectId };
            }
        }
        return { type: "project", projectId: current.projectId };
    }
}
```

Similarly update `"agents"`, `"notes"`, `"architecture"`, and `"infrastructure"` branches to handle `folder` type.

2. Update `commands.ts` — update descriptions to say "folder" where relevant:

```typescript
{ id: "delete-note", key: "d", section: "Notes", description: "Delete focused note or folder", mode: "notes" },
{ id: "rename-note", key: "r", section: "Notes", description: "Rename focused note or folder", mode: "notes" },
```

**Step 2: Commit**

```bash
git add src/lib/focus-helpers.ts src/lib/commands.ts
git commit -m "refactor: update focus-helpers and commands for folder-based notes"
```

---

### Task 11: Update Sidebar.svelte focus effect for folder type

**Files:**
- Modify: `src/lib/Sidebar.svelte`

**Step 1: Add folder focus handling in the $effect block**

In the `$effect` that handles `currentFocus` changes (around lines 55-105), add a case for `folder`:

```typescript
} else if (currentFocus?.type === "folder") {
    if (!expandedProjectSet.has(currentFocus.folder)) {
        // Don't auto-expand, just focus the row
    }
    if (sidebarEl) {
        requestAnimationFrame(() => {
            const el = sidebarEl?.querySelector<HTMLElement>(`[data-folder-id="${currentFocus.folder}"]`);
            if (el) el.focus();
        });
    }
}
```

Update the `note` focus handler to use `folder` instead of `projectId`:

```typescript
} else if (currentFocus?.type === "note") {
    if (!expandedProjectSet.has(currentFocus.folder)) {
        const next = new Set(expandedProjectSet);
        next.add(currentFocus.folder);
        expandedProjects.set(next);
    }
    if (sidebarEl) {
        requestAnimationFrame(() => {
            const el = sidebarEl?.querySelector<HTMLElement>(`[data-note-id="${currentFocus.folder}:${currentFocus.filename}"]`);
            if (el) el.focus();
        });
    }
}
```

**Step 2: Commit**

```bash
git add src/lib/Sidebar.svelte
git commit -m "refactor: add folder focus handling in Sidebar"
```

---

### Task 12: Full compilation check and smoke test

**Step 1: Run TypeScript check**

Run: `npx tsc --noEmit`
Expected: No errors.

**Step 2: Run Rust tests**

Run: `cd src-tauri && cargo test`
Expected: All tests pass.

**Step 3: Run frontend tests**

Run: `npx vitest run`
Expected: All tests pass.

**Step 4: Dev server smoke test**

Run: `npm run tauri dev`
Manual verification:
- Switch to Notes workspace (Space → n)
- Existing folders (formerly project-named) appear in sidebar
- Can expand a folder and see notes
- Can create a new note (n key) — folder picker appears
- Can create a note in a new folder
- Can rename/delete notes with r/d keys
- Can navigate with j/k between folders and notes
- Editor loads and saves correctly
- Escape from note → folder → sidebar works

**Step 5: Commit any fixes**

```bash
git add -A
git commit -m "fix: resolve compilation and integration issues from notes decoupling"
```
