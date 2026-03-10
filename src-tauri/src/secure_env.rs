use std::fs;
use std::path::Path;

#[allow(dead_code)]
pub(crate) struct EnvWriteResult {
    pub(crate) created: bool,
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
pub(crate) enum SecureEnvResponseKind {
    Ok,
    Error,
}

#[allow(dead_code)]
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct SecureEnvResponse {
    pub(crate) kind: SecureEnvResponseKind,
    pub(crate) status: String,
    pub(crate) request_id: String,
}

#[allow(dead_code)]
pub(crate) fn validate_env_key(key: &str) -> Result<(), String> {
    if key.is_empty() {
        return Err("Env var key cannot be empty".to_string());
    }
    if key.contains('=') {
        return Err("Env var key cannot contain '='".to_string());
    }
    Ok(())
}

#[allow(dead_code)]
pub(crate) fn parse_secure_env_request(message: &str) -> Result<SecureEnvRequest, String> {
    let mut parts = message.split('|');
    let action = parts.next().ok_or_else(|| "Invalid secure env request message".to_string())?;
    let project_selector = parts.next().ok_or_else(|| "Invalid secure env request message".to_string())?;
    let key = parts.next().ok_or_else(|| "Invalid secure env request message".to_string())?;
    let request_id = parts.next().ok_or_else(|| "Invalid secure env request message".to_string())?;

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
pub(crate) fn update_env_file(
    env_path: &Path,
    key: &str,
    value: &str,
) -> Result<EnvWriteResult, String> {
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

    use tempfile::TempDir;

    use super::{
        format_secure_env_response, parse_secure_env_request, update_env_file,
        validate_env_key, SecureEnvResponse, SecureEnvResponseKind,
    };

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
        assert_eq!(validate_env_key(""), Err("Env var key cannot be empty".to_string()));
        assert_eq!(
            validate_env_key("BAD=KEY"),
            Err("Env var key cannot contain '='".to_string())
        );
    }

    #[test]
    fn parses_secure_env_request_message() {
        let request =
            parse_secure_env_request("set|demo-project|OPENAI_API_KEY|req-123").unwrap();

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
}
