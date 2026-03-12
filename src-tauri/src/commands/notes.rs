use tauri::State;

use crate::notes::{self, NoteEntry};
use crate::state::AppState;

pub(crate) fn list_notes(
    state: State<'_, AppState>,
    folder: String,
) -> Result<Vec<NoteEntry>, String> {
    let base_dir = state
        .storage
        .lock()
        .map_err(|e| e.to_string())?
        .base_dir();
    notes::list_notes(&base_dir, &folder).map_err(|e| e.to_string())
}

pub(crate) fn read_note(
    state: State<'_, AppState>,
    folder: String,
    filename: String,
) -> Result<String, String> {
    let base_dir = state
        .storage
        .lock()
        .map_err(|e| e.to_string())?
        .base_dir();
    notes::read_note(&base_dir, &folder, &filename).map_err(|e| e.to_string())
}

pub(crate) fn write_note(
    state: State<'_, AppState>,
    folder: String,
    filename: String,
    content: String,
) -> Result<(), String> {
    let base_dir = state
        .storage
        .lock()
        .map_err(|e| e.to_string())?
        .base_dir();
    notes::write_note(&base_dir, &folder, &filename, &content).map_err(|e| e.to_string())
}

pub(crate) fn create_note(
    state: State<'_, AppState>,
    folder: String,
    title: String,
) -> Result<String, String> {
    let base_dir = state
        .storage
        .lock()
        .map_err(|e| e.to_string())?
        .base_dir();
    notes::create_note(&base_dir, &folder, &title).map_err(|e| e.to_string())
}

pub(crate) fn rename_note(
    state: State<'_, AppState>,
    folder: String,
    old_name: String,
    new_name: String,
) -> Result<String, String> {
    let base_dir = state
        .storage
        .lock()
        .map_err(|e| e.to_string())?
        .base_dir();
    notes::rename_note(&base_dir, &folder, &old_name, &new_name)
        .map_err(|e| e.to_string())
}

pub(crate) fn duplicate_note(
    state: State<'_, AppState>,
    folder: String,
    filename: String,
) -> Result<String, String> {
    let base_dir = state
        .storage
        .lock()
        .map_err(|e| e.to_string())?
        .base_dir();
    notes::duplicate_note(&base_dir, &folder, &filename).map_err(|e| e.to_string())
}

pub(crate) fn delete_note(
    state: State<'_, AppState>,
    folder: String,
    filename: String,
) -> Result<(), String> {
    let base_dir = state
        .storage
        .lock()
        .map_err(|e| e.to_string())?
        .base_dir();
    notes::delete_note(&base_dir, &folder, &filename).map_err(|e| e.to_string())
}

pub(crate) fn list_folders(
    state: State<'_, AppState>,
) -> Result<Vec<String>, String> {
    let base_dir = state.storage.lock().map_err(|e| e.to_string())?.base_dir();
    notes::list_folders(&base_dir).map_err(|e| e.to_string())
}

pub(crate) fn create_folder(
    state: State<'_, AppState>,
    name: String,
) -> Result<(), String> {
    let base_dir = state.storage.lock().map_err(|e| e.to_string())?.base_dir();
    notes::create_folder(&base_dir, &name).map_err(|e| e.to_string())
}

pub(crate) fn rename_folder(
    state: State<'_, AppState>,
    old_name: String,
    new_name: String,
) -> Result<(), String> {
    let base_dir = state.storage.lock().map_err(|e| e.to_string())?.base_dir();
    notes::rename_folder(&base_dir, &old_name, &new_name).map_err(|e| e.to_string())
}

pub(crate) fn delete_folder(
    state: State<'_, AppState>,
    name: String,
    force: bool,
) -> Result<(), String> {
    let base_dir = state.storage.lock().map_err(|e| e.to_string())?.base_dir();
    notes::delete_folder(&base_dir, &name, force).map_err(|e| e.to_string())
}
