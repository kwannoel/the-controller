use std::path::Path;

use tauri::State;

use crate::notes::{self, NoteEntry};
use crate::state::AppState;

/// Resolve the notes base directory for a project.
/// Notes live at `{repo_path}/` — the notes module adds the `notes/` prefix internally.
pub(crate) fn resolve_notes_base(
    storage: &std::sync::Arc<std::sync::Mutex<crate::storage::Storage>>,
    project_id: &str,
) -> Result<std::path::PathBuf, String> {
    let id = uuid::Uuid::parse_str(project_id).map_err(|e| e.to_string())?;
    let storage = storage.lock().map_err(|e| e.to_string())?;
    let project = storage.load_project(id).map_err(|e| e.to_string())?;
    Ok(std::path::PathBuf::from(&project.repo_path))
}

/// Best-effort git commit. Logs errors but doesn't fail the operation.
fn try_commit(base_dir: &std::path::Path, message: &str) {
    tracing::debug!("committing notes");
    if let Err(e) = notes::commit_notes(base_dir, message) {
        tracing::error!(error = %e, "notes git commit failed");
    }
}

pub(crate) async fn list_notes(
    state: State<'_, AppState>,
    project_id: String,
    folder: String,
) -> Result<Vec<NoteEntry>, String> {
    tracing::debug!("listing notes");
    let storage = state.storage.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let base_dir = resolve_notes_base(&storage, &project_id)?;
        notes::list_notes(&base_dir, &folder).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

pub(crate) async fn read_note(
    state: State<'_, AppState>,
    project_id: String,
    folder: String,
    filename: String,
) -> Result<String, String> {
    tracing::debug!("reading note");
    let storage = state.storage.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let base_dir = resolve_notes_base(&storage, &project_id)?;
        notes::read_note(&base_dir, &folder, &filename).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

pub(crate) async fn write_note(
    state: State<'_, AppState>,
    project_id: String,
    folder: String,
    filename: String,
    content: String,
) -> Result<(), String> {
    tracing::debug!("writing note");
    let storage = state.storage.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let base_dir = resolve_notes_base(&storage, &project_id)?;
        notes::write_note(&base_dir, &folder, &filename, &content).map_err(|e| e.to_string())
        // No git commit here — batched via commit_notes command
    })
    .await
    .map_err(|e| e.to_string())?
}

pub(crate) async fn create_note(
    state: State<'_, AppState>,
    project_id: String,
    folder: String,
    title: String,
) -> Result<String, String> {
    tracing::debug!("creating note");
    let storage = state.storage.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let base_dir = resolve_notes_base(&storage, &project_id)?;
        let filename = notes::create_note(&base_dir, &folder, &title).map_err(|e| e.to_string())?;
        try_commit(&base_dir, &format!("create {}/{}", folder, filename));
        Ok(filename)
    })
    .await
    .map_err(|e| e.to_string())?
}

pub(crate) async fn rename_note(
    state: State<'_, AppState>,
    project_id: String,
    folder: String,
    old_name: String,
    new_name: String,
) -> Result<String, String> {
    tracing::debug!("renaming note");
    let storage = state.storage.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let base_dir = resolve_notes_base(&storage, &project_id)?;
        let new_filename = notes::rename_note(&base_dir, &folder, &old_name, &new_name)
            .map_err(|e| e.to_string())?;
        try_commit(
            &base_dir,
            &format!("rename {}/{} → {}", folder, old_name, new_filename),
        );
        Ok(new_filename)
    })
    .await
    .map_err(|e| e.to_string())?
}

pub(crate) async fn duplicate_note(
    state: State<'_, AppState>,
    project_id: String,
    folder: String,
    filename: String,
) -> Result<String, String> {
    tracing::debug!("duplicating note");
    let storage = state.storage.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let base_dir = resolve_notes_base(&storage, &project_id)?;
        let copy =
            notes::duplicate_note(&base_dir, &folder, &filename).map_err(|e| e.to_string())?;
        try_commit(
            &base_dir,
            &format!("duplicate {}/{} → {}", folder, filename, copy),
        );
        Ok(copy)
    })
    .await
    .map_err(|e| e.to_string())?
}

pub(crate) async fn delete_note(
    state: State<'_, AppState>,
    project_id: String,
    folder: String,
    filename: String,
) -> Result<(), String> {
    tracing::debug!("deleting note");
    let storage = state.storage.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let base_dir = resolve_notes_base(&storage, &project_id)?;
        notes::delete_note(&base_dir, &folder, &filename).map_err(|e| e.to_string())?;
        try_commit(&base_dir, &format!("delete {}/{}", folder, filename));
        Ok(())
    })
    .await
    .map_err(|e| e.to_string())?
}

pub(crate) async fn list_folders(
    state: State<'_, AppState>,
    project_id: String,
) -> Result<Vec<String>, String> {
    tracing::debug!("listing note folders");
    let storage = state.storage.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let base_dir = resolve_notes_base(&storage, &project_id)?;
        notes::list_folders(&base_dir).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

pub(crate) async fn create_folder(
    state: State<'_, AppState>,
    project_id: String,
    name: String,
) -> Result<(), String> {
    tracing::debug!("creating folder");
    let storage = state.storage.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let base_dir = resolve_notes_base(&storage, &project_id)?;
        notes::create_folder(&base_dir, &name).map_err(|e| e.to_string())?;
        try_commit(&base_dir, &format!("create folder {}", name));
        Ok(())
    })
    .await
    .map_err(|e| e.to_string())?
}

pub(crate) async fn rename_folder(
    state: State<'_, AppState>,
    project_id: String,
    old_name: String,
    new_name: String,
) -> Result<(), String> {
    tracing::debug!("renaming folder");
    let storage = state.storage.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let base_dir = resolve_notes_base(&storage, &project_id)?;
        notes::rename_folder(&base_dir, &old_name, &new_name).map_err(|e| e.to_string())?;
        try_commit(
            &base_dir,
            &format!("rename folder {} → {}", old_name, new_name),
        );
        Ok(())
    })
    .await
    .map_err(|e| e.to_string())?
}

pub(crate) async fn delete_folder(
    state: State<'_, AppState>,
    project_id: String,
    name: String,
    force: bool,
) -> Result<(), String> {
    tracing::debug!(force, "deleting folder");
    let storage = state.storage.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let base_dir = resolve_notes_base(&storage, &project_id)?;
        notes::delete_folder(&base_dir, &name, force).map_err(|e| e.to_string())?;
        try_commit(&base_dir, &format!("delete folder {}", name));
        Ok(())
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Commit any pending note changes (content edits).
/// Called by the frontend when switching notes.
pub(crate) async fn commit_notes(
    state: State<'_, AppState>,
    project_id: String,
) -> Result<bool, String> {
    tracing::debug!("committing pending note changes");
    let storage = state.storage.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let base_dir = resolve_notes_base(&storage, &project_id)?;
        notes::commit_notes(&base_dir, "update notes").map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Recursively copy a directory tree from `src` to `dst`.
pub(crate) fn copy_dir_recursive(src: &Path, dst: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dst)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let dest_path = dst.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_dir_recursive(&entry.path(), &dest_path)?;
        } else {
            std::fs::copy(entry.path(), dest_path)?;
        }
    }
    Ok(())
}

/// Migrate global notes from `~/.the-controller/notes/` into a project's `notes/` directory.
/// Copies folders recursively, skipping folders that already exist.
/// Returns the number of folders migrated.
pub(crate) async fn migrate_notes(
    state: State<'_, AppState>,
    project_id: String,
) -> Result<u32, String> {
    tracing::info!(project_id = %project_id, "migrating global notes to project");
    let storage = state.storage.clone();

    tauri::async_runtime::spawn_blocking(move || {
        let id = uuid::Uuid::parse_str(&project_id).map_err(|e| e.to_string())?;
        let storage_guard = storage.lock().map_err(|e| e.to_string())?;
        let project = storage_guard.load_project(id).map_err(|e| e.to_string())?;
        let base_dir = storage_guard.base_dir();
        drop(storage_guard);

        let global_notes = base_dir.join("notes");
        let project_notes = std::path::PathBuf::from(&project.repo_path).join("notes");

        if !global_notes.exists() {
            return Ok(0);
        }

        std::fs::create_dir_all(&project_notes).map_err(|e| e.to_string())?;

        let mut migrated = 0u32;
        for entry in std::fs::read_dir(&global_notes).map_err(|e| e.to_string())? {
            let entry = entry.map_err(|e| e.to_string())?;
            if !entry.file_type().map_err(|e| e.to_string())?.is_dir() {
                continue;
            }
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with('.') {
                continue;
            }
            let dest = project_notes.join(&name);
            if dest.exists() {
                tracing::warn!(
                    "skipping migration of folder '{}': already exists in project",
                    name
                );
                continue;
            }
            copy_dir_recursive(&entry.path(), &dest).map_err(|e| e.to_string())?;
            migrated += 1;
        }

        Ok(migrated)
    })
    .await
    .map_err(|e| format!("Task failed: {}", e))?
}
