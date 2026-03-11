use tauri::State;

use crate::notes::{self, NoteEntry};
use crate::state::AppState;

pub(crate) fn list_notes(
    state: State<'_, AppState>,
    project_name: String,
) -> Result<Vec<NoteEntry>, String> {
    let base_dir = state
        .storage
        .lock()
        .map_err(|e| e.to_string())?
        .base_dir();
    notes::list_notes(&base_dir, &project_name).map_err(|e| e.to_string())
}

pub(crate) fn read_note(
    state: State<'_, AppState>,
    project_name: String,
    filename: String,
) -> Result<String, String> {
    let base_dir = state
        .storage
        .lock()
        .map_err(|e| e.to_string())?
        .base_dir();
    notes::read_note(&base_dir, &project_name, &filename).map_err(|e| e.to_string())
}

pub(crate) fn write_note(
    state: State<'_, AppState>,
    project_name: String,
    filename: String,
    content: String,
) -> Result<(), String> {
    let base_dir = state
        .storage
        .lock()
        .map_err(|e| e.to_string())?
        .base_dir();
    notes::write_note(&base_dir, &project_name, &filename, &content).map_err(|e| e.to_string())
}

pub(crate) fn create_note(
    state: State<'_, AppState>,
    project_name: String,
    title: String,
) -> Result<String, String> {
    let base_dir = state
        .storage
        .lock()
        .map_err(|e| e.to_string())?
        .base_dir();
    notes::create_note(&base_dir, &project_name, &title).map_err(|e| e.to_string())
}

pub(crate) fn rename_note(
    state: State<'_, AppState>,
    project_name: String,
    old_name: String,
    new_name: String,
) -> Result<String, String> {
    let base_dir = state
        .storage
        .lock()
        .map_err(|e| e.to_string())?
        .base_dir();
    notes::rename_note(&base_dir, &project_name, &old_name, &new_name)
        .map_err(|e| e.to_string())
}

pub(crate) fn duplicate_note(
    state: State<'_, AppState>,
    project_name: String,
    filename: String,
) -> Result<String, String> {
    let base_dir = state
        .storage
        .lock()
        .map_err(|e| e.to_string())?
        .base_dir();
    notes::duplicate_note(&base_dir, &project_name, &filename).map_err(|e| e.to_string())
}

pub(crate) fn delete_note(
    state: State<'_, AppState>,
    project_name: String,
    filename: String,
) -> Result<(), String> {
    let base_dir = state
        .storage
        .lock()
        .map_err(|e| e.to_string())?
        .base_dir();
    notes::delete_note(&base_dir, &project_name, &filename).map_err(|e| e.to_string())
}
