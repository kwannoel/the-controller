pub mod commands;
pub mod models;
pub mod pty_manager;
pub mod state;
pub mod storage;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(state::AppState::new())
        .invoke_handler(tauri::generate_handler![
            commands::create_project,
            commands::load_project,
            commands::list_projects,
            commands::archive_project,
            commands::get_agents_md,
            commands::update_agents_md,
            commands::create_session,
            commands::write_to_pty,
            commands::resize_pty,
            commands::close_session,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
