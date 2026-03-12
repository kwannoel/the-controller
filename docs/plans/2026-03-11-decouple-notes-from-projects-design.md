# Decouple Notes from Projects

## Problem

Notes are currently scoped to projects — stored under `~/.the-controller/notes/{project_name}/`, accessed via project context in the sidebar, and threaded through stores/hotkeys by `projectId`. This coupling is arbitrary. Notes should be independent entities organized by user-defined folders.

## Approach

Minimal refactor: rename "project" to "folder" throughout the notes system. The filesystem structure already works — no migration needed. Folder names replace project names as the organizational unit.

## Design

### 1. Storage & Backend

No filesystem migration. Existing structure `~/.the-controller/notes/{folder_name}/` stays as-is.

**Rename `project_name` → `folder` everywhere** in `notes.rs` and `commands/notes.rs`. Pure rename, no logic change.

**New commands:**

- `list_folders()` — scans `~/.the-controller/notes/` and returns folder names (subdirectories). Replaces using the project list to discover note groupings.
- `create_folder(name)` — creates `~/.the-controller/notes/{name}/`
- `rename_folder(old_name, new_name)` — renames the directory
- `delete_folder(name)` — removes the directory (only if empty, or with a `force` flag)

### 2. Frontend Stores & Data Model

**Store changes:**

- `activeNote`: `{ projectId, filename }` → `{ folder, filename }`
- `noteEntries`: `Map<string, NoteEntry[]>` keyed by folder name instead of projectId
- New store: `noteFolders` — `writable<string[]>([])`, populated by `list_folders()`
- `expandedProjects` reused with folder names as keys in notes mode

**FocusTarget changes:**

- `{ type: "note"; filename; projectId }` → `{ type: "note"; filename; folder }`
- `{ type: "notes-editor"; projectId; entryKey? }` → `{ type: "notes-editor"; folder; entryKey? }`

**HotkeyAction changes:**

- `delete-note`, `rename-note`, `duplicate-note`: `projectId` → `folder`
- `create-note`: no longer infers project, opens modal with folder picker

### 3. Sidebar — NotesTree

Complete rewrite of `NotesTree.svelte`:

- Receives `folders: string[]` from `noteFolders` store instead of `projects`
- Each folder is an expandable row: folder name + note count
- Expanding a folder calls `list_notes({ folder })` and populates `noteEntries`
- Context menu on folders for rename/delete
- Empty state: "No folders — press `n` to create a note"

**Props change from:**
```
projects, expandedProjectSet, currentFocus, onToggleProject, onProjectFocus, onNoteFocus, onNoteSelect
```

**To:**
```
folders, expandedFolderSet, currentFocus, onToggleFolder, onFolderFocus, onNoteFocus, onNoteSelect
```

**NewNoteModal changes:**

- Adds folder selector: dropdown of existing folders + text input for new folder name
- `onSubmit` signature: `(title: string)` → `(title: string, folder: string)`
- Both title and folder required

### 4. NotesEditor & AI Chat

Minimal changes:

- `projectName` derivation → just use `currentNote.folder` directly (simpler)
- All `command()` calls swap `projectName` → `folder` parameter
- `prevNoteKey`: `${projectId}:${filename}` → `${folder}:${filename}`
- NoteAiPanel — no changes (doesn't touch backend directly)

### 5. HotkeyManager

**Navigation:**

- `flatItems` in notes mode iterates folders from `noteFolders` and notes from `noteEntries` keyed by folder
- Item type: `{ type: "note"; filename; folder }` (was `projectId`)
- Folder focus items replace project focus items

**Action dispatch:**

- `create-note` — opens modal with folder picker
- `delete-note`, `rename-note`, `duplicate-note` — pass `folder` instead of `projectId`
- `r`/`d` keys on focused folder row trigger rename/delete folder

## Constraints

- Flat folders only (one level deep) — nesting can be added later
- Every note must belong to a folder
- No migration needed — existing `~/.the-controller/notes/{name}/` directories become folders
- Folder management (rename/delete) from sidebar context menu + keyboard shortcuts
