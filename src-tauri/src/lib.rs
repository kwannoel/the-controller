use tauri::Manager;

pub mod commands;
pub mod config;
pub mod models;
pub mod pty_manager;
pub mod state;
pub mod storage;
pub mod tmux;
pub mod worktree;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .manage(state::AppState::new())
        .invoke_handler(tauri::generate_handler![
            commands::restore_sessions,
            commands::create_project,
            commands::load_project,
            commands::list_projects,
            commands::archive_project,
            commands::delete_project,
            commands::list_archived_projects,
            commands::unarchive_project,
            commands::get_agents_md,
            commands::update_agents_md,
            commands::create_session,
            commands::write_to_pty,
            commands::send_raw_to_pty,
            commands::resize_pty,
            commands::close_session,
            commands::archive_session,
            commands::unarchive_session,
            commands::start_claude_login,
            commands::stop_claude_login,
            commands::home_dir,
            commands::check_onboarding,
            commands::save_onboarding_config,
            commands::check_claude_cli,
            commands::list_directories_at,
            commands::list_root_directories,
            commands::generate_project_names,
            commands::scaffold_project,
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app_handle, event| {
            if let tauri::RunEvent::ExitRequested { .. } = event {
                // In release builds, kill tmux sessions on quit so they don't linger.
                // In dev builds, let tmux sessions survive so they reattach after
                // cargo-watch restarts the app (the whole point of the tmux layer).
                if cfg!(not(debug_assertions)) {
                    if let Some(state) = app_handle.try_state::<state::AppState>() {
                        if let Ok(mut pty_manager) = state.pty_manager.lock() {
                            let ids = pty_manager.session_ids();
                            for id in ids {
                                let _ = pty_manager.close_session(id);
                            }
                        }
                    }
                }
            }
        });
}
