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
