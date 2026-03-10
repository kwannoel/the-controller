use std::fs;
use std::path::Path;

#[allow(dead_code)]
pub(crate) struct EnvWriteResult {
    pub(crate) created: bool,
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

    use super::{update_env_file, validate_env_key};

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
}
