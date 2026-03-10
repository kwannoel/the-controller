use crate::models::{
    ControllerBridgeActionResult, ControllerChatNoteOpenEvent, ControllerChatTranscriptRow,
    ControllerChatTranscriptRowKind,
};
use crate::notes;
use serde::{Deserialize, Serialize};
use std::path::Path;
use uuid::Uuid;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ControllerFocus {
    pub project_id: Option<Uuid>,
    pub project_name: Option<String>,
    pub session_id: Option<Uuid>,
    pub note_filename: Option<String>,
    pub workspace_mode: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ControllerFocusUpdate {
    pub project_id: Option<Uuid>,
    pub project_name: Option<String>,
    pub session_id: Option<Uuid>,
    pub note_filename: Option<String>,
    pub workspace_mode: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ControllerChatItemKind {
    User,
    Assistant,
}

#[derive(Debug, Clone, PartialEq, Eq)]
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

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ControllerChatSession {
    pub focus: ControllerFocus,
    pub items: Vec<ControllerChatItem>,
    pub turn_in_progress: bool,
}

impl ControllerChatSession {
    pub fn update_focus(&mut self, update: ControllerFocusUpdate) {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::ControllerChatNoteOpenEvent;
    use crate::notes;
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
}
