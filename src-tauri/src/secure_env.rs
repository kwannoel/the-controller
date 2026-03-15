use std::fs;
use std::path::{Path, PathBuf};
use std::sync::mpsc::SyncSender;

use uuid::Uuid;

use crate::state::AppState;

#[allow(dead_code)]
pub struct EnvWriteResult {
    pub created: bool,
}

#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingSecureEnvRequest {
    pub request_id: String,
    pub project_id: Uuid,
    pub project_name: String,
    pub env_path: PathBuf,
    pub key: String,
}

#[allow(dead_code)]
#[derive(Debug)]
pub(crate) struct ActiveSecureEnvRequest {
    pub(crate) pending: PendingSecureEnvRequest,
    pub(crate) response_tx: Option<SyncSender<SecureEnvResponse>>,
}

#[allow(dead_code)]
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct SecureEnvRequest {
    pub(crate) project_selector: String,
    pub(crate) key: String,
    pub(crate) request_id: String,
}

#[allow(dead_code)]
#[derive(Debug, PartialEq, Eq)]
pub enum SecureEnvResponseKind {
    Ok,
    Error,
}

#[allow(dead_code)]
#[derive(Debug, PartialEq, Eq)]
pub struct SecureEnvResponse {
    pub kind: SecureEnvResponseKind,
    pub status: String,
    pub request_id: String,
}

#[allow(dead_code)]
pub(crate) fn validate_env_key(key: &str) -> Result<(), String> {
    if key.is_empty() {
        return Err("Env var key cannot be empty".to_string());
    }
    if key.contains('=') {
        return Err("Env var key cannot contain '='".to_string());
    }
    let mut chars = key.chars();
    let Some(first) = chars.next() else {
        return Err("Env var key cannot be empty".to_string());
    };
    if !(first.is_ascii_alphabetic() || first == '_')
        || !chars.all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
    {
        return Err("Env var key must match [A-Za-z_][A-Za-z0-9_]*".to_string());
    }
    Ok(())
}

#[allow(dead_code)]
pub(crate) fn parse_secure_env_request(message: &str) -> Result<SecureEnvRequest, String> {
    let mut parts = message.split('|');
    let action = parts
        .next()
        .ok_or_else(|| "Invalid secure env request message".to_string())?;
    let project_selector = parts
        .next()
        .ok_or_else(|| "Invalid secure env request message".to_string())?;
    let key = parts
        .next()
        .ok_or_else(|| "Invalid secure env request message".to_string())?;
    let request_id = parts
        .next()
        .ok_or_else(|| "Invalid secure env request message".to_string())?;

    if parts.next().is_some()
        || project_selector.is_empty()
        || key.is_empty()
        || request_id.is_empty()
    {
        return Err("Invalid secure env request message".to_string());
    }

    if action != "set" {
        return Err(format!("Unsupported secure env request action: {action}"));
    }

    validate_env_key(key)?;

    Ok(SecureEnvRequest {
        project_selector: project_selector.to_string(),
        key: key.to_string(),
        request_id: request_id.to_string(),
    })
}

#[allow(dead_code)]
pub(crate) fn format_secure_env_response(response: &SecureEnvResponse) -> String {
    let kind = match response.kind {
        SecureEnvResponseKind::Ok => "ok",
        SecureEnvResponseKind::Error => "error",
    };
    format!("{kind}|{}|{}", response.status, response.request_id)
}

#[allow(dead_code)]
pub(crate) fn begin_secure_env_request(
    state: &AppState,
    project_selector: &str,
    key: &str,
    request_id: &str,
) -> Result<PendingSecureEnvRequest, String> {
    begin_secure_env_request_with_response(state, project_selector, key, request_id, None)
}

#[allow(dead_code)]
pub(crate) fn begin_secure_env_request_with_response(
    state: &AppState,
    project_selector: &str,
    key: &str,
    request_id: &str,
    response_tx: Option<SyncSender<SecureEnvResponse>>,
) -> Result<PendingSecureEnvRequest, String> {
    validate_env_key(key)?;

    let project = {
        let storage = state.storage.lock().map_err(|err| err.to_string())?;
        let inventory = storage.list_projects().map_err(|err| err.to_string())?;
        inventory
            .projects
            .into_iter()
            .find(|project| {
                project.name == project_selector || project.id.to_string() == project_selector
            })
            .ok_or_else(|| format!("Unknown project: {project_selector}"))?
    };

    let pending = PendingSecureEnvRequest {
        request_id: request_id.to_string(),
        project_id: project.id,
        project_name: project.name.clone(),
        env_path: PathBuf::from(&project.repo_path).join(".env"),
        key: key.to_string(),
    };

    let mut active = state
        .secure_env_request
        .lock()
        .map_err(|err| err.to_string())?;
    if active.is_some() {
        return Err("A secure env request is already active".to_string());
    }
    *active = Some(ActiveSecureEnvRequest {
        pending: pending.clone(),
        response_tx,
    });

    Ok(pending)
}

#[allow(dead_code)]
pub fn cancel_secure_env_request(state: &AppState, request_id: &str) -> Result<(), String> {
    let mut active = state
        .secure_env_request
        .lock()
        .map_err(|err| err.to_string())?;
    match active.as_ref() {
        Some(request) if request.pending.request_id == request_id => {
            let response_tx = request.response_tx.clone();
            *active = None;
            if let Some(response_tx) = response_tx {
                let _ = response_tx.send(SecureEnvResponse {
                    kind: SecureEnvResponseKind::Error,
                    status: "cancelled".to_string(),
                    request_id: request_id.to_string(),
                });
            }
            Ok(())
        }
        Some(_) => Err(format!("Unknown secure env request: {request_id}")),
        None => Err("No active secure env request".to_string()),
    }
}

#[allow(dead_code)]
pub fn take_secure_env_submission(
    state: &AppState,
    request_id: &str,
) -> Result<
    (
        PendingSecureEnvRequest,
        Option<SyncSender<SecureEnvResponse>>,
    ),
    String,
> {
    let mut active = state
        .secure_env_request
        .lock()
        .map_err(|err| err.to_string())?;
    match active.as_ref() {
        Some(request) if request.pending.request_id == request_id => {
            let pending = request.pending.clone();
            let response_tx = request.response_tx.clone();
            *active = None;
            Ok((pending, response_tx))
        }
        Some(_) => Err(format!("Unknown secure env request: {request_id}")),
        None => Err("No active secure env request".to_string()),
    }
}

#[allow(dead_code)]
pub fn finish_secure_env_submission(
    request_id: &str,
    response_tx: Option<SyncSender<SecureEnvResponse>>,
    result: Result<EnvWriteResult, String>,
) -> Result<EnvWriteResult, String> {
    match result {
        Ok(result) => {
            if let Some(response_tx) = response_tx {
                let _ = response_tx.send(SecureEnvResponse {
                    kind: SecureEnvResponseKind::Ok,
                    status: if result.created {
                        "created".to_string()
                    } else {
                        "updated".to_string()
                    },
                    request_id: request_id.to_string(),
                });
            }
            Ok(result)
        }
        Err(error) => {
            if let Some(response_tx) = response_tx {
                let _ = response_tx.send(SecureEnvResponse {
                    kind: SecureEnvResponseKind::Error,
                    status: "write-failed".to_string(),
                    request_id: request_id.to_string(),
                });
            }
            Err(error)
        }
    }
}

#[allow(dead_code)]
pub(crate) fn submit_secure_env_value(
    state: &AppState,
    request_id: &str,
    value: &str,
) -> Result<EnvWriteResult, String> {
    let (pending, response_tx) = take_secure_env_submission(state, request_id)?;
    finish_secure_env_submission(
        request_id,
        response_tx,
        update_env_file(&pending.env_path, &pending.key, value),
    )
}

#[allow(dead_code)]
pub fn update_env_file(env_path: &Path, key: &str, value: &str) -> Result<EnvWriteResult, String> {
    validate_env_key(key)?;

    let existing = match fs::read_to_string(env_path) {
        Ok(contents) => contents,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(err) => return Err(format!("failed to read {}: {}", env_path.display(), err)),
    };

    let mut replaced = false;
    let mut lines = Vec::new();
    for line in existing.lines() {
        if line
            .strip_prefix(key)
            .and_then(|rest| rest.strip_prefix('='))
            .is_some()
        {
            lines.push(format!("{key}={value}"));
            replaced = true;
        } else {
            lines.push(line.to_string());
        }
    }

    if !replaced {
        lines.push(format!("{key}={value}"));
    }

    let mut updated = lines.join("\n");
    updated.push('\n');

    fs::write(env_path, updated)
        .map_err(|err| format!("failed to write {}: {}", env_path.display(), err))?;

    Ok(EnvWriteResult { created: !replaced })
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;

    use tempfile::TempDir;
    use uuid::Uuid;

    use super::{
        begin_secure_env_request, cancel_secure_env_request, format_secure_env_response,
        parse_secure_env_request, submit_secure_env_value, update_env_file, validate_env_key,
        SecureEnvResponse, SecureEnvResponseKind,
    };
    use crate::emitter::NoopEmitter;
    use crate::models::{AutoWorkerConfig, MaintainerConfig, Project};
    use crate::state::AppState;
    use crate::storage::Storage;

    fn make_app_state(tmp: &TempDir) -> AppState {
        AppState::from_storage(Storage::new(tmp.path().to_path_buf()), NoopEmitter::new()).unwrap()
    }

    fn save_project(state: &AppState, name: &str, repo_path: PathBuf) -> Project {
        let project = Project {
            id: Uuid::new_v4(),
            name: name.to_string(),
            repo_path: repo_path.to_string_lossy().to_string(),
            created_at: "2026-03-10T00:00:00Z".to_string(),
            archived: false,
            maintainer: MaintainerConfig::default(),
            auto_worker: AutoWorkerConfig::default(),
            prompts: vec![],
            sessions: vec![],
            staged_session: None,
        };

        state
            .storage
            .lock()
            .unwrap()
            .save_project(&project)
            .unwrap();
        project
    }

    #[test]
    fn updates_existing_env_key_without_touching_other_lines() {
        let tmp = TempDir::new().unwrap();
        let env_path = tmp.path().join(".env");
        fs::write(&env_path, "# comment\nOPENAI_API_KEY=old\nFOO=bar\n").unwrap();

        let result = update_env_file(&env_path, "OPENAI_API_KEY", "new-secret").unwrap();

        let updated = fs::read_to_string(&env_path).unwrap();
        assert_eq!(updated, "# comment\nOPENAI_API_KEY=new-secret\nFOO=bar\n");
        assert!(!result.created);
    }

    #[test]
    fn appends_missing_env_key() {
        let tmp = TempDir::new().unwrap();
        let env_path = tmp.path().join(".env");
        fs::write(&env_path, "FOO=bar\n").unwrap();

        let result = update_env_file(&env_path, "OPENAI_API_KEY", "new-secret").unwrap();

        let updated = fs::read_to_string(&env_path).unwrap();
        assert_eq!(updated, "FOO=bar\nOPENAI_API_KEY=new-secret\n");
        assert!(result.created);
    }

    #[test]
    fn creates_env_file_when_missing() {
        let tmp = TempDir::new().unwrap();
        let env_path = tmp.path().join(".env");

        update_env_file(&env_path, "OPENAI_API_KEY", "new-secret").unwrap();

        let updated = fs::read_to_string(&env_path).unwrap();
        assert_eq!(updated, "OPENAI_API_KEY=new-secret\n");
    }

    #[test]
    fn preserves_unrelated_lines_and_comments() {
        let tmp = TempDir::new().unwrap();
        let env_path = tmp.path().join(".env");
        fs::write(
            &env_path,
            "# top\nEMPTY=\nFOO=bar\n# trailing comment\nBAR=baz\n",
        )
        .unwrap();

        update_env_file(&env_path, "OPENAI_API_KEY", "new-secret").unwrap();

        let updated = fs::read_to_string(&env_path).unwrap();
        assert_eq!(
            updated,
            "# top\nEMPTY=\nFOO=bar\n# trailing comment\nBAR=baz\nOPENAI_API_KEY=new-secret\n"
        );
    }

    #[test]
    fn rejects_invalid_env_keys() {
        assert_eq!(
            validate_env_key(""),
            Err("Env var key cannot be empty".to_string())
        );
        assert_eq!(
            validate_env_key("BAD=KEY"),
            Err("Env var key cannot contain '='".to_string())
        );
        assert_eq!(
            validate_env_key("BAD KEY"),
            Err("Env var key must match [A-Za-z_][A-Za-z0-9_]*".to_string())
        );
        assert_eq!(
            validate_env_key("BAD\nKEY"),
            Err("Env var key must match [A-Za-z_][A-Za-z0-9_]*".to_string())
        );
        assert_eq!(
            validate_env_key("1BAD"),
            Err("Env var key must match [A-Za-z_][A-Za-z0-9_]*".to_string())
        );
    }

    #[test]
    fn parses_secure_env_request_message() {
        let request = parse_secure_env_request("set|demo-project|OPENAI_API_KEY|req-123").unwrap();

        assert_eq!(request.project_selector, "demo-project");
        assert_eq!(request.key, "OPENAI_API_KEY");
        assert_eq!(request.request_id, "req-123");
    }

    #[test]
    fn rejects_malformed_secure_env_request_messages() {
        assert_eq!(
            parse_secure_env_request("set|demo-project|OPENAI_API_KEY"),
            Err("Invalid secure env request message".to_string())
        );
        assert_eq!(
            parse_secure_env_request("get|demo-project|OPENAI_API_KEY|req-123"),
            Err("Unsupported secure env request action: get".to_string())
        );
        assert_eq!(
            parse_secure_env_request("set|demo-project|BAD=KEY|req-123"),
            Err("Env var key cannot contain '='".to_string())
        );
    }

    #[test]
    fn formats_success_secure_env_response() {
        let response = SecureEnvResponse {
            kind: SecureEnvResponseKind::Ok,
            status: "updated".to_string(),
            request_id: "req-123".to_string(),
        };

        assert_eq!(format_secure_env_response(&response), "ok|updated|req-123");
    }

    #[test]
    fn formats_error_secure_env_response() {
        let response = SecureEnvResponse {
            kind: SecureEnvResponseKind::Error,
            status: "busy".to_string(),
            request_id: "req-123".to_string(),
        };

        assert_eq!(format_secure_env_response(&response), "error|busy|req-123");
    }

    #[test]
    fn resolves_known_project_for_secure_env_request() {
        let tmp = TempDir::new().unwrap();
        let state = make_app_state(&tmp);
        let repo_path = tmp.path().join("demo-project");
        fs::create_dir_all(&repo_path).unwrap();
        let project = save_project(&state, "demo-project", repo_path.clone());

        let request =
            begin_secure_env_request(&state, "demo-project", "OPENAI_API_KEY", "req-123").unwrap();

        assert_eq!(request.project_id, project.id);
        assert_eq!(request.project_name, "demo-project");
        assert_eq!(request.env_path, repo_path.join(".env"));
        assert_eq!(request.key, "OPENAI_API_KEY");
    }

    #[test]
    fn rejects_unknown_project_for_secure_env_request() {
        let tmp = TempDir::new().unwrap();
        let state = make_app_state(&tmp);

        let error =
            begin_secure_env_request(&state, "missing-project", "OPENAI_API_KEY", "req-123")
                .unwrap_err();

        assert_eq!(error, "Unknown project: missing-project");
    }

    #[test]
    fn rejects_second_active_secure_env_request() {
        let tmp = TempDir::new().unwrap();
        let state = make_app_state(&tmp);
        let repo_path = tmp.path().join("demo-project");
        fs::create_dir_all(&repo_path).unwrap();
        save_project(&state, "demo-project", repo_path);

        begin_secure_env_request(&state, "demo-project", "OPENAI_API_KEY", "req-123").unwrap();

        let error =
            begin_secure_env_request(&state, "demo-project", "ANTHROPIC_API_KEY", "req-456")
                .unwrap_err();

        assert_eq!(error, "A secure env request is already active");
    }

    #[test]
    fn cancel_clears_active_secure_env_request() {
        let tmp = TempDir::new().unwrap();
        let state = make_app_state(&tmp);
        let repo_path = tmp.path().join("demo-project");
        fs::create_dir_all(&repo_path).unwrap();
        save_project(&state, "demo-project", repo_path);

        begin_secure_env_request(&state, "demo-project", "OPENAI_API_KEY", "req-123").unwrap();
        cancel_secure_env_request(&state, "req-123").unwrap();

        assert!(state.secure_env_request.lock().unwrap().is_none());
    }

    #[test]
    fn submit_writes_env_file_and_clears_active_request() {
        let tmp = TempDir::new().unwrap();
        let state = make_app_state(&tmp);
        let repo_path = tmp.path().join("demo-project");
        fs::create_dir_all(&repo_path).unwrap();
        save_project(&state, "demo-project", repo_path.clone());

        begin_secure_env_request(&state, "demo-project", "OPENAI_API_KEY", "req-123").unwrap();
        submit_secure_env_value(&state, "req-123", "new-secret").unwrap();

        let written = fs::read_to_string(repo_path.join(".env")).unwrap();
        assert_eq!(written, "OPENAI_API_KEY=new-secret\n");
        assert!(state.secure_env_request.lock().unwrap().is_none());
    }

    #[test]
    fn submit_write_failure_clears_active_request_and_notifies_cli() {
        let tmp = TempDir::new().unwrap();
        let state = make_app_state(&tmp);
        let repo_path = tmp.path().join("demo-project");
        fs::create_dir_all(repo_path.join(".env")).unwrap();
        save_project(&state, "demo-project", repo_path);

        let (tx, rx) = std::sync::mpsc::sync_channel(1);
        super::begin_secure_env_request_with_response(
            &state,
            "demo-project",
            "OPENAI_API_KEY",
            "req-123",
            Some(tx),
        )
        .unwrap();

        let error = match submit_secure_env_value(&state, "req-123", "new-secret") {
            Ok(_) => panic!("expected write failure"),
            Err(error) => error,
        };

        assert!(error.contains("failed to read"));
        assert!(state.secure_env_request.lock().unwrap().is_none());
        assert_eq!(
            rx.recv().unwrap(),
            SecureEnvResponse {
                kind: SecureEnvResponseKind::Error,
                status: "write-failed".to_string(),
                request_id: "req-123".to_string(),
            }
        );
    }
}
