use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum ConfigDefaultProvider {
    #[default]
    ClaudeCode,
    Codex,
    CursorAgent,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub projects_root: String,
    #[serde(default)]
    pub default_provider: ConfigDefaultProvider,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DirEntry {
    pub name: String,
    pub path: String,
}

/// Returns the path to config.json within the given base directory.
pub fn config_path(base_dir: &Path) -> PathBuf {
    base_dir.join("config.json")
}

/// Reads config.json from the base directory. Returns None if missing or invalid.
pub fn load_config(base_dir: &Path) -> Option<Config> {
    let path = config_path(base_dir);
    let json = fs::read_to_string(path).ok()?;
    serde_json::from_str(&json).ok()
}

/// Writes config.json as pretty JSON into the base directory.
pub fn save_config(base_dir: &Path, config: &Config) -> std::io::Result<()> {
    fs::create_dir_all(base_dir)?;
    let json = serde_json::to_string_pretty(config)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    fs::write(config_path(base_dir), json)
}

/// Checks if the claude CLI is installed and authenticated.
///
/// Returns one of:
/// - "not_installed" if `which claude` fails
/// - "authenticated" if `claude --print "say ok"` succeeds
/// - "not_authenticated" otherwise
pub fn check_claude_cli_status() -> String {
    check_claude_cli_status_with_binaries(Path::new("which"), Path::new("claude"))
}

fn check_claude_cli_status_with_binaries(which_bin: &Path, claude_bin: &Path) -> String {
    let which_result = Command::new(which_bin).arg("claude").output();

    match which_result {
        Ok(output) if output.status.success() => {}
        _ => return "not_installed".to_string(),
    }

    let auth_result = Command::new(claude_bin)
        .arg("--print")
        .arg("say ok")
        .env_remove("CLAUDECODE")
        .output();

    match auth_result {
        Ok(output) if output.status.success() => "authenticated".to_string(),
        _ => "not_authenticated".to_string(),
    }
}

/// Lists immediate child directories of `root`, filtering out hidden directories
/// (names starting with `.`) and non-directory entries. Results are sorted
/// alphabetically by name.
pub fn list_directories(root: &Path) -> std::io::Result<Vec<DirEntry>> {
    let mut entries = Vec::new();

    for entry in fs::read_dir(root)? {
        let entry = entry?;
        let metadata = entry.metadata()?;
        if !metadata.is_dir() {
            continue;
        }

        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with('.') {
            continue;
        }

        let path = entry.path().to_string_lossy().to_string();
        entries.push(DirEntry { name, path });
    }

    entries.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(entries)
}

/// Shells out to the `claude` CLI to generate short, hyphenated project
/// directory name suggestions based on a description.
pub fn generate_names_via_cli(description: &str) -> Result<Vec<String>, String> {
    let description = description.trim();
    if description.is_empty() {
        return Err("Description must not be empty".to_string());
    }
    if description.len() > 500 {
        return Err("Description is too long (max 500 characters)".to_string());
    }

    let prompt = format!(
        "Suggest 3 short, lowercase, hyphenated project directory names for: {}. Return only the 3 names, one per line, nothing else.",
        description
    );

    let output = Command::new("claude")
        .args(["--print", &prompt])
        .env_remove("CLAUDECODE")
        .output()
        .map_err(|e| format!("Failed to run claude CLI: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!(
            "claude CLI error (exit {}): {}",
            output.status.code().unwrap_or(-1),
            stderr.trim()
        ));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let names: Vec<String> = stdout
        .lines()
        .map(|l| l.trim().to_string())
        .filter(|l| {
            !l.is_empty()
                && l.chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
        })
        .take(3)
        .collect();

    if names.is_empty() {
        return Err("No valid names generated".to_string());
    }

    Ok(names)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    #[cfg(unix)]
    use std::os::unix::fs::PermissionsExt;
    use std::path::Path;
    use tempfile::TempDir;

    #[cfg(unix)]
    fn write_executable(path: &Path, body: &str) {
        fs::write(path, body).expect("write fake executable");
        let mut perms = fs::metadata(path)
            .expect("stat fake executable")
            .permissions();
        perms.set_mode(0o755);
        fs::set_permissions(path, perms).expect("chmod fake executable");
    }

    #[test]
    fn test_load_config_missing() {
        let tmp = TempDir::new().unwrap();
        let result = load_config(tmp.path());
        assert!(result.is_none());
    }

    #[test]
    fn test_save_and_load_config() {
        let tmp = TempDir::new().unwrap();
        let config = Config {
            projects_root: "/home/user/projects".to_string(),
            default_provider: ConfigDefaultProvider::ClaudeCode,
        };

        save_config(tmp.path(), &config).expect("save_config should succeed");
        let loaded = load_config(tmp.path()).expect("load_config should return Some");

        assert_eq!(loaded.projects_root, "/home/user/projects");
        assert_eq!(loaded.default_provider, ConfigDefaultProvider::ClaudeCode);
    }

    #[test]
    fn test_save_config_writes_default_provider_field() {
        let tmp = TempDir::new().unwrap();
        let config = Config {
            projects_root: "/home/user/projects".to_string(),
            default_provider: ConfigDefaultProvider::ClaudeCode,
        };

        save_config(tmp.path(), &config).expect("save_config should succeed");
        let json = fs::read_to_string(config_path(tmp.path())).expect("config.json should exist");

        assert!(json.contains(r#""default_provider": "claude-code""#));
    }

    #[test]
    fn test_save_and_load_config_with_codex_default_provider() {
        let tmp = TempDir::new().unwrap();
        let config = Config {
            projects_root: "/home/user/projects".to_string(),
            default_provider: ConfigDefaultProvider::Codex,
        };

        save_config(tmp.path(), &config).expect("save_config should succeed");
        let loaded = load_config(tmp.path()).expect("load_config should return Some");

        assert_eq!(loaded.default_provider, ConfigDefaultProvider::Codex);
    }

    #[cfg(unix)]
    #[test]
    fn test_check_claude_cli_reports_not_installed_when_which_fails() {
        let tmp = TempDir::new().unwrap();
        let which_bin = tmp.path().join("which");
        let claude_bin = tmp.path().join("claude");
        write_executable(&which_bin, "#!/bin/sh\nexit 1\n");
        write_executable(&claude_bin, "#!/bin/sh\nexit 0\n");

        let status = check_claude_cli_status_with_binaries(&which_bin, &claude_bin);

        assert_eq!(status, "not_installed");
    }

    #[cfg(unix)]
    #[test]
    fn test_check_claude_cli_reports_authenticated_when_claude_succeeds() {
        let tmp = TempDir::new().unwrap();
        let which_bin = tmp.path().join("which");
        let claude_bin = tmp.path().join("claude");
        write_executable(&which_bin, "#!/bin/sh\nexit 0\n");
        write_executable(&claude_bin, "#!/bin/sh\nexit 0\n");

        let status = check_claude_cli_status_with_binaries(&which_bin, &claude_bin);

        assert_eq!(status, "authenticated");
    }

    #[cfg(unix)]
    #[test]
    fn test_check_claude_cli_reports_not_authenticated_when_claude_fails() {
        let tmp = TempDir::new().unwrap();
        let which_bin = tmp.path().join("which");
        let claude_bin = tmp.path().join("claude");
        write_executable(&which_bin, "#!/bin/sh\nexit 0\n");
        write_executable(&claude_bin, "#!/bin/sh\nexit 7\n");

        let status = check_claude_cli_status_with_binaries(&which_bin, &claude_bin);

        assert_eq!(status, "not_authenticated");
    }

    #[test]
    fn test_list_directories() {
        let tmp = TempDir::new().unwrap();

        // Create 2 visible directories
        fs::create_dir(tmp.path().join("alpha")).unwrap();
        fs::create_dir(tmp.path().join("beta")).unwrap();

        // Create 1 hidden directory
        fs::create_dir(tmp.path().join(".hidden")).unwrap();

        // Create 1 regular file
        fs::write(tmp.path().join("file.txt"), "hello").unwrap();

        let dirs = list_directories(tmp.path()).expect("list_directories should succeed");

        assert_eq!(dirs.len(), 2);
        assert_eq!(dirs[0].name, "alpha");
        assert_eq!(dirs[1].name, "beta");
    }

    #[test]
    fn test_config_path_returns_expected() {
        let base = Path::new("/tmp/test-base");
        assert_eq!(
            config_path(base),
            PathBuf::from("/tmp/test-base/config.json")
        );
    }

    #[test]
    fn test_load_config_invalid_json() {
        let tmp = TempDir::new().unwrap();
        fs::write(config_path(tmp.path()), "not valid json").unwrap();
        let result = load_config(tmp.path());
        assert!(result.is_none());
    }

    #[test]
    fn test_load_config_without_default_provider_uses_claude_code() {
        let tmp = TempDir::new().unwrap();
        fs::write(
            config_path(tmp.path()),
            r#"{"projects_root":"/home/user/projects"}"#,
        )
        .unwrap();

        let result = load_config(tmp.path()).expect("load_config should return Some");

        assert_eq!(result.default_provider, ConfigDefaultProvider::ClaudeCode);
    }

    #[test]
    fn test_list_directories_empty_dir() {
        let tmp = TempDir::new().unwrap();
        let dirs = list_directories(tmp.path()).unwrap();
        assert!(dirs.is_empty());
    }

    #[test]
    fn test_generate_names_via_cli_empty_description() {
        let result = generate_names_via_cli("");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("must not be empty"));
    }

    #[test]
    fn test_generate_names_via_cli_whitespace_only() {
        let result = generate_names_via_cli("   ");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("must not be empty"));
    }

    #[test]
    fn test_generate_names_via_cli_too_long() {
        let long = "a".repeat(501);
        let result = generate_names_via_cli(&long);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("too long"));
    }
}
