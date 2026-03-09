# Notes Editor Workspace Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use the-controller-executing-plans to implement this plan task-by-task.

**Goal:** Add a "notes" workspace mode with a per-project markdown editor, file tree sidebar, auto-save, and edit/preview toggle.

**Architecture:** New workspace mode `"notes"` follows the existing pattern (development/agents). Rust backend provides file I/O commands for notes stored at `~/.the-controller/notes/{project-name}/`. Frontend adds a `NotesTree` sidebar component and `NotesEditor` main area component. Keyboard navigation follows existing patterns (j/k sidebar, l/Enter to open, Escape to exit editor).

**Tech Stack:** Svelte 5 (runes), Rust/Tauri commands, Catppuccin Mocha theme, plain textarea editor, simple regex markdown renderer.

---

### Task 1: Rust notes module — data types and list/read/write

**Files:**
- Create: `src-tauri/src/notes.rs`
- Modify: `src-tauri/src/lib.rs` (add `pub mod notes;`)

**Step 1: Write failing tests for notes module**

In `src-tauri/src/notes.rs`, add the module with types and tests first:

```rust
use std::fs;
use std::path::PathBuf;

use serde::Serialize;

/// A note file entry returned to the frontend.
#[derive(Debug, Clone, Serialize)]
pub struct NoteEntry {
    pub filename: String,
    pub modified_at: String, // ISO 8601
}

/// Return the notes directory for a project.
pub fn notes_dir(project_name: &str) -> PathBuf {
    let home = dirs::home_dir().expect("could not determine home directory");
    home.join(".the-controller").join("notes").join(project_name)
}

/// Return the notes directory for a project, using a custom base dir (for testing).
pub fn notes_dir_with_base(base: &std::path::Path, project_name: &str) -> PathBuf {
    base.join("notes").join(project_name)
}

/// List all `.md` files in a project's notes directory, sorted by modified time (newest first).
pub fn list_notes(base: &std::path::Path, project_name: &str) -> std::io::Result<Vec<NoteEntry>> {
    let dir = notes_dir_with_base(base, project_name);
    if !dir.exists() {
        return Ok(Vec::new());
    }
    let mut entries = Vec::new();
    for entry in fs::read_dir(&dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().map_or(false, |e| e == "md") {
            let filename = path.file_name().unwrap().to_string_lossy().to_string();
            let modified = entry.metadata()?.modified()?;
            let datetime: chrono::DateTime<chrono::Utc> = modified.into();
            entries.push(NoteEntry {
                filename,
                modified_at: datetime.to_rfc3339(),
            });
        }
    }
    entries.sort_by(|a, b| b.modified_at.cmp(&a.modified_at));
    Ok(entries)
}

/// Read the content of a note file.
pub fn read_note(base: &std::path::Path, project_name: &str, filename: &str) -> std::io::Result<String> {
    let path = notes_dir_with_base(base, project_name).join(filename);
    fs::read_to_string(path)
}

/// Write content to a note file, creating the directory if needed.
pub fn write_note(base: &std::path::Path, project_name: &str, filename: &str, content: &str) -> std::io::Result<()> {
    let dir = notes_dir_with_base(base, project_name);
    fs::create_dir_all(&dir)?;
    fs::write(dir.join(filename), content)
}

/// Create a new note with the given title (used as filename). Returns the entry.
pub fn create_note(base: &std::path::Path, project_name: &str, title: &str) -> std::io::Result<NoteEntry> {
    let filename = if title.ends_with(".md") {
        title.to_string()
    } else {
        format!("{}.md", title)
    };
    let dir = notes_dir_with_base(base, project_name);
    fs::create_dir_all(&dir)?;
    let path = dir.join(&filename);
    if path.exists() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::AlreadyExists,
            format!("Note '{}' already exists", filename),
        ));
    }
    fs::write(&path, format!("# {}\n", title.trim_end_matches(".md")))?;
    let modified = fs::metadata(&path)?.modified()?;
    let datetime: chrono::DateTime<chrono::Utc> = modified.into();
    Ok(NoteEntry {
        filename,
        modified_at: datetime.to_rfc3339(),
    })
}

/// Rename a note file.
pub fn rename_note(base: &std::path::Path, project_name: &str, old_name: &str, new_name: &str) -> std::io::Result<()> {
    let dir = notes_dir_with_base(base, project_name);
    let new_filename = if new_name.ends_with(".md") {
        new_name.to_string()
    } else {
        format!("{}.md", new_name)
    };
    let old_path = dir.join(old_name);
    let new_path = dir.join(&new_filename);
    if new_path.exists() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::AlreadyExists,
            format!("Note '{}' already exists", new_filename),
        ));
    }
    fs::rename(old_path, new_path)
}

/// Delete a note file.
pub fn delete_note(base: &std::path::Path, project_name: &str, filename: &str) -> std::io::Result<()> {
    let path = notes_dir_with_base(base, project_name).join(filename);
    if path.exists() {
        fs::remove_file(path)
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_list_notes_empty() {
        let tmp = TempDir::new().unwrap();
        let result = list_notes(tmp.path(), "my-project").unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn test_create_and_list_notes() {
        let tmp = TempDir::new().unwrap();
        create_note(tmp.path(), "my-project", "hello").unwrap();
        create_note(tmp.path(), "my-project", "world").unwrap();
        let notes = list_notes(tmp.path(), "my-project").unwrap();
        assert_eq!(notes.len(), 2);
        let filenames: Vec<&str> = notes.iter().map(|n| n.filename.as_str()).collect();
        assert!(filenames.contains(&"hello.md"));
        assert!(filenames.contains(&"world.md"));
    }

    #[test]
    fn test_create_note_adds_md_extension() {
        let tmp = TempDir::new().unwrap();
        let entry = create_note(tmp.path(), "proj", "my-note").unwrap();
        assert_eq!(entry.filename, "my-note.md");
    }

    #[test]
    fn test_create_note_preserves_md_extension() {
        let tmp = TempDir::new().unwrap();
        let entry = create_note(tmp.path(), "proj", "my-note.md").unwrap();
        assert_eq!(entry.filename, "my-note.md");
    }

    #[test]
    fn test_create_duplicate_note_fails() {
        let tmp = TempDir::new().unwrap();
        create_note(tmp.path(), "proj", "test").unwrap();
        let result = create_note(tmp.path(), "proj", "test");
        assert!(result.is_err());
    }

    #[test]
    fn test_read_note() {
        let tmp = TempDir::new().unwrap();
        create_note(tmp.path(), "proj", "test").unwrap();
        let content = read_note(tmp.path(), "proj", "test.md").unwrap();
        assert_eq!(content, "# test\n");
    }

    #[test]
    fn test_write_and_read_note() {
        let tmp = TempDir::new().unwrap();
        write_note(tmp.path(), "proj", "test.md", "hello world").unwrap();
        let content = read_note(tmp.path(), "proj", "test.md").unwrap();
        assert_eq!(content, "hello world");
    }

    #[test]
    fn test_rename_note() {
        let tmp = TempDir::new().unwrap();
        create_note(tmp.path(), "proj", "old-name").unwrap();
        rename_note(tmp.path(), "proj", "old-name.md", "new-name").unwrap();
        let notes = list_notes(tmp.path(), "proj").unwrap();
        assert_eq!(notes.len(), 1);
        assert_eq!(notes[0].filename, "new-name.md");
    }

    #[test]
    fn test_rename_to_existing_fails() {
        let tmp = TempDir::new().unwrap();
        create_note(tmp.path(), "proj", "a").unwrap();
        create_note(tmp.path(), "proj", "b").unwrap();
        let result = rename_note(tmp.path(), "proj", "a.md", "b");
        assert!(result.is_err());
    }

    #[test]
    fn test_delete_note() {
        let tmp = TempDir::new().unwrap();
        create_note(tmp.path(), "proj", "test").unwrap();
        delete_note(tmp.path(), "proj", "test.md").unwrap();
        let notes = list_notes(tmp.path(), "proj").unwrap();
        assert!(notes.is_empty());
    }

    #[test]
    fn test_delete_nonexistent_note_is_ok() {
        let tmp = TempDir::new().unwrap();
        let result = delete_note(tmp.path(), "proj", "nonexistent.md");
        assert!(result.is_ok());
    }

    #[test]
    fn test_notes_are_project_scoped() {
        let tmp = TempDir::new().unwrap();
        create_note(tmp.path(), "proj-a", "note1").unwrap();
        create_note(tmp.path(), "proj-b", "note2").unwrap();
        let a_notes = list_notes(tmp.path(), "proj-a").unwrap();
        let b_notes = list_notes(tmp.path(), "proj-b").unwrap();
        assert_eq!(a_notes.len(), 1);
        assert_eq!(b_notes.len(), 1);
        assert_eq!(a_notes[0].filename, "note1.md");
        assert_eq!(b_notes[0].filename, "note2.md");
    }
}
```

**Step 2: Register the module**

In `src-tauri/src/lib.rs`, add:
```rust
pub mod notes;
```

**Step 3: Run tests to verify they pass**

Run: `cd src-tauri && cargo test notes`
Expected: All 12 tests PASS

**Step 4: Commit**

```bash
git add src-tauri/src/notes.rs src-tauri/src/lib.rs
git commit -m "feat(notes): add notes module with CRUD operations and tests"
```

---

### Task 2: Tauri commands for notes

**Files:**
- Create: `src-tauri/src/commands/notes.rs`
- Modify: `src-tauri/src/commands.rs` (add `mod notes;`)
- Modify: `src-tauri/src/lib.rs` (register commands in invoke_handler)

**Step 1: Create Tauri command wrappers**

Create `src-tauri/src/commands/notes.rs`:

```rust
use crate::notes::{self, NoteEntry};
use crate::state::AppState;
use tauri::State;

#[tauri::command]
pub async fn list_notes(
    state: State<'_, AppState>,
    project_name: String,
) -> Result<Vec<NoteEntry>, String> {
    let storage = state.storage.lock().map_err(|e| e.to_string())?;
    let base = storage.base_dir();
    notes::list_notes(&base, &project_name).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn read_note(
    state: State<'_, AppState>,
    project_name: String,
    filename: String,
) -> Result<String, String> {
    let storage = state.storage.lock().map_err(|e| e.to_string())?;
    let base = storage.base_dir();
    notes::read_note(&base, &project_name, &filename).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn write_note(
    state: State<'_, AppState>,
    project_name: String,
    filename: String,
    content: String,
) -> Result<(), String> {
    let storage = state.storage.lock().map_err(|e| e.to_string())?;
    let base = storage.base_dir();
    notes::write_note(&base, &project_name, &filename, &content).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn create_note(
    state: State<'_, AppState>,
    project_name: String,
    title: String,
) -> Result<NoteEntry, String> {
    let storage = state.storage.lock().map_err(|e| e.to_string())?;
    let base = storage.base_dir();
    notes::create_note(&base, &project_name, &title).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn rename_note(
    state: State<'_, AppState>,
    project_name: String,
    old_name: String,
    new_name: String,
) -> Result<(), String> {
    let storage = state.storage.lock().map_err(|e| e.to_string())?;
    let base = storage.base_dir();
    notes::rename_note(&base, &project_name, &old_name, &new_name).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn delete_note(
    state: State<'_, AppState>,
    project_name: String,
    filename: String,
) -> Result<(), String> {
    let storage = state.storage.lock().map_err(|e| e.to_string())?;
    let base = storage.base_dir();
    notes::delete_note(&base, &project_name, &filename).map_err(|e| e.to_string())
}
```

**Step 2: Register the sub-module**

In `src-tauri/src/commands.rs`, add at the top with the other `mod` declarations:
```rust
mod notes;
```

And add `pub use notes::*;` or make the functions public via the module path. Actually, looking at the existing pattern, the commands are referenced as `commands::function_name` in `lib.rs`. The sub-modules `github` and `media` are used the same way — their functions are `pub` and accessed through re-export. Check how `github` and `media` commands are registered.

Looking at `lib.rs`, commands like `commands::create_github_issue` are referenced directly. The sub-module functions need to be re-exported. Add to `src-tauri/src/commands.rs`:
```rust
mod notes;
```

And ensure the functions in `commands/notes.rs` are `pub`.

In `src-tauri/src/lib.rs`, add to the `invoke_handler` list:
```rust
commands::notes::list_notes,
commands::notes::read_note,
commands::notes::write_note,
commands::notes::create_note,
commands::notes::rename_note,
commands::notes::delete_note,
```

**Step 3: Verify it compiles**

Run: `cd src-tauri && cargo build`
Expected: Compiles without errors

**Step 4: Commit**

```bash
git add src-tauri/src/commands/notes.rs src-tauri/src/commands.rs src-tauri/src/lib.rs
git commit -m "feat(notes): add Tauri commands for notes CRUD"
```

---

### Task 3: Frontend store types

**Files:**
- Modify: `src/lib/stores.ts`

**Step 1: Update WorkspaceMode type**

Change:
```typescript
export type WorkspaceMode = "development" | "agents";
```
To:
```typescript
export type WorkspaceMode = "development" | "agents" | "notes";
```

**Step 2: Add NoteEntry interface and notes stores**

Add after the `Config` interface:
```typescript
export interface NoteEntry {
  filename: string;
  modified_at: string;
}

export const activeNote = writable<{ projectId: string; filename: string } | null>(null);
export const noteEntries = writable<Map<string, NoteEntry[]>>(new Map());
export const notePreviewMode = writable<boolean>(false);
```

**Step 3: Add FocusTarget variants for notes**

Update the `FocusTarget` type to add:
```typescript
export type FocusTarget =
  | { type: "terminal"; projectId: string }
  | { type: "session"; sessionId: string; projectId: string }
  | { type: "project"; projectId: string }
  | { type: "agent"; agentKind: AgentKind; projectId: string }
  | { type: "agent-panel"; agentKind: AgentKind; projectId: string }
  | { type: "note"; filename: string; projectId: string }
  | { type: "notes-editor"; projectId: string }
  | null;
```

**Step 4: Add HotkeyAction variants for notes**

Add to the `HotkeyAction` type:
```typescript
  | { type: "create-note" }
  | { type: "delete-note"; projectId: string; filename: string }
  | { type: "rename-note"; projectId: string; filename: string }
  | { type: "toggle-note-preview" }
```

**Step 5: Commit**

```bash
git add src/lib/stores.ts
git commit -m "feat(notes): add notes types, stores, and focus targets"
```

---

### Task 4: Commands registry for notes mode

**Files:**
- Modify: `src/lib/commands.ts`
- Modify: `src/lib/commands.test.ts`

**Step 1: Update commands.ts**

Add `"Notes"` to `CommandSection`:
```typescript
export type CommandSection = "Navigation" | "Sessions" | "Projects" | "Panels" | "Agents" | "Notes";
```

Add notes command IDs to `CommandId`:
```typescript
export type CommandId =
  // ... existing IDs ...
  | "create-note"
  | "delete-note"
  | "rename-note"
  | "toggle-note-preview";
```

Add notes commands to the `commands` array (at the end, before the closing `]`):
```typescript
  // ── Notes ──
  { id: "create-note", key: "n", section: "Notes", description: "Create new note", mode: "notes" },
  { id: "delete-note", key: "d", section: "Notes", description: "Delete focused note", mode: "notes" },
  { id: "rename-note", key: "r", section: "Notes", description: "Rename focused note", mode: "notes" },
  { id: "toggle-note-preview", key: "p", section: "Notes", description: "Toggle edit/preview", mode: "notes" },
```

Add `"Notes"` to `SECTION_ORDER`:
```typescript
const SECTION_ORDER: CommandSection[] = ["Navigation", "Sessions", "Projects", "Panels", "Agents", "Notes"];
```

**Step 2: Update commands.test.ts**

Update the test that checks key uniqueness to include `"notes"` mode:
```typescript
const modes = ["development", "agents", "notes"] as const;
```

Update the test for `getHelpSections` for development mode — since notes commands are mode-scoped, they won't appear. No change needed for existing tests, but add a new test:

```typescript
it("getHelpSections returns sections for notes mode", () => {
  const sections = getHelpSections("notes");
  expect(sections.map(s => s.label)).toEqual(["Navigation", "Panels", "Notes"]);
});

it("buildKeyMap for notes includes notes commands but not dev or agents commands", () => {
  const map = buildKeyMap("notes");
  expect(map.has("n")).toBe(true); // create-note (notes)
  expect(map.get("n")).toBe("create-note");
  expect(map.has("d")).toBe(true); // delete-note (notes)
  expect(map.get("d")).toBe("delete-note");
  expect(map.has("r")).toBe(true); // rename-note (notes)
  expect(map.has("p")).toBe(true); // toggle-note-preview (notes)
  expect(map.has("c")).toBe(false); // create-session-claude is dev-only
  expect(map.has("o")).toBe(false); // toggle-mode/toggle-agent not in notes
});
```

Update the `getHelpSections without mode` test to include "Notes":
```typescript
expect(sections.map(s => s.label)).toEqual(["Navigation", "Sessions", "Projects", "Panels", "Agents", "Notes"]);
```

Update the `help sections have correct entry counts for development mode` — the counts stay the same since notes commands are mode-scoped.

Add notes-specific help section count test:
```typescript
it("help sections have correct entry counts for notes mode", () => {
  const sections = getHelpSections("notes");
  const notes = sections.find(s => s.label === "Notes")!;
  expect(notes.entries).toHaveLength(4);
});
```

**Step 3: Run tests**

Run: `npx vitest run src/lib/commands.test.ts`
Expected: All tests PASS

**Step 4: Commit**

```bash
git add src/lib/commands.ts src/lib/commands.test.ts
git commit -m "feat(notes): add notes commands to registry"
```

---

### Task 5: Focus helpers for notes mode

**Files:**
- Modify: `src/lib/focus-helpers.ts`
- Modify: `src/lib/focus-helpers.test.ts`

**Step 1: Write failing tests**

Add to `focus-helpers.test.ts`:

```typescript
describe("focusForModeSwitch - notes mode", () => {
  it("translates session focus to project when switching to notes", () => {
    const projects = [makeProject("p1", ["s1"])];
    const result = focusForModeSwitch(
      { type: "session", sessionId: "s1", projectId: "p1" },
      "notes",
      "s1",
      projects,
    );
    expect(result).toEqual({ type: "project", projectId: "p1" });
  });

  it("translates agent focus to project when switching to notes", () => {
    const projects = [makeProject("p1", ["s1"])];
    const result = focusForModeSwitch(
      { type: "agent", agentKind: "auto-worker", projectId: "p1" },
      "notes",
      "s1",
      projects,
    );
    expect(result).toEqual({ type: "project", projectId: "p1" });
  });

  it("translates note focus to active session when switching to development", () => {
    const projects = [makeProject("p1", ["s1"])];
    const result = focusForModeSwitch(
      { type: "note", filename: "test.md", projectId: "p1" },
      "development",
      "s1",
      projects,
    );
    expect(result).toEqual({ type: "session", sessionId: "s1", projectId: "p1" });
  });

  it("translates note focus to project when no active session", () => {
    const projects = [makeProject("p1", ["s1"])];
    const result = focusForModeSwitch(
      { type: "note", filename: "test.md", projectId: "p1" },
      "development",
      null,
      projects,
    );
    expect(result).toEqual({ type: "project", projectId: "p1" });
  });

  it("translates notes-editor focus to project when switching to agents", () => {
    const projects = [makeProject("p1", ["s1"])];
    const result = focusForModeSwitch(
      { type: "notes-editor", projectId: "p1" },
      "agents",
      "s1",
      projects,
    );
    expect(result).toEqual({ type: "project", projectId: "p1" });
  });

  it("preserves project focus when switching to notes", () => {
    const projects = [makeProject("p1", ["s1"])];
    const focus = { type: "project" as const, projectId: "p1" };
    expect(focusForModeSwitch(focus, "notes", "s1", projects)).toBe(focus);
  });
});
```

**Step 2: Run tests to verify they fail**

Run: `npx vitest run src/lib/focus-helpers.test.ts`
Expected: FAIL — new focus target types not handled

**Step 3: Update focus-helpers.ts**

Update `focusForModeSwitch` to handle notes mode:

```typescript
export function focusForModeSwitch(
  current: FocusTarget,
  newMode: WorkspaceMode,
  activeSessionId: string | null,
  projectList: Project[],
): FocusTarget {
  if (!current) return null;

  if (newMode === "development") {
    if (current.type === "agent" || current.type === "agent-panel" || current.type === "note" || current.type === "notes-editor") {
      if (activeSessionId) {
        const project = projectList.find(p => p.id === current.projectId);
        if (project?.sessions.some(s => s.id === activeSessionId && !s.archived)) {
          return { type: "session", sessionId: activeSessionId, projectId: current.projectId };
        }
      }
      return { type: "project", projectId: current.projectId };
    }
  }

  if (newMode === "agents") {
    if (current.type === "session" || current.type === "note" || current.type === "notes-editor") {
      return { type: "project", projectId: current.projectId };
    }
  }

  if (newMode === "notes") {
    if (current.type === "session" || current.type === "agent" || current.type === "agent-panel") {
      return { type: "project", projectId: current.projectId };
    }
  }

  return current;
}
```

**Step 4: Run tests to verify they pass**

Run: `npx vitest run src/lib/focus-helpers.test.ts`
Expected: All tests PASS

**Step 5: Commit**

```bash
git add src/lib/focus-helpers.ts src/lib/focus-helpers.test.ts
git commit -m "feat(notes): handle notes focus targets in mode switch"
```

---

### Task 6: Workspace switcher — add notes option

**Files:**
- Modify: `src/lib/WorkspaceModePicker.svelte`
- Modify: `src/lib/HotkeyManager.svelte`

**Step 1: Add notes to WorkspaceModePicker**

In `WorkspaceModePicker.svelte`, add to the `modes` array:
```typescript
const modes: { key: string; id: WorkspaceMode; label: string }[] = [
  { key: "d", id: "development", label: "Development" },
  { key: "a", id: "agents", label: "Agents" },
  { key: "n", id: "notes", label: "Notes" },
];
```

**Step 2: Handle `n` key in HotkeyManager**

In `HotkeyManager.svelte`, update `handleWorkspaceModeKey`:
```typescript
function handleWorkspaceModeKey(key: string) {
  workspaceModeActive = false;
  workspaceModePickerVisible.set(false);
  if (key === "d") {
    workspaceMode.set("development");
    const newFocus = focusForModeSwitch(currentFocus, "development", activeId, projectList);
    if (newFocus !== currentFocus) focusTarget.set(newFocus);
    return;
  }
  if (key === "a") {
    workspaceMode.set("agents");
    const newFocus = focusForModeSwitch(currentFocus, "agents", activeId, projectList);
    if (newFocus !== currentFocus) focusTarget.set(newFocus);
    return;
  }
  if (key === "n") {
    workspaceMode.set("notes");
    const newFocus = focusForModeSwitch(currentFocus, "notes", activeId, projectList);
    if (newFocus !== currentFocus) focusTarget.set(newFocus);
    return;
  }
  // Any other key (including Escape) cancels
}
```

**Step 3: Commit**

```bash
git add src/lib/WorkspaceModePicker.svelte src/lib/HotkeyManager.svelte
git commit -m "feat(notes): add notes option to workspace switcher"
```

---

### Task 7: NotesTree sidebar component

**Files:**
- Create: `src/lib/sidebar/NotesTree.svelte`

**Step 1: Create the component**

```svelte
<script lang="ts">
  import { fromStore } from "svelte/store";
  import { invoke } from "@tauri-apps/api/core";
  import { noteEntries, type NoteEntry, type Project, type FocusTarget } from "../stores";

  interface Props {
    projects: Project[];
    expandedProjectSet: Set<string>;
    currentFocus: FocusTarget;
    onToggleProject: (projectId: string) => void;
    onProjectFocus: (projectId: string) => void;
    onNoteFocus: (filename: string, projectId: string) => void;
    onNoteSelect: (filename: string, projectId: string) => void;
  }

  let { projects, expandedProjectSet, currentFocus, onToggleProject, onProjectFocus, onNoteFocus, onNoteSelect }: Props = $props();

  const noteEntriesState = fromStore(noteEntries);
  let entriesMap: Map<string, NoteEntry[]> = $derived(noteEntriesState.current);

  // Fetch notes when a project is expanded
  $effect(() => {
    for (const project of projects) {
      if (expandedProjectSet.has(project.id)) {
        fetchNotes(project);
      }
    }
  });

  async function fetchNotes(project: Project) {
    try {
      const notes = await invoke<NoteEntry[]>("list_notes", { projectName: project.name });
      noteEntries.update(m => {
        const next = new Map(m);
        next.set(project.id, notes);
        return next;
      });
    } catch {
      // Silently handle — empty list
    }
  }

  function isProjectFocused(projectId: string): boolean {
    return currentFocus?.type === "project" && currentFocus.projectId === projectId;
  }

  function isNoteFocused(projectId: string, filename: string): boolean {
    return currentFocus?.type === "note" && currentFocus.projectId === projectId && currentFocus.filename === filename;
  }

  function getProjectNotes(projectId: string): NoteEntry[] {
    return entriesMap.get(projectId) ?? [];
  }
</script>

{#each projects as project (project.id)}
  <div class="project-item">
    <!-- svelte-ignore a11y_no_noninteractive_tabindex -->
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <div
      class="project-header"
      class:focus-target={isProjectFocused(project.id)}
      tabindex="0"
      data-project-id={project.id}
      onfocusin={(e: FocusEvent) => {
        if (e.target === e.currentTarget) onProjectFocus(project.id);
      }}
    >
      <button class="btn-expand" onclick={() => onToggleProject(project.id)}>
        {expandedProjectSet.has(project.id) ? "\u25BC" : "\u25B6"}
      </button>
      <span class="project-name">{project.name}</span>
      {#if getProjectNotes(project.id).length > 0}
        <span class="note-count">{getProjectNotes(project.id).length}</span>
      {/if}
    </div>

    {#if expandedProjectSet.has(project.id)}
      <div class="note-list">
        {#each getProjectNotes(project.id) as note (note.filename)}
          <!-- svelte-ignore a11y_no_noninteractive_tabindex -->
          <!-- svelte-ignore a11y_no_static_element_interactions -->
          <div
            class="note-item"
            class:focus-target={isNoteFocused(project.id, note.filename)}
            data-note-id="{project.id}:{note.filename}"
            tabindex="0"
            onfocusin={() => onNoteFocus(note.filename, project.id)}
            ondblclick={() => onNoteSelect(note.filename, project.id)}
          >
            <span class="note-icon">📄</span>
            <span class="note-name">{note.filename.replace(/\.md$/, "")}</span>
          </div>
        {/each}
        {#if getProjectNotes(project.id).length === 0}
          <div class="empty-notes">No notes yet — press <kbd>n</kbd></div>
        {/if}
      </div>
    {/if}
  </div>
{/each}

{#if projects.length === 0}
  <div class="empty">No projects</div>
{/if}

<style>
  .project-item { border-bottom: 1px solid #313244; }
  .project-header { display: flex; align-items: center; padding: 8px 16px; gap: 8px; }
  .project-header:hover { background: #313244; }
  .project-header.focus-target { outline: 2px solid #89b4fa; outline-offset: -2px; border-radius: 4px; }
  .btn-expand { background: none; border: none; color: #6c7086; cursor: pointer; padding: 0; font-size: 10px; width: 16px; text-align: center; box-shadow: none; }
  .project-name { flex: 1; font-size: 13px; font-weight: 500; word-break: break-word; }
  .note-count { font-size: 11px; color: #6c7086; background: #313244; padding: 1px 6px; border-radius: 8px; }
  .note-list { padding: 0; }
  .note-item { display: flex; align-items: center; gap: 8px; padding: 6px 16px 6px 40px; cursor: pointer; font-size: 12px; outline: none; }
  .note-item:hover { background: #313244; }
  .note-item.focus-target { outline: 2px solid #89b4fa; outline-offset: -2px; border-radius: 4px; }
  .note-icon { font-size: 12px; flex-shrink: 0; }
  .note-name { flex: 1; color: #cdd6f4; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .empty-notes { padding: 6px 16px 6px 40px; font-size: 11px; color: #6c7086; }
  .empty-notes kbd { background: #313244; color: #89b4fa; padding: 1px 6px; border-radius: 3px; font-family: monospace; font-size: 11px; }
  .empty { padding: 16px; color: #6c7086; font-size: 13px; text-align: center; }
</style>
```

**Step 2: Commit**

```bash
git add src/lib/sidebar/NotesTree.svelte
git commit -m "feat(notes): add NotesTree sidebar component"
```

---

### Task 8: NotesEditor component

**Files:**
- Create: `src/lib/NotesEditor.svelte`
- Create: `src/lib/markdown.ts` (simple markdown renderer)

**Step 1: Create markdown renderer**

Create `src/lib/markdown.ts`:

```typescript
/**
 * Minimal markdown-to-HTML renderer.
 * Supports: headers, bold, italic, code blocks, inline code, links, unordered lists.
 */
export function renderMarkdown(md: string): string {
  const lines = md.split("\n");
  const html: string[] = [];
  let inCodeBlock = false;
  let inList = false;

  for (const line of lines) {
    // Code blocks
    if (line.startsWith("```")) {
      if (inCodeBlock) {
        html.push("</code></pre>");
        inCodeBlock = false;
      } else {
        if (inList) { html.push("</ul>"); inList = false; }
        html.push("<pre><code>");
        inCodeBlock = true;
      }
      continue;
    }
    if (inCodeBlock) {
      html.push(escapeHtml(line));
      html.push("\n");
      continue;
    }

    // Close list if current line is not a list item
    if (inList && !line.match(/^[-*]\s/)) {
      html.push("</ul>");
      inList = false;
    }

    // Empty line
    if (line.trim() === "") {
      html.push("<br>");
      continue;
    }

    // Headers
    const headerMatch = line.match(/^(#{1,6})\s+(.*)/);
    if (headerMatch) {
      const level = headerMatch[1].length;
      html.push(`<h${level}>${inlineFormat(headerMatch[2])}</h${level}>`);
      continue;
    }

    // Unordered list
    const listMatch = line.match(/^[-*]\s+(.*)/);
    if (listMatch) {
      if (!inList) { html.push("<ul>"); inList = true; }
      html.push(`<li>${inlineFormat(listMatch[1])}</li>`);
      continue;
    }

    // Paragraph
    html.push(`<p>${inlineFormat(line)}</p>`);
  }

  if (inCodeBlock) html.push("</code></pre>");
  if (inList) html.push("</ul>");

  return html.join("\n");
}

function escapeHtml(s: string): string {
  return s.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;");
}

function inlineFormat(s: string): string {
  let result = escapeHtml(s);
  // Inline code
  result = result.replace(/`([^`]+)`/g, "<code>$1</code>");
  // Bold
  result = result.replace(/\*\*([^*]+)\*\*/g, "<strong>$1</strong>");
  // Italic
  result = result.replace(/\*([^*]+)\*/g, "<em>$1</em>");
  // Links
  result = result.replace(/\[([^\]]+)\]\(([^)]+)\)/g, '<a href="$2">$1</a>');
  return result;
}
```

**Step 2: Create NotesEditor component**

Create `src/lib/NotesEditor.svelte`:

```svelte
<script lang="ts">
  import { fromStore } from "svelte/store";
  import { invoke } from "@tauri-apps/api/core";
  import { activeNote, notePreviewMode, noteEntries, focusTarget, projects, hotkeyAction, type FocusTarget, type Project } from "./stores";
  import { renderMarkdown } from "./markdown";
  import { showToast } from "./toast";

  const activeNoteState = fromStore(activeNote);
  let current = $derived(activeNoteState.current);
  const notePreviewModeState = fromStore(notePreviewMode);
  let isPreview = $derived(notePreviewModeState.current);
  const projectsState = fromStore(projects);
  let projectList: Project[] = $derived(projectsState.current);
  const focusTargetState = fromStore(focusTarget);
  let currentFocus: FocusTarget = $derived(focusTargetState.current);
  let isFocused = $derived(currentFocus?.type === "notes-editor");

  let content = $state("");
  let savedContent = $state("");
  let loading = $state(false);
  let saveTimer: ReturnType<typeof setTimeout> | null = null;

  let projectName = $derived(
    current ? projectList.find(p => p.id === current.projectId)?.name ?? null : null
  );

  // Load note content when active note changes
  $effect(() => {
    if (current && projectName) {
      loadNote(projectName, current.filename);
    } else {
      content = "";
      savedContent = "";
    }
  });

  // Handle preview toggle
  $effect(() => {
    const unsub = hotkeyAction.subscribe((action) => {
      if (action?.type === "toggle-note-preview") {
        notePreviewMode.update(v => !v);
      }
    });
    return unsub;
  });

  async function loadNote(pName: string, filename: string) {
    loading = true;
    try {
      const result = await invoke<string>("read_note", { projectName: pName, filename });
      content = result;
      savedContent = result;
    } catch (e) {
      showToast(String(e), "error");
      content = "";
      savedContent = "";
    } finally {
      loading = false;
    }
  }

  function handleInput(e: Event) {
    const textarea = e.target as HTMLTextAreaElement;
    content = textarea.value;
    debounceSave();
  }

  function debounceSave() {
    if (saveTimer) clearTimeout(saveTimer);
    saveTimer = setTimeout(() => saveNote(), 500);
  }

  async function saveNote() {
    if (!current || !projectName || content === savedContent) return;
    try {
      await invoke("write_note", { projectName, filename: current.filename, content });
      savedContent = content;
    } catch (e) {
      showToast(`Failed to save: ${e}`, "error");
    }
  }

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === "Escape") {
      e.preventDefault();
      e.stopPropagation();
      // Save immediately on escape
      if (saveTimer) { clearTimeout(saveTimer); saveTimer = null; }
      saveNote();
      // Return focus to the note in sidebar
      if (current) {
        focusTarget.set({ type: "note", filename: current.filename, projectId: current.projectId });
      }
    }
    // Allow Tab to insert spaces
    if (e.key === "Tab") {
      e.preventDefault();
      const textarea = e.target as HTMLTextAreaElement;
      const start = textarea.selectionStart;
      const end = textarea.selectionEnd;
      content = content.substring(0, start) + "  " + content.substring(end);
      // Set cursor position after the inserted spaces
      requestAnimationFrame(() => {
        textarea.selectionStart = textarea.selectionEnd = start + 2;
      });
    }
  }

  let dirty = $derived(content !== savedContent);
  let noteTitle = $derived(current?.filename.replace(/\.md$/, "") ?? "");

  let editorEl: HTMLTextAreaElement | undefined = $state();

  // Auto-focus editor when focus target is notes-editor
  $effect(() => {
    if (isFocused && editorEl && !isPreview) {
      editorEl.focus();
    }
  });
</script>

<div class="editor-container">
  {#if !current}
    <div class="empty-state">
      <div class="empty-title">No note selected</div>
      <div class="empty-hint">Select a note from the sidebar, or press <kbd>n</kbd> to create one</div>
    </div>
  {:else if loading}
    <div class="empty-state">
      <div class="empty-title">Loading...</div>
    </div>
  {:else}
    <div class="editor-header">
      <h2 class="editor-title">{noteTitle}</h2>
      <div class="header-actions">
        {#if dirty}
          <span class="dirty-indicator">unsaved</span>
        {/if}
        <button
          class="preview-toggle"
          class:active={isPreview}
          onclick={() => notePreviewMode.update(v => !v)}
        >
          {isPreview ? "Edit" : "Preview"}
        </button>
      </div>
    </div>
    <div class="editor-body" class:focused={isFocused}>
      {#if isPreview}
        <div class="preview-content">
          {@html renderMarkdown(content)}
        </div>
      {:else}
        <textarea
          bind:this={editorEl}
          class="editor-textarea"
          value={content}
          oninput={handleInput}
          onkeydown={handleKeydown}
          onfocus={() => {
            if (current) focusTarget.set({ type: "notes-editor", projectId: current.projectId });
          }}
          spellcheck="false"
        ></textarea>
      {/if}
    </div>
  {/if}
</div>

<style>
  .editor-container { width: 100%; height: 100%; display: flex; flex-direction: column; background: #11111b; color: #cdd6f4; }
  .empty-state { display: flex; flex-direction: column; align-items: center; justify-content: center; height: 100%; gap: 8px; }
  .empty-title { font-size: 16px; font-weight: 500; }
  .empty-hint { color: #6c7086; font-size: 13px; }
  .empty-hint kbd { background: #313244; color: #89b4fa; padding: 1px 6px; border-radius: 3px; font-family: monospace; font-size: 12px; }
  .editor-header { display: flex; align-items: center; padding: 12px 24px; border-bottom: 1px solid #313244; }
  .editor-title { font-size: 16px; font-weight: 600; margin: 0; flex: 1; }
  .header-actions { display: flex; align-items: center; gap: 8px; }
  .dirty-indicator { font-size: 11px; color: #f9e2af; font-style: italic; }
  .preview-toggle { background: #313244; border: none; color: #cdd6f4; padding: 4px 12px; border-radius: 4px; font-size: 12px; cursor: pointer; box-shadow: none; }
  .preview-toggle:hover { background: #45475a; }
  .preview-toggle.active { background: #89b4fa; color: #1e1e2e; }
  .editor-body { flex: 1; overflow: hidden; }
  .editor-body.focused { outline: 2px solid #89b4fa; outline-offset: -2px; border-radius: 4px; }
  .editor-textarea {
    width: 100%; height: 100%;
    background: #11111b; color: #cdd6f4;
    border: none; outline: none;
    padding: 16px 24px;
    font-family: monospace; font-size: 14px; line-height: 1.6;
    resize: none;
    box-sizing: border-box;
  }
  .preview-content {
    padding: 16px 24px;
    overflow-y: auto;
    height: 100%;
    font-size: 14px;
    line-height: 1.6;
  }
  .preview-content :global(h1) { font-size: 24px; font-weight: 600; margin: 0 0 12px; border-bottom: 1px solid #313244; padding-bottom: 8px; }
  .preview-content :global(h2) { font-size: 20px; font-weight: 600; margin: 16px 0 8px; }
  .preview-content :global(h3) { font-size: 16px; font-weight: 600; margin: 12px 0 6px; }
  .preview-content :global(p) { margin: 0 0 8px; }
  .preview-content :global(pre) { background: #1e1e2e; padding: 12px; border-radius: 6px; overflow-x: auto; margin: 8px 0; }
  .preview-content :global(code) { font-family: monospace; font-size: 13px; }
  .preview-content :global(p code) { background: #1e1e2e; padding: 2px 6px; border-radius: 3px; }
  .preview-content :global(strong) { font-weight: 600; }
  .preview-content :global(em) { font-style: italic; }
  .preview-content :global(a) { color: #89b4fa; text-decoration: none; }
  .preview-content :global(a:hover) { text-decoration: underline; }
  .preview-content :global(ul) { margin: 8px 0; padding-left: 24px; }
  .preview-content :global(li) { margin: 4px 0; }
  .preview-content :global(br) { display: block; margin: 4px 0; content: ""; }
</style>
```

**Step 3: Commit**

```bash
git add src/lib/markdown.ts src/lib/NotesEditor.svelte
git commit -m "feat(notes): add NotesEditor component and markdown renderer"
```

---

### Task 9: Sidebar integration — add NotesTree

**Files:**
- Modify: `src/lib/Sidebar.svelte`

**Step 1: Import NotesTree and add to template**

Import at the top:
```typescript
import NotesTree from "./sidebar/NotesTree.svelte";
```

Import notes stores:
```typescript
import { ..., activeNote, noteEntries } from "./stores";
```

**Step 2: Add notes tree to the sidebar template**

In the `<div class="project-list">` section, change the conditional to include notes mode:
```svelte
{#if currentMode === "agents"}
  <AgentTree ... />
{:else if currentMode === "notes"}
  <NotesTree
    projects={projectList}
    {expandedProjectSet}
    {currentFocus}
    onToggleProject={toggleProject}
    onProjectFocus={(projectId) => {
      focusTarget.set({ type: "project", projectId });
    }}
    onNoteFocus={(filename, projectId) => {
      focusTarget.set({ type: "note", filename, projectId });
    }}
    onNoteSelect={(filename, projectId) => {
      activeNote.set({ projectId, filename });
      focusTarget.set({ type: "notes-editor", projectId });
    }}
  />
{:else}
  <ProjectTree ... />
{/if}
```

**Step 3: Update sidebar header**

Change the header text logic:
```svelte
<h2>{isArchiveView ? "Archives" : currentMode === "agents" ? "Agents" : currentMode === "notes" ? "Notes" : "Development"}</h2>
```

**Step 4: Update sidebar footer**

Notes mode doesn't have Active/Archives tabs:
```svelte
{#if currentMode !== "agents" && currentMode !== "notes"}
  <button class="footer-tab" ...>Active</button>
  <button class="footer-tab" ...>Archives</button>
{:else}
  <div class="footer-spacer"></div>
{/if}
```

**Step 5: Add notes-specific hotkey actions**

In the `$effect` that subscribes to `hotkeyAction`, add handlers for notes actions:
```typescript
case "create-note": {
  const project = currentFocus?.type === "project" || currentFocus?.type === "note"
    ? projectList.find(p => p.id === currentFocus.projectId)
    : projectList[0];
  if (project) createNoteForProject(project);
  break;
}
case "delete-note": {
  deleteNoteTarget = { projectId: action.projectId, filename: action.filename };
  break;
}
case "rename-note": {
  renameNoteTarget = { projectId: action.projectId, filename: action.filename };
  break;
}
```

Add state variables:
```typescript
let deleteNoteTarget: { projectId: string; filename: string } | null = $state(null);
let renameNoteTarget: { projectId: string; filename: string } | null = $state(null);
let showNewNoteModal = $state(false);
let newNoteProjectId = $state("");
```

Add helper functions:
```typescript
function createNoteForProject(project: Project) {
  newNoteProjectId = project.id;
  showNewNoteModal = true;
}

async function handleCreateNote(title: string) {
  const project = projectList.find(p => p.id === newNoteProjectId);
  if (!project) return;
  showNewNoteModal = false;
  try {
    const entry = await invoke<NoteEntry>("create_note", { projectName: project.name, title });
    // Refresh notes list
    const notes = await invoke<NoteEntry[]>("list_notes", { projectName: project.name });
    noteEntries.update(m => { const next = new Map(m); next.set(project.id, notes); return next; });
    // Expand project and focus the new note
    expandedProjects.update(s => { const next = new Set(s); next.add(project.id); return next; });
    activeNote.set({ projectId: project.id, filename: entry.filename });
    focusTarget.set({ type: "notes-editor", projectId: project.id });
  } catch (e) {
    showToast(String(e), "error");
  }
}

async function handleDeleteNote(projectId: string, filename: string) {
  const project = projectList.find(p => p.id === projectId);
  if (!project) return;
  try {
    await invoke("delete_note", { projectName: project.name, filename });
    // Refresh notes list
    const notes = await invoke<NoteEntry[]>("list_notes", { projectName: project.name });
    noteEntries.update(m => { const next = new Map(m); next.set(project.id, notes); return next; });
    // Clear active note if it was deleted
    const an = activeNoteState.current;
    if (an?.projectId === projectId && an?.filename === filename) {
      activeNote.set(null);
    }
    focusTarget.set({ type: "project", projectId });
    showToast("Note deleted", "info");
  } catch (e) {
    showToast(String(e), "error");
  }
}

async function handleRenameNote(projectId: string, oldName: string, newName: string) {
  const project = projectList.find(p => p.id === projectId);
  if (!project) return;
  try {
    await invoke("rename_note", { projectName: project.name, oldName, newName });
    // Refresh notes list
    const notes = await invoke<NoteEntry[]>("list_notes", { projectName: project.name });
    noteEntries.update(m => { const next = new Map(m); next.set(project.id, notes); return next; });
    // Update active note if renamed
    const an = activeNoteState.current;
    const newFilename = newName.endsWith(".md") ? newName : `${newName}.md`;
    if (an?.projectId === projectId && an?.filename === oldName) {
      activeNote.set({ projectId, filename: newFilename });
    }
    focusTarget.set({ type: "note", filename: newFilename, projectId });
    showToast("Note renamed", "info");
  } catch (e) {
    showToast(String(e), "error");
  }
}
```

Add modals for create/delete/rename at the bottom of the sidebar template (inside `<aside>`):

For create note, use a simple modal (or reuse ConfirmModal pattern). Simplest approach — a small inline modal:

```svelte
{#if showNewNoteModal}
  <NewNoteModal
    onSubmit={handleCreateNote}
    onClose={() => { showNewNoteModal = false; }}
  />
{/if}

{#if deleteNoteTarget}
  <ConfirmModal
    title="Delete Note"
    message={`Delete "${deleteNoteTarget.filename}"?`}
    confirmLabel="Delete"
    onConfirm={() => {
      if (deleteNoteTarget) handleDeleteNote(deleteNoteTarget.projectId, deleteNoteTarget.filename);
      deleteNoteTarget = null;
    }}
    onClose={() => (deleteNoteTarget = null)}
  />
{/if}

{#if renameNoteTarget}
  <RenameNoteModal
    currentName={renameNoteTarget.filename}
    onSubmit={(newName) => {
      if (renameNoteTarget) handleRenameNote(renameNoteTarget.projectId, renameNoteTarget.filename, newName);
      renameNoteTarget = null;
    }}
    onClose={() => { renameNoteTarget = null; }}
  />
{/if}
```

**Step 6: Handle focus for note items in sidebar**

In the `$effect` that watches `currentFocus`, add handling for note items:
```typescript
} else if (currentFocus?.type === "note") {
  if (!expandedProjectSet.has(currentFocus.projectId)) {
    const next = new Set(expandedProjectSet);
    next.add(currentFocus.projectId);
    expandedProjects.set(next);
  }
  if (sidebarEl) {
    requestAnimationFrame(() => {
      const el = sidebarEl?.querySelector<HTMLElement>(`[data-note-id="${currentFocus.projectId}:${currentFocus.filename}"]`);
      if (el) el.focus();
    });
  }
}
```

**Step 7: Commit**

```bash
git add src/lib/Sidebar.svelte
git commit -m "feat(notes): integrate NotesTree in sidebar with CRUD actions"
```

---

### Task 10: NewNoteModal and RenameNoteModal

**Files:**
- Create: `src/lib/NewNoteModal.svelte`
- Create: `src/lib/RenameNoteModal.svelte`

**Step 1: Create NewNoteModal**

```svelte
<script lang="ts">
  interface Props {
    onSubmit: (title: string) => void;
    onClose: () => void;
  }

  let { onSubmit, onClose }: Props = $props();
  let title = $state("");
  let inputEl: HTMLInputElement | undefined = $state();

  $effect(() => { inputEl?.focus(); });

  function handleSubmit() {
    const trimmed = title.trim();
    if (trimmed) onSubmit(trimmed);
  }

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === "Enter") { e.preventDefault(); handleSubmit(); }
    if (e.key === "Escape") { e.preventDefault(); onClose(); }
  }
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<!-- svelte-ignore a11y_click_events_have_key_events -->
<div class="overlay" onclick={onClose} role="dialog">
  <!-- svelte-ignore a11y_click_events_have_key_events -->
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div class="modal" onclick={(e) => e.stopPropagation()}>
    <h3>New Note</h3>
    <input
      bind:this={inputEl}
      type="text"
      placeholder="Note title"
      bind:value={title}
      onkeydown={handleKeydown}
    />
    <div class="actions">
      <button class="btn" onclick={onClose}>Cancel</button>
      <button class="btn btn-primary" onclick={handleSubmit} disabled={!title.trim()}>Create</button>
    </div>
  </div>
</div>

<style>
  .overlay { position: fixed; inset: 0; background: rgba(0,0,0,0.5); display: flex; align-items: center; justify-content: center; z-index: 100; }
  .modal { background: #1e1e2e; border: 1px solid #313244; border-radius: 8px; padding: 20px 24px; min-width: 300px; }
  h3 { margin: 0 0 16px; font-size: 14px; font-weight: 600; color: #cdd6f4; }
  input { width: 100%; background: #11111b; border: 1px solid #313244; border-radius: 4px; padding: 8px 12px; color: #cdd6f4; font-size: 13px; box-sizing: border-box; outline: none; }
  input:focus { border-color: #89b4fa; }
  .actions { display: flex; justify-content: flex-end; gap: 8px; margin-top: 16px; }
  .btn { background: #313244; border: none; color: #cdd6f4; padding: 6px 16px; border-radius: 4px; font-size: 12px; cursor: pointer; box-shadow: none; }
  .btn:hover { background: #45475a; }
  .btn-primary { background: #89b4fa; color: #1e1e2e; }
  .btn-primary:hover { background: #74c7ec; }
  .btn-primary:disabled { opacity: 0.5; cursor: not-allowed; }
</style>
```

**Step 2: Create RenameNoteModal**

```svelte
<script lang="ts">
  interface Props {
    currentName: string;
    onSubmit: (newName: string) => void;
    onClose: () => void;
  }

  let { currentName, onSubmit, onClose }: Props = $props();
  let name = $state(currentName.replace(/\.md$/, ""));
  let inputEl: HTMLInputElement | undefined = $state();

  $effect(() => {
    if (inputEl) { inputEl.focus(); inputEl.select(); }
  });

  function handleSubmit() {
    const trimmed = name.trim();
    if (trimmed && trimmed !== currentName.replace(/\.md$/, "")) onSubmit(trimmed);
  }

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === "Enter") { e.preventDefault(); handleSubmit(); }
    if (e.key === "Escape") { e.preventDefault(); onClose(); }
  }
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<!-- svelte-ignore a11y_click_events_have_key_events -->
<div class="overlay" onclick={onClose} role="dialog">
  <!-- svelte-ignore a11y_click_events_have_key_events -->
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div class="modal" onclick={(e) => e.stopPropagation()}>
    <h3>Rename Note</h3>
    <input
      bind:this={inputEl}
      type="text"
      placeholder="New name"
      bind:value={name}
      onkeydown={handleKeydown}
    />
    <div class="actions">
      <button class="btn" onclick={onClose}>Cancel</button>
      <button class="btn btn-primary" onclick={handleSubmit} disabled={!name.trim()}>Rename</button>
    </div>
  </div>
</div>

<style>
  .overlay { position: fixed; inset: 0; background: rgba(0,0,0,0.5); display: flex; align-items: center; justify-content: center; z-index: 100; }
  .modal { background: #1e1e2e; border: 1px solid #313244; border-radius: 8px; padding: 20px 24px; min-width: 300px; }
  h3 { margin: 0 0 16px; font-size: 14px; font-weight: 600; color: #cdd6f4; }
  input { width: 100%; background: #11111b; border: 1px solid #313244; border-radius: 4px; padding: 8px 12px; color: #cdd6f4; font-size: 13px; box-sizing: border-box; outline: none; }
  input:focus { border-color: #89b4fa; }
  .actions { display: flex; justify-content: flex-end; gap: 8px; margin-top: 16px; }
  .btn { background: #313244; border: none; color: #cdd6f4; padding: 6px 16px; border-radius: 4px; font-size: 12px; cursor: pointer; box-shadow: none; }
  .btn:hover { background: #45475a; }
  .btn-primary { background: #89b4fa; color: #1e1e2e; }
  .btn-primary:hover { background: #74c7ec; }
  .btn-primary:disabled { opacity: 0.5; cursor: not-allowed; }
</style>
```

**Step 3: Commit**

```bash
git add src/lib/NewNoteModal.svelte src/lib/RenameNoteModal.svelte
git commit -m "feat(notes): add NewNoteModal and RenameNoteModal components"
```

---

### Task 11: App.svelte integration

**Files:**
- Modify: `src/App.svelte`

**Step 1: Import NotesEditor**

```typescript
import NotesEditor from "./lib/NotesEditor.svelte";
```

**Step 2: Add notes workspace rendering**

Update the main content area conditional:
```svelte
<main class="terminal-area">
  {#if workspaceModeState.current === "agents"}
    <AgentDashboard />
  {:else if workspaceModeState.current === "notes"}
    <NotesEditor />
  {:else}
    <TerminalManager />
  {/if}
</main>
```

**Step 3: Commit**

```bash
git add src/App.svelte
git commit -m "feat(notes): render NotesEditor in App.svelte"
```

---

### Task 12: HotkeyManager — notes mode navigation and commands

**Files:**
- Modify: `src/lib/HotkeyManager.svelte`

**Step 1: Import noteEntries store**

```typescript
import { ..., noteEntries, activeNote } from "./stores";
```

Add derived state:
```typescript
const noteEntriesState = fromStore(noteEntries);
let noteEntriesMap = $derived(noteEntriesState.current);
```

**Step 2: Update `getVisibleItems()` for notes mode**

Add notes mode handling:
```typescript
function getVisibleItems(): SidebarItem[] {
  if (currentMode === "agents") {
    // ... existing agents code ...
  }
  if (currentMode === "notes") {
    const result: SidebarItem[] = [];
    for (const p of projectList) {
      result.push({ type: "project", projectId: p.id });
      if (!expandedSet.has(p.id)) continue;
      const notes = noteEntriesMap.get(p.id) ?? [];
      for (const n of notes) {
        result.push({ type: "note", filename: n.filename, projectId: p.id });
      }
    }
    return result;
  }
  // ... existing development code ...
}
```

**Step 3: Update `SidebarItem` type**

Add to the SidebarItem union:
```typescript
type SidebarItem =
  | { type: "project"; projectId: string }
  | { type: "session"; sessionId: string; projectId: string }
  | { type: "agent"; agentKind: "auto-worker" | "maintainer"; projectId: string }
  | { type: "note"; filename: string; projectId: string };
```

**Step 4: Update `navigateItem()` to handle notes**

In the `navigateItem` function, add note handling alongside session:
```typescript
// Add to the current focus index finding:
} else if (currentFocus?.type === "note") {
  idx = items.findIndex(it => it.type === "note" && it.projectId === currentFocus.projectId && it.filename === currentFocus.filename);
}

// Add to the navigation target setting:
} else if (next.type === "note") {
  focusTarget.set({ type: "note", filename: next.filename, projectId: next.projectId });
}
```

**Step 5: Update `handleHotkey()` with notes commands**

Add new cases to the switch:
```typescript
case "create-note":
  dispatchAction({ type: "create-note" });
  return true;
case "delete-note":
  if (currentFocus?.type === "note") {
    dispatchAction({ type: "delete-note", projectId: currentFocus.projectId, filename: currentFocus.filename });
  }
  return true;
case "rename-note":
  if (currentFocus?.type === "note") {
    dispatchAction({ type: "rename-note", projectId: currentFocus.projectId, filename: currentFocus.filename });
  }
  return true;
case "toggle-note-preview":
  dispatchAction({ type: "toggle-note-preview" });
  return true;
```

**Step 6: Update expand-collapse for notes**

In the `"expand-collapse"` case, add handling for note items:
```typescript
} else if (currentFocus?.type === "note") {
  activeNote.set({ projectId: currentFocus.projectId, filename: currentFocus.filename });
  focusTarget.set({ type: "notes-editor", projectId: currentFocus.projectId });
}
```

**Step 7: Update Escape handling for notes-editor**

In the ambient-mode Escape handling section, add:
```typescript
} else if (currentFocus?.type === "notes-editor") {
  const an = activeNote.get?.() ?? null;
  // Use store.subscribe to get current value synchronously
  let currentNote: { projectId: string; filename: string } | null = null;
  const unsub = activeNote.subscribe(v => { currentNote = v; });
  unsub();
  if (currentNote) {
    focusTarget.set({ type: "note", filename: currentNote.filename, projectId: currentNote.projectId });
  } else if (currentFocus.projectId) {
    focusTarget.set({ type: "project", projectId: currentFocus.projectId });
  }
  e.stopPropagation();
  e.preventDefault();
  pushKeystroke("Esc");
} else if (currentFocus?.type === "note") {
  focusTarget.set({ type: "project", projectId: currentFocus.projectId });
  e.stopPropagation();
  e.preventDefault();
  pushKeystroke("Esc");
}
```

**Step 8: Update `navigateItem` and `navigateProject` functions**

In `navigateProject`, add `currentFocus?.type === "note"` and `currentFocus?.type === "notes-editor"` to the project-finding condition:
```typescript
const focusedProjectId = currentFocus?.type === "project" || currentFocus?.type === "session" || currentFocus?.type === "agent" || currentFocus?.type === "agent-panel" || currentFocus?.type === "note" || currentFocus?.type === "notes-editor"
  ? currentFocus.projectId
  : null;
```

In `getFocusedProject`, add note types:
```typescript
function getFocusedProject(): Project | null {
  if (currentFocus?.type === "project" || currentFocus?.type === "session" || currentFocus?.type === "agent" || currentFocus?.type === "agent-panel" || currentFocus?.type === "note" || currentFocus?.type === "notes-editor") {
    return projectList.find((p) => p.id === currentFocus.projectId) ?? null;
  }
  return null;
}
```

**Step 9: Commit**

```bash
git add src/lib/HotkeyManager.svelte
git commit -m "feat(notes): wire notes navigation and commands in HotkeyManager"
```

---

### Task 13: Final verification

**Step 1: Run Rust tests**

Run: `cd src-tauri && cargo test`
Expected: All tests PASS

**Step 2: Run frontend tests**

Run: `npx vitest run`
Expected: All tests PASS

**Step 3: Type check**

Run: `npx svelte-check`
Expected: No errors

**Step 4: Build check**

Run: `cd src-tauri && cargo build`
Expected: Compiles without errors

**Step 5: Commit any fixups**

If any fixes were needed, commit them:
```bash
git commit -m "fix(notes): resolve type errors and test failures"
```

**Step 6: Final commit message**

Once everything passes:
```bash
git add -A
git commit -m "feat: add notes editor workspace (closes #227)"
```
