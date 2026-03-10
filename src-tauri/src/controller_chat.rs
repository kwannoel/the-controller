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

impl ControllerChatItem {
    pub fn user(text: impl Into<String>) -> Self {
        Self {
            kind: ControllerChatItemKind::User,
            text: text.into(),
        }
    }

    pub fn assistant(text: impl Into<String>) -> Self {
        Self {
            kind: ControllerChatItemKind::Assistant,
            text: text.into(),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ControllerChatSession {
    pub focus: ControllerFocus,
    pub items: Vec<ControllerChatItem>,
    pub turn_in_progress: bool,
}

impl ControllerChatSession {
    pub fn update_focus(&mut self, update: ControllerFocusUpdate) -> Result<(), String> {
        let has_changes = update.project_id.is_some()
            || update.project_name.is_some()
            || update.session_id.is_some()
            || update.note_filename.is_some()
            || update.workspace_mode.is_some();
        if !has_changes {
            return Err("controller focus update must include at least one field".to_string());
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

        Ok(())
    }

    pub fn push_item(&mut self, item: ControllerChatItem) {
        self.items.push(item);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

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
            })
            .unwrap();

        session
            .update_focus(ControllerFocusUpdate {
                project_id: None,
                project_name: None,
                session_id: None,
                note_filename: Some("issue-123.md".to_string()),
                workspace_mode: None,
            })
            .unwrap();

        assert_eq!(session.focus.project_id, Some(project_id));
        assert_eq!(session.focus.project_name.as_deref(), Some("proj"));
        assert_eq!(session.focus.session_id, Some(session_id));
        assert_eq!(session.focus.note_filename.as_deref(), Some("issue-123.md"));
        assert_eq!(session.focus.workspace_mode.as_deref(), Some("notes"));
    }

    #[test]
    fn test_controller_chat_appends_items_in_order() {
        let mut session = ControllerChatSession::default();

        session.push_item(ControllerChatItem::user("Fetch issue 123"));
        session.push_item(ControllerChatItem::assistant(
            "Fetched the issue and wrote it to a note",
        ));

        assert_eq!(session.items.len(), 2);
        assert_eq!(session.items[0].kind, ControllerChatItemKind::User);
        assert_eq!(session.items[0].text, "Fetch issue 123");
        assert_eq!(session.items[1].kind, ControllerChatItemKind::Assistant);
        assert_eq!(
            session.items[1].text,
            "Fetched the issue and wrote it to a note"
        );
    }
}
