use tauri::Manager;

pub mod auto_worker;
pub mod controller_chat;
pub mod commands;
pub mod config;
pub mod emitter;
pub mod labels;
pub mod maintainer;
pub mod models;
pub mod notes;
pub mod pty_manager;
pub mod session_args;
pub mod skills;
pub mod state;
pub mod status_socket;
pub mod storage;
pub mod tmux;
pub mod token_usage;
pub mod worktree;

fn show_startup_error(error: &std::io::Error) {
    eprintln!("Failed to initialize app storage: {error}");
    let _ = rfd::MessageDialog::new()
        .set_level(rfd::MessageLevel::Error)
        .set_title("The Controller failed to start")
        .set_description(format!(
            "The Controller could not initialize its storage directory.\n\n{}",
            error
        ))
        .show();
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .setup(|app| {
            let emitter = emitter::TauriEmitter::new(app.handle().clone());
            let app_state = match state::AppState::new(emitter) {
                Ok(state) => state,
                Err(error) => {
                    show_startup_error(&error);
                    std::process::exit(1);
                }
            };
            app.manage(app_state);
            skills::sync_skills();
            status_socket::start_listener(app.handle().clone());
            maintainer::MaintainerScheduler::start(app.handle().clone());
            auto_worker::AutoWorkerScheduler::start(app.handle().clone());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::restore_sessions,
            commands::connect_session,
            commands::create_project,
            commands::load_project,
            commands::list_projects,
            commands::delete_project,
            commands::get_agents_md,
            commands::update_agents_md,
            commands::create_session,
            commands::write_to_pty,
            commands::send_raw_to_pty,
            commands::resize_pty,
            commands::close_session,
            commands::set_initial_prompt,
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
            commands::list_github_issues,
            commands::list_assigned_issues,
            commands::generate_issue_body,
            commands::create_github_issue,
            commands::post_github_comment,
            commands::add_github_label,
            commands::remove_github_label,
            commands::merge_session_branch,
            commands::copy_image_file_to_clipboard,
            commands::capture_app_screenshot,
            commands::get_session_commits,
            commands::configure_maintainer,
            commands::get_maintainer_status,
            commands::get_maintainer_history,
            commands::trigger_maintainer_check,
            commands::clear_maintainer_reports,
            commands::get_maintainer_issues,
            commands::get_maintainer_issue_detail,
            commands::configure_auto_worker,
            commands::get_auto_worker_queue,
            commands::get_worker_reports,
            commands::list_notes,
            commands::read_note,
            commands::write_note,
            commands::create_note,
            commands::rename_note,
            commands::delete_note,
            commands::get_controller_chat_session,
            commands::update_controller_chat_focus,
            commands::send_controller_chat_message,
            commands::save_session_prompt,
            commands::list_project_prompts,
            commands::stage_session_inplace,
            commands::unstage_session_inplace,
            commands::get_repo_head,
            commands::get_session_token_usage,
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app_handle, event| {
            if let tauri::RunEvent::ExitRequested { .. } = event {
                status_socket::cleanup();
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
