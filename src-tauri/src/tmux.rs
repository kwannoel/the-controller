use std::process::Command;
use uuid::Uuid;

const TMUX_BIN: &str = "/opt/homebrew/bin/tmux";
const SESSION_PREFIX: &str = "ctrl-";

pub struct TmuxManager;

impl TmuxManager {
    /// Check whether the tmux binary is available on this system.
    pub fn is_available() -> bool {
        std::path::Path::new(TMUX_BIN).exists()
    }

    pub fn session_name(session_id: Uuid) -> String {
        format!("{}{}", SESSION_PREFIX, session_id)
    }

    pub fn has_session(session_id: Uuid) -> bool {
        let name = Self::session_name(session_id);
        Command::new(TMUX_BIN)
            .args(["has-session", "-t", &name])
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }

    pub fn create_session(
        session_id: Uuid,
        working_dir: &str,
        command: &str,
        continue_session: bool,
        initial_prompt: Option<&str>,
    ) -> Result<(), String> {
        let name = Self::session_name(session_id);
        let settings_json = crate::status_socket::hook_settings_json(session_id);
        let mut args = vec![
            "new-session", "-d", "-s", &name, "-c", working_dir, "-x", "80", "-y", "24", command,
        ];
        if continue_session {
            args.push("--continue");
        }
        if command == "claude" {
            args.push("--settings");
            args.push(&settings_json);
            if let Some(prompt) = initial_prompt {
                args.push("--prompt");
                args.push(prompt);
            }
        }
        let output = Command::new(TMUX_BIN)
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
        let _ = Command::new(TMUX_BIN)
            .args(["set-option", "-t", &name, "extended-keys", "always"])
            .output();
        let _ = Command::new(TMUX_BIN)
            .args(["set-option", "-t", &name, "extended-keys-format", "csi-u"])
            .output();

        Ok(())
    }

    /// Send raw bytes to a tmux pane using `send-keys -H`, bypassing tmux's
    /// outer terminal input parser. Used for escape sequences (e.g. CSI u for
    /// Shift+Enter) that tmux wouldn't recognise from the outer PTY.
    pub fn send_keys_hex(session_id: Uuid, data: &[u8]) -> Result<(), String> {
        let name = Self::session_name(session_id);
        let hex_bytes: Vec<String> = data.iter().map(|b| format!("{:02x}", b)).collect();
        let mut args = vec![
            "send-keys".to_string(),
            "-H".to_string(),
            "-t".to_string(),
            name,
        ];
        args.extend(hex_bytes);

        let output = Command::new(TMUX_BIN)
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
        let output = Command::new(TMUX_BIN)
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

    pub fn resize_session(session_id: Uuid, cols: u16, rows: u16) -> Result<(), String> {
        let name = Self::session_name(session_id);
        let output = Command::new(TMUX_BIN)
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
    fn test_has_session_returns_false_for_nonexistent() {
        let id = Uuid::new_v4();
        assert!(!TmuxManager::has_session(id));
    }

    #[test]
    fn test_kill_nonexistent_session_is_not_error() {
        let id = Uuid::new_v4();
        assert!(TmuxManager::kill_session(id).is_ok());
    }
}
