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
            command.to_string(),
        ];
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
        let args = Self::build_create_args(session_id, working_dir, command, continue_session, initial_prompt);
        let name = Self::session_name(session_id);
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

        // Enable mouse passthrough so wheel/scroll events reach the inner
        // application (Claude Code) instead of being swallowed by tmux.
        let _ = Command::new(TMUX_BIN)
            .args(["set-option", "-t", &name, "mouse", "on"])
            .output();

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

    /// Ensure mouse passthrough is enabled on a tmux session.
    /// Idempotent — safe to call on sessions that already have it set.
    pub fn ensure_mouse_on(session_id: Uuid) -> Result<(), String> {
        let name = Self::session_name(session_id);
        let _ = Command::new(TMUX_BIN)
            .args(["set-option", "-t", &name, "mouse", "on"])
            .output();
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

    /// Query the current window dimensions of a tmux session.
    /// Returns `(cols, rows)` or `None` if the query fails.
    pub fn session_size(session_id: Uuid) -> Option<(u16, u16)> {
        let name = Self::session_name(session_id);
        let output = Command::new(TMUX_BIN)
            .args(["display-message", "-t", &name, "-p", "#{window_width} #{window_height}"])
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
    fn test_create_args_passes_env_via_tmux_e_flag() {
        // The -e flag ensures THE_CONTROLLER_SESSION_ID is set inside the tmux
        // session, not just on the tmux client process. Without -e, the env var
        // doesn't propagate when the tmux server is already running.
        let id = Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap();
        let args = TmuxManager::build_create_args(id, "/tmp", "claude", false, None);

        let e_idx = args.iter().position(|a| a == "-e")
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
        assert!(e_idx < cmd_idx, "-e flag must come before the shell command");
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
}
