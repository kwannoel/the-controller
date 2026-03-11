#[cfg(test)]
use std::cell::RefCell;
use std::fs;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::Command;
use uuid::Uuid;

const TMUX_CANDIDATES: [&str; 3] = ["/opt/homebrew/bin/tmux", "/usr/local/bin/tmux", "tmux"];
const SESSION_PREFIX: &str = "ctrl-";

pub struct TmuxManager;

impl TmuxManager {
    /// Check whether the tmux binary is available on this system.
    pub fn is_available() -> bool {
        Self::tmux_binary().is_some()
    }

    pub fn tmux_binary() -> Option<String> {
        #[cfg(test)]
        if let Some(binary) = test_tmux_binary_override(|override_bin| override_bin.clone()) {
            return Some(binary);
        }

        resolve_tmux_binary()
    }

    pub fn session_name(session_id: Uuid) -> String {
        format!("{}{}", SESSION_PREFIX, session_id)
    }

    pub fn has_session(session_id: Uuid) -> bool {
        let Some(tmux_bin) = Self::tmux_binary() else {
            return false;
        };
        let name = Self::session_name(session_id);
        Command::new(&tmux_bin)
            .args(["has-session", "-t", &name])
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }

    /// Build the argument list for `tmux new-session`.
    /// Extracted for testability.
    fn build_create_args(
        session_id: Uuid,
        working_dir: &str,
        command: &str,
        continue_session: bool,
        initial_prompt: Option<&str>,
    ) -> Vec<String> {
        let name = Self::session_name(session_id);
        let mut args = vec![
            "new-session".to_string(),
            "-d".to_string(),
            "-s".to_string(),
            name,
            "-c".to_string(),
            working_dir.to_string(),
            "-x".to_string(),
            "80".to_string(),
            "-y".to_string(),
            "24".to_string(),
            "-e".to_string(),
            format!("THE_CONTROLLER_SESSION_ID={}", session_id),
        ];
        // Prepend ~/.the-controller/bin to PATH so controller-cli is available
        if let Some(path_val) = crate::cli_install::path_with_controller_bin() {
            args.push("-e".to_string());
            args.push(format!("PATH={}", path_val));
        }
        args.push(command.to_string());
        args.extend(crate::session_args::build_session_args(
            command,
            session_id,
            continue_session,
            initial_prompt,
        ));
        args
    }

    pub fn create_session(
        session_id: Uuid,
        working_dir: &str,
        command: &str,
        continue_session: bool,
        initial_prompt: Option<&str>,
    ) -> Result<(), String> {
        let args = Self::build_create_args(
            session_id,
            working_dir,
            command,
            continue_session,
            initial_prompt,
        );
        let name = Self::session_name(session_id);
        let tmux_bin = Self::tmux_binary().ok_or_else(|| "tmux binary not found".to_string())?;
        let output = Command::new(&tmux_bin)
            .args(&args)
            .env("THE_CONTROLLER_SESSION_ID", session_id.to_string())
            .env_remove("CLAUDECODE")
            .output()
            .map_err(|e| format!("failed to run tmux: {}", e))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(format!("tmux new-session failed: {}", stderr.trim()));
        }

        // Enable extended keys so modifier combos (e.g. Shift+Enter) pass through.
        // Use csi-u format (kitty keyboard protocol) so Claude Code's crossterm can parse them.
        let _ = Command::new(&tmux_bin)
            .args(["set-option", "-t", &name, "extended-keys", "always"])
            .output();
        let _ = Command::new(&tmux_bin)
            .args(["set-option", "-t", &name, "extended-keys-format", "csi-u"])
            .output();

        Ok(())
    }

    /// Send raw bytes to a tmux pane using `send-keys -H`, bypassing tmux's
    /// outer terminal input parser. Used for escape sequences (e.g. CSI u for
    /// Shift+Enter) that tmux wouldn't recognise from the outer PTY.
    pub fn send_keys_hex(session_id: Uuid, data: &[u8]) -> Result<(), String> {
        let name = Self::session_name(session_id);
        let tmux_bin = Self::tmux_binary().ok_or_else(|| "tmux binary not found".to_string())?;
        let hex_bytes: Vec<String> = data.iter().map(|b| format!("{:02x}", b)).collect();
        let mut args = vec![
            "send-keys".to_string(),
            "-H".to_string(),
            "-t".to_string(),
            name,
        ];
        args.extend(hex_bytes);

        let output = Command::new(&tmux_bin)
            .args(&args)
            .output()
            .map_err(|e| format!("failed to run tmux send-keys: {}", e))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(format!("tmux send-keys failed: {}", stderr.trim()));
        }

        Ok(())
    }

    pub fn kill_session(session_id: Uuid) -> Result<(), String> {
        let name = Self::session_name(session_id);
        let Some(tmux_bin) = Self::tmux_binary() else {
            return Ok(());
        };
        let output = Command::new(&tmux_bin)
            .args(["kill-session", "-t", &name])
            .output()
            .map_err(|e| format!("failed to run tmux: {}", e))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            if !stderr.contains("no server running")
                && !stderr.contains("error connecting to")
                && !stderr.contains("session not found")
                && !stderr.contains("can't find session")
            {
                return Err(format!("tmux kill-session failed: {}", stderr.trim()));
            }
        }

        Ok(())
    }

    /// Query the current window dimensions of a tmux session.
    /// Returns `(cols, rows)` or `None` if the query fails.
    pub fn session_size(session_id: Uuid) -> Option<(u16, u16)> {
        let name = Self::session_name(session_id);
        let tmux_bin = Self::tmux_binary()?;
        let output = Command::new(&tmux_bin)
            .args([
                "display-message",
                "-t",
                &name,
                "-p",
                "#{window_width} #{window_height}",
            ])
            .output()
            .ok()?;
        if !output.status.success() {
            return None;
        }
        let text = String::from_utf8_lossy(&output.stdout);
        let parts: Vec<&str> = text.trim().split_whitespace().collect();
        if parts.len() == 2 {
            let cols = parts[0].parse::<u16>().ok()?;
            let rows = parts[1].parse::<u16>().ok()?;
            Some((cols, rows))
        } else {
            None
        }
    }

    pub fn resize_session(session_id: Uuid, cols: u16, rows: u16) -> Result<(), String> {
        let name = Self::session_name(session_id);
        let tmux_bin = Self::tmux_binary().ok_or_else(|| "tmux binary not found".to_string())?;
        let output = Command::new(&tmux_bin)
            .args([
                "resize-window",
                "-t",
                &name,
                "-x",
                &cols.to_string(),
                "-y",
                &rows.to_string(),
            ])
            .output()
            .map_err(|e| format!("failed to run tmux: {}", e))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(format!("tmux resize-window failed: {}", stderr.trim()));
        }

        Ok(())
    }
}

fn resolve_tmux_binary() -> Option<String> {
    resolve_tmux_binary_with(|candidate| {
        if candidate.contains('/') {
            is_executable_file(Path::new(candidate)).then(|| candidate.to_string())
        } else {
            resolve_tmux_on_path(candidate)
        }
    })
}

fn resolve_tmux_binary_with<F>(mut resolve_candidate: F) -> Option<String>
where
    F: FnMut(&str) -> Option<String>,
{
    for candidate in TMUX_CANDIDATES {
        if let Some(resolved) = resolve_candidate(candidate) {
            return Some(resolved);
        }
    }

    None
}

fn resolve_tmux_on_path(binary_name: &str) -> Option<String> {
    let path = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path) {
        let candidate = dir.join(binary_name);
        if is_executable_file(&candidate) {
            return Some(candidate.to_string_lossy().into_owned());
        }
    }

    None
}

fn is_executable_file(path: &Path) -> bool {
    let Ok(metadata) = fs::metadata(path) else {
        return false;
    };
    if !metadata.is_file() {
        return false;
    }

    #[cfg(unix)]
    {
        metadata.permissions().mode() & 0o111 != 0
    }

    #[cfg(not(unix))]
    {
        true
    }
}

#[cfg(test)]
thread_local! {
    static TMUX_BINARY_OVERRIDE: RefCell<Option<String>> = const { RefCell::new(None) };
}

#[cfg(test)]
fn test_tmux_binary_override<R>(f: impl FnOnce(&mut Option<String>) -> R) -> R {
    TMUX_BINARY_OVERRIDE.with(|override_bin| f(&mut override_bin.borrow_mut()))
}

#[cfg(test)]
pub(crate) struct TestTmuxBinaryGuard {
    previous: Option<String>,
}

#[cfg(test)]
impl Drop for TestTmuxBinaryGuard {
    fn drop(&mut self) {
        test_tmux_binary_override(|override_bin| *override_bin = self.previous.take());
    }
}

#[cfg(test)]
pub(crate) fn set_test_tmux_binary(binary: Option<&str>) -> TestTmuxBinaryGuard {
    let previous = test_tmux_binary_override(|override_bin| {
        let previous = override_bin.clone();
        *override_bin = binary.map(str::to_string);
        previous
    });
    TestTmuxBinaryGuard { previous }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_available_returns_bool() {
        // Just verifies it doesn't panic; result depends on system
        let _ = TmuxManager::is_available();
    }

    #[test]
    fn test_session_name_format() {
        let id = Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap();
        assert_eq!(
            TmuxManager::session_name(id),
            "ctrl-550e8400-e29b-41d4-a716-446655440000"
        );
    }

    #[test]
    fn test_create_args_passes_env_via_tmux_e_flag() {
        // The -e flag ensures THE_CONTROLLER_SESSION_ID is set inside the tmux
        // session, not just on the tmux client process. Without -e, the env var
        // doesn't propagate when the tmux server is already running.
        let id = Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap();
        let args = TmuxManager::build_create_args(id, "/tmp", "claude", false, None);

        let e_idx = args
            .iter()
            .position(|a| a == "-e")
            .expect("-e flag must be present in tmux new-session args");
        let env_val = &args[e_idx + 1];
        assert_eq!(
            env_val,
            &format!("THE_CONTROLLER_SESSION_ID={}", id),
            "-e must be followed by THE_CONTROLLER_SESSION_ID=<uuid>"
        );

        // -e must appear before the shell command (which is the first
        // positional arg after all flags)
        let cmd_idx = args.iter().position(|a| a == "claude").unwrap();
        assert!(
            e_idx < cmd_idx,
            "-e flag must come before the shell command"
        );
    }

    #[test]
    fn test_create_args_prepends_controller_bin_to_path() {
        let id = Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap();
        let args = TmuxManager::build_create_args(id, "/tmp", "claude", false, None);

        // Find the PATH -e flag (second -e, after THE_CONTROLLER_SESSION_ID)
        let e_positions: Vec<usize> = args
            .iter()
            .enumerate()
            .filter(|(_, a)| *a == "-e")
            .map(|(i, _)| i)
            .collect();

        // Should have at least 2 -e flags (session ID + PATH)
        assert!(
            e_positions.len() >= 2,
            "expected at least 2 -e flags, got {}",
            e_positions.len()
        );

        let path_e_idx = e_positions[1];
        let path_val = &args[path_e_idx + 1];
        assert!(
            path_val.starts_with("PATH="),
            "second -e should set PATH, got: {}",
            path_val
        );
        assert!(
            path_val.contains(".the-controller/bin"),
            "PATH should contain .the-controller/bin, got: {}",
            path_val
        );

        // PATH -e must come before the shell command
        let cmd_idx = args.iter().position(|a| a == "claude").unwrap();
        assert!(
            path_e_idx < cmd_idx,
            "PATH -e flag must come before the shell command"
        );
    }

    #[test]
    fn test_has_session_returns_false_for_nonexistent() {
        let id = Uuid::new_v4();
        assert!(!TmuxManager::has_session(id));
    }

    #[test]
    fn test_session_size_returns_none_for_nonexistent() {
        let id = Uuid::new_v4();
        assert!(TmuxManager::session_size(id).is_none());
    }

    #[test]
    fn test_kill_nonexistent_session_is_not_error() {
        let id = Uuid::new_v4();
        assert!(TmuxManager::kill_session(id).is_ok());
    }

    #[test]
    fn test_resolve_tmux_binary_checks_usr_local_homebrew_path() {
        let resolved = resolve_tmux_binary_with(|candidate| match candidate {
            "/usr/local/bin/tmux" => Some(candidate.to_string()),
            _ => None,
        });

        assert_eq!(resolved.as_deref(), Some("/usr/local/bin/tmux"));
    }

    #[test]
    fn test_resolve_tmux_binary_falls_back_to_path_lookup() {
        let resolved = resolve_tmux_binary_with(|candidate| match candidate {
            "tmux" => Some("/tmp/test-bin/tmux".to_string()),
            _ => None,
        });

        assert_eq!(resolved.as_deref(), Some("/tmp/test-bin/tmux"));
    }
}
