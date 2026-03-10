use crate::models::{
    ControllerBridgeActionResult, ControllerChatNoteOpenEvent, ControllerChatTranscriptRow,
    ControllerChatTranscriptRowKind,
};
use crate::notes;
use crate::state::AppState;
use serde::{Deserialize, Serialize};
use std::path::Path;
use uuid::Uuid;

pub const CONTROLLER_CHAT_SESSION_UPDATED_EVENT: &str = "controller-chat-session-updated";
pub const CONTROLLER_CHAT_NOTE_OPENED_EVENT: &str = "controller-chat-note-opened";

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ControllerFocus {
    pub project_id: Option<Uuid>,
    pub project_name: Option<String>,
    pub session_id: Option<Uuid>,
    pub note_filename: Option<String>,
    pub workspace_mode: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ControllerFocusUpdate {
    pub project_id: Option<Uuid>,
    pub project_name: Option<String>,
    pub session_id: Option<Uuid>,
    pub note_filename: Option<String>,
    pub workspace_mode: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ControllerChatItemKind {
    User,
    Tool,
    Assistant,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ControllerChatItem {
    pub kind: ControllerChatItemKind,
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "tool", rename_all = "snake_case")]
pub enum ControllerBridgeAction {
    CreateNote { filename: String },
    WriteNote { filename: String, content: String },
    OpenNote { filename: String },
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ControllerChatSession {
    pub focus: ControllerFocus,
    pub items: Vec<ControllerChatItem>,
    pub turn_in_progress: bool,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct ControllerAgentTurnOutput {
    pub assistant_message: String,
    #[serde(default)]
    pub controller_actions: Vec<ControllerBridgeAction>,
}

impl ControllerChatSession {
    pub fn update_focus(&mut self, update: ControllerFocusUpdate) {
        let has_project_scope = update.project_id.is_some();
        let has_session_scope = update.session_id.is_some();
        let has_note_scope = update.note_filename.is_some();
        let project_focus_update = has_project_scope && !has_session_scope && !has_note_scope;
        let project_changed = update
            .project_id
            .is_some_and(|project_id| self.focus.project_id != Some(project_id));

        if project_changed && update.project_name.is_none() {
            self.focus.project_name = None;
        }
        if project_changed || project_focus_update {
            self.focus.session_id = None;
        }
        if project_changed
            || project_focus_update
            || (has_session_scope && !has_note_scope)
        {
            self.focus.note_filename = None;
        }

        if let Some(project_id) = update.project_id {
            self.focus.project_id = Some(project_id);
        }
        if let Some(project_name) = update.project_name {
            self.focus.project_name = Some(project_name);
        }
        if let Some(session_id) = update.session_id {
            self.focus.session_id = Some(session_id);
        }
        if let Some(note_filename) = update.note_filename {
            self.focus.note_filename = Some(note_filename);
        }
        if let Some(workspace_mode) = update.workspace_mode {
            self.focus.workspace_mode = Some(workspace_mode);
        }
    }

    pub fn push_item(&mut self, item: ControllerChatItem) {
        self.items.push(item);
    }
}

fn require_project_context(session: &ControllerChatSession) -> Result<(Uuid, String), String> {
    let project_id = session
        .focus
        .project_id
        .ok_or_else(|| "controller chat focus is missing project_id".to_string())?;
    let project_name = session
        .focus
        .project_name
        .clone()
        .ok_or_else(|| "controller chat focus is missing project_name".to_string())?;
    Ok((project_id, project_name))
}

fn tool_row(text: impl Into<String>) -> ControllerChatTranscriptRow {
    ControllerChatTranscriptRow {
        kind: ControllerChatTranscriptRowKind::Tool,
        text: text.into(),
    }
}

pub fn execute_bridge_actions(
    base: &Path,
    session: &mut ControllerChatSession,
    actions: Vec<ControllerBridgeAction>,
) -> Result<Vec<ControllerBridgeActionResult>, String> {
    let (project_id, project_name) = require_project_context(session)?;
    let mut results = Vec::with_capacity(actions.len());

    for action in actions {
        match action {
            ControllerBridgeAction::CreateNote { filename } => {
                let created_filename = notes::create_note(base, &project_name, &filename)
                    .map_err(|e| e.to_string())?;
                session.update_focus(ControllerFocusUpdate {
                    project_id: None,
                    project_name: None,
                    session_id: None,
                    note_filename: Some(created_filename.clone()),
                    workspace_mode: None,
                });
                results.push(ControllerBridgeActionResult {
                    transcript_row: tool_row(format!(
                        "controller.create_note({})",
                        created_filename
                    )),
                    note_open_event: None,
                });
            }
            ControllerBridgeAction::WriteNote { filename, content } => {
                notes::write_note(base, &project_name, &filename, &content)
                    .map_err(|e| e.to_string())?;
                results.push(ControllerBridgeActionResult {
                    transcript_row: tool_row(format!("controller.write_note({})", filename)),
                    note_open_event: None,
                });
            }
            ControllerBridgeAction::OpenNote { filename } => {
                let exists = notes::note_exists(base, &project_name, &filename)
                    .map_err(|e| e.to_string())?;
                if !exists {
                    return Err(format!("note '{}' not found", filename));
                }

                session.update_focus(ControllerFocusUpdate {
                    project_id: None,
                    project_name: None,
                    session_id: None,
                    note_filename: Some(filename.clone()),
                    workspace_mode: None,
                });
                results.push(ControllerBridgeActionResult {
                    transcript_row: tool_row(format!("controller.open_note({})", filename)),
                    note_open_event: Some(ControllerChatNoteOpenEvent {
                        project_id,
                        filename,
                    }),
                });
            }
        }
    }

    Ok(results)
}

pub fn parse_agent_turn_output(raw: &str) -> Result<ControllerAgentTurnOutput, String> {
    serde_json::from_str(raw)
        .map_err(|e| format!("Failed to parse controller chat output: {}", e))
}

fn emit_session_updated(state: &AppState, session: &ControllerChatSession) -> Result<(), String> {
    let payload = serde_json::to_string(session).map_err(|e| e.to_string())?;
    state
        .emitter
        .emit(CONTROLLER_CHAT_SESSION_UPDATED_EVENT, &payload)
}

fn emit_note_opened(state: &AppState, event: &ControllerChatNoteOpenEvent) -> Result<(), String> {
    let payload = serde_json::to_string(event).map_err(|e| e.to_string())?;
    state
        .emitter
        .emit(CONTROLLER_CHAT_NOTE_OPENED_EVENT, &payload)
}

pub fn get_controller_chat_session(state: &AppState) -> Result<ControllerChatSession, String> {
    state
        .controller_chat
        .lock()
        .map_err(|e| e.to_string())
        .map(|session| session.clone())
}

pub fn update_focus_snapshot(
    state: &AppState,
    update: ControllerFocusUpdate,
) -> Result<ControllerChatSession, String> {
    let snapshot = {
        let mut session = state.controller_chat.lock().map_err(|e| e.to_string())?;
        session.update_focus(update);
        session.clone()
    };
    emit_session_updated(state, &snapshot)?;
    Ok(snapshot)
}

fn build_turn_prompt(session: &ControllerChatSession) -> String {
    let focus = serde_json::to_string(&session.focus).unwrap_or_else(|_| "{}".to_string());
    let transcript = serde_json::to_string(&session.items).unwrap_or_else(|_| "[]".to_string());

    format!(
        "You are the controller chat agent for this project.\n\
Return only valid JSON with this shape:\n\
{{\"assistant_message\":\"...\",\"controller_actions\":[...]}}\n\
Environment tools are available to you directly. Only app-owned note actions may appear in controller_actions.\n\
Valid controller actions:\n\
- {{\"tool\":\"create_note\",\"filename\":\"note.md\"}}\n\
- {{\"tool\":\"write_note\",\"filename\":\"note.md\",\"content\":\"...\"}}\n\
- {{\"tool\":\"open_note\",\"filename\":\"note.md\"}}\n\
Current focus JSON:\n{focus}\n\
Transcript JSON:\n{transcript}\n"
    )
}

pub async fn run_controller_turn_spawn_blocking_with<F>(
    repo_path: String,
    prompt: String,
    runner: F,
) -> Result<ControllerAgentTurnOutput, String>
where
    F: FnOnce(String, String) -> Result<ControllerAgentTurnOutput, String> + Send + 'static,
{
    tokio::task::spawn_blocking(move || runner(repo_path, prompt))
        .await
        .map_err(|e| format!("Task failed: {}", e))?
}

fn run_codex_turn(repo_path: String, prompt: String) -> Result<ControllerAgentTurnOutput, String> {
    let output = std::process::Command::new("codex")
        .arg("exec")
        .arg("--sandbox")
        .arg("danger-full-access")
        .arg("--ask-for-approval")
        .arg("never")
        .arg(&prompt)
        .current_dir(repo_path)
        .env_remove("CLAUDECODE")
        .output()
        .map_err(|e| format!("Failed to run codex exec: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("codex exec failed: {}", stderr.trim()));
    }

    parse_agent_turn_output(String::from_utf8_lossy(&output.stdout).trim())
}

pub async fn send_message_with_runner<F>(
    state: &AppState,
    message: String,
    runner: F,
) -> Result<ControllerChatSession, String>
where
    F: FnOnce(String, String) -> Result<ControllerAgentTurnOutput, String> + Send + 'static,
{
    let (repo_path, prompt, base_dir) = {
        let mut session = state.controller_chat.lock().map_err(|e| e.to_string())?;
        let project_id = session
            .focus
            .project_id
            .ok_or_else(|| "controller chat focus is missing project_id".to_string())?;
        let storage = state.storage.lock().map_err(|e| e.to_string())?;
        let project = storage.load_project(project_id).map_err(|e| e.to_string())?;
        let base_dir = storage.base_dir();

        session.push_item(ControllerChatItem {
            kind: ControllerChatItemKind::User,
            text: message,
        });
        session.turn_in_progress = true;
        let prompt = build_turn_prompt(&session);
        let snapshot = session.clone();
        drop(session);
        emit_session_updated(state, &snapshot)?;

        (project.repo_path, prompt, base_dir)
    };

    let turn_result = run_controller_turn_spawn_blocking_with(repo_path, prompt, runner).await;

    match turn_result {
        Ok(output) => {
            let bridge_outcome = {
                let mut session = state.controller_chat.lock().map_err(|e| e.to_string())?;
                let bridge_results = match execute_bridge_actions(
                    &base_dir,
                    &mut session,
                    output.controller_actions,
                ) {
                    Ok(results) => results,
                    Err(error) => {
                        session.turn_in_progress = false;
                        let snapshot = session.clone();
                        drop(session);
                        let _ = emit_session_updated(state, &snapshot);
                        return Err(error);
                    }
                };

                for result in &bridge_results {
                    session.push_item(ControllerChatItem {
                        kind: ControllerChatItemKind::Tool,
                        text: result.transcript_row.text.clone(),
                    });
                }
                session.push_item(ControllerChatItem {
                    kind: ControllerChatItemKind::Assistant,
                    text: output.assistant_message,
                });
                session.turn_in_progress = false;
                (session.clone(), bridge_results)
            };
            let (snapshot, bridge_results) = bridge_outcome;

            emit_session_updated(state, &snapshot)?;
            for result in &bridge_results {
                if let Some(note_open_event) = &result.note_open_event {
                    emit_note_opened(state, note_open_event)?;
                }
            }
            Ok(snapshot)
        }
        Err(error) => {
            let snapshot = {
                let mut session = state.controller_chat.lock().map_err(|e| e.to_string())?;
                session.turn_in_progress = false;
                session.clone()
            };
            let _ = emit_session_updated(state, &snapshot);
            Err(error)
        }
    }
}

pub async fn send_message(
    state: &AppState,
    message: String,
) -> Result<ControllerChatSession, String> {
    send_message_with_runner(state, message, run_codex_turn).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::emitter::EventEmitter;
    use crate::models::{
        AutoWorkerConfig, ControllerChatNoteOpenEvent, MaintainerConfig, Project, SavedPrompt,
        SessionConfig,
    };
    use crate::notes;
    use crate::state::AppState;
    use crate::storage::Storage;
    use std::path::Path;
    use std::sync::{Arc, Mutex};
    use std::thread;
    use tempfile::TempDir;
    use uuid::Uuid;

    fn focused_session(project_name: &str) -> ControllerChatSession {
        let mut session = ControllerChatSession::default();
        session.update_focus(ControllerFocusUpdate {
            project_id: Some(Uuid::from_u128(42)),
            project_name: Some(project_name.to_string()),
            session_id: Some(Uuid::from_u128(7)),
            note_filename: None,
            workspace_mode: Some("notes".to_string()),
        });
        session
    }

    #[derive(Default)]
    struct RecordingEmitter {
        events: Mutex<Vec<(String, String)>>,
    }

    impl RecordingEmitter {
        fn recorded(&self) -> Vec<(String, String)> {
            self.events.lock().unwrap().clone()
        }
    }

    impl EventEmitter for RecordingEmitter {
        fn emit(&self, event: &str, payload: &str) -> Result<(), String> {
            self.events
                .lock()
                .unwrap()
                .push((event.to_string(), payload.to_string()));
            Ok(())
        }
    }

    fn make_test_project(project_id: Uuid, repo_path: &Path) -> Project {
        Project {
            id: project_id,
            name: "proj".to_string(),
            repo_path: repo_path.to_string_lossy().to_string(),
            created_at: "2026-03-10T00:00:00Z".to_string(),
            archived: false,
            sessions: Vec::<SessionConfig>::new(),
            maintainer: MaintainerConfig::default(),
            auto_worker: AutoWorkerConfig::default(),
            prompts: Vec::<SavedPrompt>::new(),
            staged_session: None,
        }
    }

    fn make_test_state(base_dir: &Path, emitter: Arc<dyn EventEmitter>) -> AppState {
        let storage = Storage::new(base_dir.to_path_buf());
        AppState::from_storage(storage, emitter).expect("app state")
    }

    #[test]
    fn test_controller_chat_session_starts_without_focus() {
        let session = ControllerChatSession::default();

        assert!(session.focus.project_id.is_none());
        assert!(session.focus.project_name.is_none());
        assert!(session.focus.session_id.is_none());
        assert!(session.focus.note_filename.is_none());
        assert!(session.focus.workspace_mode.is_none());
        assert!(session.items.is_empty());
        assert!(!session.turn_in_progress);
    }

    #[test]
    fn test_controller_focus_updates_note_without_dropping_project() {
        let mut session = ControllerChatSession::default();
        let project_id = Uuid::nil();
        let session_id = Uuid::from_u128(1);

        session
            .update_focus(ControllerFocusUpdate {
                project_id: Some(project_id),
                project_name: Some("proj".to_string()),
                session_id: Some(session_id),
                note_filename: None,
                workspace_mode: Some("notes".to_string()),
            });

        session
            .update_focus(ControllerFocusUpdate {
                project_id: None,
                project_name: None,
                session_id: None,
                note_filename: Some("issue-123.md".to_string()),
                workspace_mode: None,
            });

        assert_eq!(session.focus.project_id, Some(project_id));
        assert_eq!(session.focus.project_name.as_deref(), Some("proj"));
        assert_eq!(session.focus.session_id, Some(session_id));
        assert_eq!(session.focus.note_filename.as_deref(), Some("issue-123.md"));
        assert_eq!(session.focus.workspace_mode.as_deref(), Some("notes"));
    }

    #[test]
    fn test_controller_focus_clears_stale_session_and_note_on_project_change() {
        let mut session = ControllerChatSession::default();
        let project_a_id = Uuid::from_u128(1);
        let project_b_id = Uuid::from_u128(2);
        let session_id = Uuid::from_u128(3);

        session.update_focus(ControllerFocusUpdate {
            project_id: Some(project_a_id),
            project_name: Some("proj-a".to_string()),
            session_id: Some(session_id),
            note_filename: Some("a.md".to_string()),
            workspace_mode: Some("notes".to_string()),
        });

        session.update_focus(ControllerFocusUpdate {
            project_id: Some(project_b_id),
            project_name: Some("proj-b".to_string()),
            session_id: None,
            note_filename: None,
            workspace_mode: None,
        });

        assert_eq!(session.focus.project_id, Some(project_b_id));
        assert_eq!(session.focus.project_name.as_deref(), Some("proj-b"));
        assert!(session.focus.session_id.is_none());
        assert!(session.focus.note_filename.is_none());
    }

    #[test]
    fn test_controller_focus_clears_stale_session_and_note_on_same_project_project_focus() {
        let mut session = ControllerChatSession::default();
        let project_id = Uuid::from_u128(10);
        let session_id = Uuid::from_u128(11);

        session.update_focus(ControllerFocusUpdate {
            project_id: Some(project_id),
            project_name: Some("proj".to_string()),
            session_id: Some(session_id),
            note_filename: Some("a.md".to_string()),
            workspace_mode: Some("notes".to_string()),
        });

        session.update_focus(ControllerFocusUpdate {
            project_id: Some(project_id),
            project_name: Some("proj".to_string()),
            session_id: None,
            note_filename: None,
            workspace_mode: None,
        });

        assert_eq!(session.focus.project_id, Some(project_id));
        assert_eq!(session.focus.project_name.as_deref(), Some("proj"));
        assert!(session.focus.session_id.is_none());
        assert!(session.focus.note_filename.is_none());
    }

    #[test]
    fn test_controller_focus_clears_stale_note_on_session_focus_update() {
        let mut session = ControllerChatSession::default();
        let project_id = Uuid::from_u128(20);
        let old_session_id = Uuid::from_u128(21);
        let new_session_id = Uuid::from_u128(22);

        session.update_focus(ControllerFocusUpdate {
            project_id: Some(project_id),
            project_name: Some("proj".to_string()),
            session_id: Some(old_session_id),
            note_filename: Some("a.md".to_string()),
            workspace_mode: Some("notes".to_string()),
        });

        session.update_focus(ControllerFocusUpdate {
            project_id: None,
            project_name: None,
            session_id: Some(new_session_id),
            note_filename: None,
            workspace_mode: None,
        });

        assert_eq!(session.focus.project_id, Some(project_id));
        assert_eq!(session.focus.session_id, Some(new_session_id));
        assert!(session.focus.note_filename.is_none());
    }

    #[test]
    fn test_controller_focus_preserves_session_on_same_project_note_focus_update() {
        let mut session = ControllerChatSession::default();
        let project_id = Uuid::from_u128(30);
        let session_id = Uuid::from_u128(31);

        session.update_focus(ControllerFocusUpdate {
            project_id: Some(project_id),
            project_name: Some("proj".to_string()),
            session_id: Some(session_id),
            note_filename: Some("old.md".to_string()),
            workspace_mode: Some("notes".to_string()),
        });

        session.update_focus(ControllerFocusUpdate {
            project_id: Some(project_id),
            project_name: Some("proj".to_string()),
            session_id: None,
            note_filename: Some("a.md".to_string()),
            workspace_mode: None,
        });

        assert_eq!(session.focus.project_id, Some(project_id));
        assert_eq!(session.focus.session_id, Some(session_id));
        assert_eq!(session.focus.note_filename.as_deref(), Some("a.md"));
    }

    #[test]
    fn test_controller_chat_appends_items_in_order() {
        let mut session = ControllerChatSession::default();

        session.push_item(ControllerChatItem {
            kind: ControllerChatItemKind::User,
            text: "Fetch issue 123".to_string(),
        });
        session.push_item(ControllerChatItem {
            kind: ControllerChatItemKind::Assistant,
            text: "Fetched the issue and wrote it to a note".to_string(),
        });

        assert_eq!(session.items.len(), 2);
        assert_eq!(session.items[0].kind, ControllerChatItemKind::User);
        assert_eq!(session.items[0].text, "Fetch issue 123");
        assert_eq!(session.items[1].kind, ControllerChatItemKind::Assistant);
        assert_eq!(
            session.items[1].text,
            "Fetched the issue and wrote it to a note"
        );
    }

    #[test]
    fn test_execute_create_and_write_note_bridge_action() {
        let tmp = TempDir::new().unwrap();
        let mut session = focused_session("proj");

        let results = execute_bridge_actions(
            tmp.path(),
            &mut session,
            vec![
                ControllerBridgeAction::CreateNote {
                    filename: "issue-123.md".to_string(),
                },
                ControllerBridgeAction::WriteNote {
                    filename: "issue-123.md".to_string(),
                    content: "# Issue 123\n".to_string(),
                },
            ],
        )
        .unwrap();

        assert_eq!(
            notes::read_note(tmp.path(), "proj", "issue-123.md").unwrap(),
            "# Issue 123\n"
        );
        assert_eq!(session.focus.note_filename.as_deref(), Some("issue-123.md"));
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].transcript_row.text, "controller.create_note(issue-123.md)");
        assert_eq!(results[1].transcript_row.text, "controller.write_note(issue-123.md)");
    }

    #[test]
    fn test_execute_open_note_returns_event_payload() {
        let tmp = TempDir::new().unwrap();
        let mut session = focused_session("proj");
        let filename = notes::create_note(tmp.path(), "proj", "issue-456").unwrap();

        let results = execute_bridge_actions(
            tmp.path(),
            &mut session,
            vec![ControllerBridgeAction::OpenNote {
                filename: filename.clone(),
            }],
        )
        .unwrap();

        assert_eq!(results.len(), 1);
        assert_eq!(
            results[0].note_open_event,
            Some(ControllerChatNoteOpenEvent {
                project_id: Uuid::from_u128(42),
                filename: filename.clone(),
            })
        );
        assert_eq!(session.focus.note_filename.as_deref(), Some(filename.as_str()));
    }

    #[test]
    fn test_execute_bridge_actions_rejects_invalid_filename() {
        let tmp = TempDir::new().unwrap();
        let mut session = focused_session("proj");

        let error = execute_bridge_actions(
            tmp.path(),
            &mut session,
            vec![ControllerBridgeAction::CreateNote {
                filename: "../escape.md".to_string(),
            }],
        )
        .unwrap_err();

        assert!(error.contains("invalid note filename"));
    }

    #[test]
    fn test_execute_bridge_actions_rejects_stale_project_name_after_project_change() {
        let tmp = TempDir::new().unwrap();
        let mut session = focused_session("proj-a");

        session.update_focus(ControllerFocusUpdate {
            project_id: Some(Uuid::from_u128(77)),
            project_name: None,
            session_id: None,
            note_filename: None,
            workspace_mode: None,
        });

        let error = execute_bridge_actions(
            tmp.path(),
            &mut session,
            vec![ControllerBridgeAction::CreateNote {
                filename: "issue-777.md".to_string(),
            }],
        )
        .unwrap_err();

        assert!(error.contains("missing project_name"));
    }

    #[test]
    fn test_update_focus_snapshot_starts_controller_chat_from_snapshot() {
        let tmp = TempDir::new().unwrap();
        let emitter = Arc::new(RecordingEmitter::default());
        let state = make_test_state(tmp.path(), emitter.clone());

        let session = update_focus_snapshot(
            &state,
            ControllerFocusUpdate {
                project_id: Some(Uuid::from_u128(99)),
                project_name: Some("proj".to_string()),
                session_id: Some(Uuid::from_u128(100)),
                note_filename: Some("focus.md".to_string()),
                workspace_mode: Some("notes".to_string()),
            },
        )
        .unwrap();

        assert_eq!(session.focus.project_id, Some(Uuid::from_u128(99)));
        assert_eq!(session.focus.note_filename.as_deref(), Some("focus.md"));
        assert_eq!(emitter.recorded().len(), 1);
    }

    #[test]
    fn test_parse_agent_turn_output_parses_controller_actions() {
        let parsed = parse_agent_turn_output(
            r#"{
                "assistant_message": "Done.",
                "controller_actions": [
                    { "tool": "open_note", "filename": "issue-123.md" }
                ]
            }"#,
        )
        .unwrap();

        assert_eq!(parsed.assistant_message, "Done.");
        assert_eq!(
            parsed.controller_actions,
            vec![ControllerBridgeAction::OpenNote {
                filename: "issue-123.md".to_string(),
            }]
        );
    }

    #[test]
    fn test_send_message_with_runner_rejects_missing_project_focus() {
        let tmp = TempDir::new().unwrap();
        let emitter = Arc::new(RecordingEmitter::default());
        let state = make_test_state(tmp.path(), emitter);

        let error = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(send_message_with_runner(
                &state,
                "fetch issue".to_string(),
                |_repo_path, _prompt| Ok(ControllerAgentTurnOutput {
                    assistant_message: "unused".to_string(),
                    controller_actions: vec![],
                }),
            ))
            .unwrap_err();

        assert!(error.contains("project"));
    }

    #[test]
    fn test_send_message_with_runner_appends_tool_rows_and_assistant_text() {
        let tmp = TempDir::new().unwrap();
        let repo_dir = tmp.path().join("repo");
        std::fs::create_dir_all(&repo_dir).unwrap();
        let emitter = Arc::new(RecordingEmitter::default());
        let state = make_test_state(tmp.path(), emitter.clone());
        let project_id = Uuid::from_u128(200);

        {
            let storage = state.storage.lock().unwrap();
            storage
                .save_project(&make_test_project(project_id, &repo_dir))
                .unwrap();
        }

        update_focus_snapshot(
            &state,
            ControllerFocusUpdate {
                project_id: Some(project_id),
                project_name: Some("proj".to_string()),
                session_id: None,
                note_filename: None,
                workspace_mode: Some("notes".to_string()),
            },
        )
        .unwrap();

        let session = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(send_message_with_runner(
                &state,
                "fetch issue 123".to_string(),
                |_repo_path, _prompt| {
                    Ok(ControllerAgentTurnOutput {
                        assistant_message: "Fetched the issue and opened the note.".to_string(),
                        controller_actions: vec![
                            ControllerBridgeAction::CreateNote {
                                filename: "issue-123.md".to_string(),
                            },
                            ControllerBridgeAction::WriteNote {
                                filename: "issue-123.md".to_string(),
                                content: "# Issue 123\n".to_string(),
                            },
                            ControllerBridgeAction::OpenNote {
                                filename: "issue-123.md".to_string(),
                            },
                        ],
                    })
                },
            ))
            .unwrap();

        assert_eq!(session.items.len(), 5);
        assert_eq!(session.items[0].kind, ControllerChatItemKind::User);
        assert_eq!(session.items[1].kind, ControllerChatItemKind::Tool);
        assert_eq!(session.items[2].kind, ControllerChatItemKind::Tool);
        assert_eq!(session.items[3].kind, ControllerChatItemKind::Tool);
        assert_eq!(session.items[4].kind, ControllerChatItemKind::Assistant);
        assert_eq!(session.items[4].text, "Fetched the issue and opened the note.");
        assert_eq!(session.focus.note_filename.as_deref(), Some("issue-123.md"));

        let events = emitter.recorded();
        assert!(events.iter().any(|(event, _)| event == CONTROLLER_CHAT_SESSION_UPDATED_EVENT));
        assert!(events.iter().any(|(event, _)| event == CONTROLLER_CHAT_NOTE_OPENED_EVENT));
    }

    #[test]
    fn test_run_controller_turn_spawn_blocking_with_offloads_work() {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();

        runtime.block_on(async {
            let runtime_thread_id = thread::current().id();
            let output = run_controller_turn_spawn_blocking_with(
                "/tmp/project".to_string(),
                "prompt".to_string(),
                move |repo_path, prompt| {
                    assert_eq!(repo_path, "/tmp/project");
                    assert_eq!(prompt, "prompt");
                    assert_ne!(thread::current().id(), runtime_thread_id);
                    Ok(ControllerAgentTurnOutput {
                        assistant_message: "done".to_string(),
                        controller_actions: vec![],
                    })
                },
            )
            .await
            .unwrap();

            assert_eq!(output.assistant_message, "done");
        });
    }
}
