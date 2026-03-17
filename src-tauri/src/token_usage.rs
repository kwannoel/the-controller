use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenDataPoint {
    pub timestamp: String,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_read_tokens: u64,
    pub cache_write_tokens: u64,
}

/// Get token usage data for a session by reading JSONL files from disk.
pub fn get_token_usage(working_dir: &str, kind: &str) -> Result<Vec<TokenDataPoint>, String> {
    match kind {
        "claude" => get_claude_token_usage(working_dir),
        "codex" => get_codex_token_usage(working_dir),
        "cursor-agent" => Err("Token usage tracking is not yet supported for cursor-agent".into()),
        _ => Err(format!("Unknown session kind: {}", kind)),
    }
}

// ---------------------------------------------------------------------------
// Claude Code
// ---------------------------------------------------------------------------

fn get_claude_token_usage(working_dir: &str) -> Result<Vec<TokenDataPoint>, String> {
    let project_dir = claude_project_dir(working_dir)?;
    let jsonl_path = most_recent_jsonl(&project_dir)?;
    parse_claude_jsonl(&jsonl_path)
}

/// Derive the Claude Code project directory from a working directory path.
/// Claude encodes the absolute path: `/` → `-`, `.` → `-`.
fn claude_project_dir(working_dir: &str) -> Result<PathBuf, String> {
    let home = dirs::home_dir().ok_or("Cannot determine home directory")?;
    let claude_projects = home.join(".claude").join("projects");
    if !claude_projects.exists() {
        return Err("~/.claude/projects/ does not exist".into());
    }

    // Claude Code encodes the working directory path by replacing `/` and `.` with `-`.
    let encoded = working_dir.replace(['/', '.'], "-");

    // Try exact match first
    let candidate = claude_projects.join(&encoded);
    if candidate.is_dir() {
        return Ok(candidate);
    }

    // Fallback: scan for directories that contain the working_dir path as a suffix.
    // This handles slight encoding differences between Claude versions.
    let entries = fs::read_dir(&claude_projects).map_err(|e| e.to_string())?;
    let mut best: Option<PathBuf> = None;
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        if (name.contains(&encoded) || encoded.contains(&name))
            && entry.file_type().map(|t| t.is_dir()).unwrap_or(false)
        {
            best = Some(entry.path());
        }
    }

    best.ok_or_else(|| format!("No Claude project directory found for {}", working_dir))
}

/// Find the most recently modified `.jsonl` file in a directory.
fn most_recent_jsonl(dir: &Path) -> Result<PathBuf, String> {
    let entries = fs::read_dir(dir).map_err(|e| e.to_string())?;
    let mut best: Option<(PathBuf, std::time::SystemTime)> = None;

    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) == Some("jsonl") {
            if let Ok(meta) = fs::metadata(&path) {
                let modified = meta.modified().unwrap_or(std::time::UNIX_EPOCH);
                if best.as_ref().is_none_or(|(_, t)| modified > *t) {
                    best = Some((path, modified));
                }
            }
        }
    }

    best.map(|(p, _)| p)
        .ok_or_else(|| format!("No .jsonl files found in {}", dir.display()))
}

/// Parse a Claude Code JSONL file for token usage data.
/// Looks for entries with `type: "assistant"` that have `message.usage`.
fn parse_claude_jsonl(path: &Path) -> Result<Vec<TokenDataPoint>, String> {
    let content = fs::read_to_string(path).map_err(|e| e.to_string())?;
    let mut points = Vec::new();

    for line in content.lines() {
        if line.is_empty() {
            continue;
        }
        let v: serde_json::Value = match serde_json::from_str(line) {
            Ok(v) => v,
            Err(_) => continue,
        };

        if v.get("type").and_then(|t| t.as_str()) != Some("assistant") {
            continue;
        }

        let usage = match v.pointer("/message/usage") {
            Some(u) => u,
            None => continue,
        };

        let input = usage
            .get("input_tokens")
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        let output = usage
            .get("output_tokens")
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        let cache_read = usage
            .get("cache_read_input_tokens")
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        let cache_write = usage
            .get("cache_creation_input_tokens")
            .and_then(|v| v.as_u64())
            .unwrap_or(0);

        let timestamp = v
            .get("timestamp")
            .and_then(|t| t.as_str())
            .unwrap_or("")
            .to_string();

        points.push(TokenDataPoint {
            timestamp,
            input_tokens: input,
            output_tokens: output,
            cache_read_tokens: cache_read,
            cache_write_tokens: cache_write,
        });
    }

    Ok(points)
}

// ---------------------------------------------------------------------------
// Codex
// ---------------------------------------------------------------------------

fn get_codex_token_usage(working_dir: &str) -> Result<Vec<TokenDataPoint>, String> {
    let home = dirs::home_dir().ok_or("Cannot determine home directory")?;
    let codex_sessions = home.join(".codex").join("sessions");
    if !codex_sessions.exists() {
        return Err("~/.codex/sessions/ does not exist".into());
    }

    // Find the JSONL file whose session_meta.payload.cwd matches working_dir.
    let jsonl_path = find_codex_session_file(&codex_sessions, working_dir)?;
    parse_codex_jsonl(&jsonl_path)
}

/// Walk the `~/.codex/sessions/YYYY/MM/DD/` tree and find the most recent file
/// whose `session_meta` entry has a matching `cwd`.
fn find_codex_session_file(sessions_dir: &Path, working_dir: &str) -> Result<PathBuf, String> {
    let mut candidates: Vec<(PathBuf, std::time::SystemTime)> = Vec::new();

    // Walk year/month/day directories
    for year in read_dir_sorted_desc(sessions_dir) {
        for month in read_dir_sorted_desc(&year) {
            for day in read_dir_sorted_desc(&month) {
                for entry in fs::read_dir(&day).into_iter().flatten().flatten() {
                    let path = entry.path();
                    if path.extension().and_then(|e| e.to_str()) == Some("jsonl") {
                        if let Ok(meta) = fs::metadata(&path) {
                            let modified = meta.modified().unwrap_or(std::time::UNIX_EPOCH);
                            candidates.push((path, modified));
                        }
                    }
                }
            }
        }
    }

    // Sort most recent first, check only recent files to avoid scanning everything.
    candidates.sort_by(|a, b| b.1.cmp(&a.1));

    for (path, _) in candidates.iter().take(50) {
        if codex_session_matches_cwd(path, working_dir) {
            return Ok(path.clone());
        }
    }

    Err(format!(
        "No Codex session file found for working dir: {}",
        working_dir
    ))
}

fn read_dir_sorted_desc(dir: &Path) -> Vec<PathBuf> {
    let mut entries: Vec<PathBuf> = fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_dir())
        .collect();
    entries.sort();
    entries.reverse();
    entries
}

/// Check if a Codex JSONL file's session_meta has a matching cwd.
fn codex_session_matches_cwd(path: &Path, working_dir: &str) -> bool {
    let file = match fs::File::open(path) {
        Ok(f) => f,
        Err(_) => return false,
    };
    let reader = std::io::BufReader::new(file);
    use std::io::BufRead;

    // Only need to check the first few lines for session_meta
    for line in reader.lines().take(5) {
        let line = match line {
            Ok(l) => l,
            Err(_) => continue,
        };
        let v: serde_json::Value = match serde_json::from_str(&line) {
            Ok(v) => v,
            Err(_) => continue,
        };
        if v.get("type").and_then(|t| t.as_str()) == Some("session_meta") {
            if let Some(cwd) = v.pointer("/payload/cwd").and_then(|c| c.as_str()) {
                return cwd == working_dir;
            }
        }
    }
    false
}

/// Parse a Codex JSONL file for token usage data.
/// Looks for entries with `type: "event_msg"` and `payload.type: "token_count"`.
/// Uses `last_token_usage` for per-turn deltas.
fn parse_codex_jsonl(path: &Path) -> Result<Vec<TokenDataPoint>, String> {
    let content = fs::read_to_string(path).map_err(|e| e.to_string())?;
    let mut points = Vec::new();
    let mut prev_input: u64 = 0;
    let mut prev_output: u64 = 0;

    for line in content.lines() {
        if line.is_empty() {
            continue;
        }
        let v: serde_json::Value = match serde_json::from_str(line) {
            Ok(v) => v,
            Err(_) => continue,
        };

        if v.get("type").and_then(|t| t.as_str()) != Some("event_msg") {
            continue;
        }
        if v.pointer("/payload/type").and_then(|t| t.as_str()) != Some("token_count") {
            continue;
        }

        let usage = match v.pointer("/payload/info/total_token_usage") {
            Some(u) => u,
            None => continue,
        };

        let total_input = usage
            .get("input_tokens")
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        let total_output = usage
            .get("output_tokens")
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        let cached = usage
            .get("cached_input_tokens")
            .and_then(|v| v.as_u64())
            .unwrap_or(0);

        // Compute per-turn delta from running totals
        let delta_input = total_input.saturating_sub(prev_input);
        let delta_output = total_output.saturating_sub(prev_output);
        prev_input = total_input;
        prev_output = total_output;

        let timestamp = v
            .get("timestamp")
            .and_then(|t| t.as_str())
            .unwrap_or("")
            .to_string();

        points.push(TokenDataPoint {
            timestamp,
            input_tokens: delta_input,
            output_tokens: delta_output,
            cache_read_tokens: cached,
            cache_write_tokens: 0,
        });
    }

    Ok(points)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::TempDir;

    #[test]
    fn test_claude_project_dir_encoding() {
        // The encoding replaces `/` and `.` with `-`
        let path = "/Users/noel/.the-controller/worktrees/foo/session-1";
        let encoded = path.replace(['/', '.'], "-");
        assert_eq!(
            encoded,
            "-Users-noel--the-controller-worktrees-foo-session-1"
        );
    }

    #[test]
    fn test_parse_claude_jsonl() {
        let dir = TempDir::new().unwrap();
        let jsonl_path = dir.path().join("test.jsonl");
        let mut f = fs::File::create(&jsonl_path).unwrap();
        // Write a non-assistant entry (should be ignored)
        writeln!(
            f,
            r#"{{"type":"progress","timestamp":"2026-01-01T00:00:00Z"}}"#
        )
        .unwrap();
        // Write an assistant entry with usage
        writeln!(
            f,
            r#"{{"type":"assistant","timestamp":"2026-01-01T00:01:00Z","message":{{"usage":{{"input_tokens":100,"output_tokens":50,"cache_read_input_tokens":20,"cache_creation_input_tokens":10}}}}}}"#
        )
        .unwrap();
        // Write another assistant entry
        writeln!(
            f,
            r#"{{"type":"assistant","timestamp":"2026-01-01T00:02:00Z","message":{{"usage":{{"input_tokens":200,"output_tokens":80}}}}}}"#
        )
        .unwrap();

        let points = parse_claude_jsonl(&jsonl_path).unwrap();
        assert_eq!(points.len(), 2);
        assert_eq!(points[0].input_tokens, 100);
        assert_eq!(points[0].output_tokens, 50);
        assert_eq!(points[0].cache_read_tokens, 20);
        assert_eq!(points[0].cache_write_tokens, 10);
        assert_eq!(points[1].input_tokens, 200);
        assert_eq!(points[1].output_tokens, 80);
        assert_eq!(points[1].cache_read_tokens, 0);
        assert_eq!(points[1].cache_write_tokens, 0);
    }

    #[test]
    fn test_parse_codex_jsonl() {
        let dir = TempDir::new().unwrap();
        let jsonl_path = dir.path().join("test.jsonl");
        let mut f = fs::File::create(&jsonl_path).unwrap();
        // First token_count event
        writeln!(
            f,
            r#"{{"type":"event_msg","timestamp":"2026-01-01T00:01:00Z","payload":{{"type":"token_count","info":{{"total_token_usage":{{"input_tokens":100,"output_tokens":50,"cached_input_tokens":30,"total_tokens":150}}}}}}}}"#
        )
        .unwrap();
        // Second token_count event (cumulative)
        writeln!(
            f,
            r#"{{"type":"event_msg","timestamp":"2026-01-01T00:02:00Z","payload":{{"type":"token_count","info":{{"total_token_usage":{{"input_tokens":300,"output_tokens":120,"cached_input_tokens":80,"total_tokens":420}}}}}}}}"#
        )
        .unwrap();

        let points = parse_codex_jsonl(&jsonl_path).unwrap();
        assert_eq!(points.len(), 2);
        // First turn: delta = total since prev is 0
        assert_eq!(points[0].input_tokens, 100);
        assert_eq!(points[0].output_tokens, 50);
        // Second turn: delta from first
        assert_eq!(points[1].input_tokens, 200);
        assert_eq!(points[1].output_tokens, 70);
    }

    #[test]
    fn test_most_recent_jsonl() {
        let dir = TempDir::new().unwrap();
        // Create two jsonl files with different modification times
        let old = dir.path().join("old.jsonl");
        fs::write(&old, "").unwrap();
        // Sleep briefly so modification times differ
        std::thread::sleep(std::time::Duration::from_millis(50));
        let new = dir.path().join("new.jsonl");
        fs::write(&new, "").unwrap();

        let result = most_recent_jsonl(dir.path()).unwrap();
        assert_eq!(result.file_name().unwrap(), "new.jsonl");
    }

    #[test]
    fn test_empty_jsonl_returns_empty_points() {
        let dir = TempDir::new().unwrap();
        let jsonl_path = dir.path().join("empty.jsonl");
        fs::write(&jsonl_path, "").unwrap();
        let points = parse_claude_jsonl(&jsonl_path).unwrap();
        assert!(points.is_empty());
    }

    #[test]
    fn test_codex_session_matches_cwd() {
        let dir = TempDir::new().unwrap();
        let jsonl_path = dir.path().join("test.jsonl");
        let mut f = fs::File::create(&jsonl_path).unwrap();
        writeln!(
            f,
            r#"{{"type":"session_meta","payload":{{"cwd":"/Users/noel/project"}}}}"#
        )
        .unwrap();

        assert!(codex_session_matches_cwd(
            &jsonl_path,
            "/Users/noel/project"
        ));
        assert!(!codex_session_matches_cwd(&jsonl_path, "/Users/noel/other"));
    }
}
