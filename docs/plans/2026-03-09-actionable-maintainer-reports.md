# Actionable Maintainer Reports Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use the-controller-executing-plans to implement this plan task-by-task.

**Goal:** Transform the maintainer from a report-generating agent into an issue-filing agent that creates/updates GitHub issues and produces lightweight run logs.

**Architecture:** The Claude subprocess prompt is rewritten to include `gh` CLI commands for searching/filing/updating GitHub issues. The Rust backend switches from parsing structured findings to parsing an action summary (issues filed/updated). Storage changes from `MaintainerReport` with findings to `MaintainerRunLog` with issue summaries. Frontend simplifies from finding detail views to a run log list with GitHub issue links.

**Tech Stack:** Rust (Tauri v2), Svelte 5, `claude --print` subprocess, `gh` CLI (used by Claude inside the subprocess)

---

### Task 1: Add new data models (Rust)

**Files:**
- Modify: `src-tauri/src/models.rs:18-22` (MaintainerConfig)
- Modify: `src-tauri/src/models.rs:115-156` (replace old types with new ones)

**Step 1: Write the failing test**

Add this test to the `mod tests` block in `src-tauri/src/models.rs`:

```rust
#[test]
fn test_maintainer_config_github_repo_defaults_to_none() {
    let json = r#"{
        "id": "550e8400-e29b-41d4-a716-446655440000",
        "name": "test-project",
        "repo_path": "/tmp/test-repo",
        "created_at": "2026-02-28T00:00:00Z",
        "archived": false,
        "sessions": []
    }"#;
    let project: Project = serde_json::from_str(json).expect("deserialize");
    assert!(project.maintainer.github_repo.is_none());
}

#[test]
fn test_maintainer_run_log_serialization() {
    let log = MaintainerRunLog {
        id: Uuid::new_v4(),
        project_id: Uuid::new_v4(),
        timestamp: "2026-03-09T12:00:00Z".to_string(),
        issues_filed: vec![IssueSummary {
            issue_number: 42,
            title: "Fix flaky test".to_string(),
            url: "https://github.com/owner/repo/issues/42".to_string(),
            labels: vec!["filed-by-maintainer".to_string(), "priority: high".to_string()],
            action: IssueAction::Filed,
        }],
        issues_updated: vec![],
        issues_unchanged: 3,
        summary: "Filed 1 new issue".to_string(),
    };
    let json = serde_json::to_string(&log).expect("serialize");
    let deserialized: MaintainerRunLog = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(deserialized.issues_filed.len(), 1);
    assert_eq!(deserialized.issues_filed[0].issue_number, 42);
    assert_eq!(deserialized.issues_unchanged, 3);
}

#[test]
fn test_issue_action_serialization() {
    let filed = IssueAction::Filed;
    let updated = IssueAction::Updated;
    assert_eq!(serde_json::to_string(&filed).unwrap(), "\"filed\"");
    assert_eq!(serde_json::to_string(&updated).unwrap(), "\"updated\"");
}
```

**Step 2: Run test to verify it fails**

Run: `cd src-tauri && cargo test test_maintainer_config_github_repo_defaults_to_none test_maintainer_run_log_serialization test_issue_action_serialization 2>&1 | tail -20`
Expected: FAIL — `MaintainerRunLog`, `IssueSummary`, `IssueAction` don't exist yet

**Step 3: Write minimal implementation**

In `src-tauri/src/models.rs`, replace the `MaintainerConfig` struct (lines 18-31) with:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MaintainerConfig {
    pub enabled: bool,
    pub interval_minutes: u64,
    #[serde(default)]
    pub github_repo: Option<String>,
}

impl Default for MaintainerConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            interval_minutes: 60,
            github_repo: None,
        }
    }
}
```

Replace the old types (lines 115-156: `FindingSeverity`, `FindingAction`, `MaintainerFinding`, `ReportStatus`, `MaintainerReport`) with:

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum IssueAction {
    Filed,
    Updated,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IssueSummary {
    pub issue_number: u32,
    pub title: String,
    pub url: String,
    pub labels: Vec<String>,
    pub action: IssueAction,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MaintainerRunLog {
    pub id: Uuid,
    pub project_id: Uuid,
    /// ISO 8601 UTC timestamp. Lexicographic order must equal chronological order.
    pub timestamp: String,
    pub issues_filed: Vec<IssueSummary>,
    pub issues_updated: Vec<IssueSummary>,
    pub issues_unchanged: u32,
    pub summary: String,
}
```

Also keep the old types temporarily with a `#[deprecated]` and `_Old` suffix so that downstream code (storage, commands, maintainer.rs) still compiles while we migrate them in subsequent tasks. Actually — we'll migrate everything in order, so just delete the old types. The compiler will point us to all the places that need updating.

**Step 4: Fix compilation errors in old tests**

Remove these tests from `src-tauri/src/models.rs` that reference deleted types:
- `test_maintainer_finding_serialization` (line 421)
- `test_maintainer_report_serialization` (line 434)

**Step 5: Run test to verify new tests pass**

Run: `cd src-tauri && cargo test test_maintainer_config_github_repo_defaults_to_none test_maintainer_run_log_serialization test_issue_action_serialization 2>&1 | tail -20`
Expected: PASS (but other modules will have compile errors — that's OK, we fix them in the next tasks)

**Step 6: Commit**

```bash
git add src-tauri/src/models.rs
git commit -m "feat: add MaintainerRunLog and IssueSummary types, remove old finding types"
```

---

### Task 2: Update storage layer

**Files:**
- Modify: `src-tauri/src/storage.rs:1-5` (imports)
- Modify: `src-tauri/src/storage.rs:158-218` (maintainer storage methods)

**Step 1: Write the failing test**

Replace the `make_report` helper and all maintainer tests in `src-tauri/src/storage.rs` (lines 416-507) with:

```rust
fn make_run_log(project_id: Uuid, timestamp: &str) -> MaintainerRunLog {
    MaintainerRunLog {
        id: Uuid::new_v4(),
        project_id,
        timestamp: timestamp.to_string(),
        issues_filed: vec![IssueSummary {
            issue_number: 1,
            title: "Test issue".to_string(),
            url: "https://github.com/owner/repo/issues/1".to_string(),
            labels: vec!["filed-by-maintainer".to_string()],
            action: IssueAction::Filed,
        }],
        issues_updated: vec![],
        issues_unchanged: 0,
        summary: "Filed 1 issue".to_string(),
    }
}

#[test]
fn test_save_and_load_run_log() {
    let tmp = TempDir::new().unwrap();
    let storage = make_storage(&tmp);
    let project_id = Uuid::new_v4();
    let log = make_run_log(project_id, "2026-03-09T00:00:00Z");
    let log_id = log.id;

    storage.save_maintainer_run_log(&log).expect("save");
    let latest = storage.latest_maintainer_run_log(project_id).expect("load");
    assert!(latest.is_some());
    let latest = latest.unwrap();
    assert_eq!(latest.id, log_id);
    assert_eq!(latest.summary, "Filed 1 issue");
}

#[test]
fn test_latest_run_log_returns_none_when_empty() {
    let tmp = TempDir::new().unwrap();
    let storage = make_storage(&tmp);
    let project_id = Uuid::new_v4();
    let latest = storage.latest_maintainer_run_log(project_id).expect("load");
    assert!(latest.is_none());
}

#[test]
fn test_run_log_history() {
    let tmp = TempDir::new().unwrap();
    let storage = make_storage(&tmp);
    let project_id = Uuid::new_v4();

    let r1 = make_run_log(project_id, "2026-03-09T01:00:00Z");
    let r2 = make_run_log(project_id, "2026-03-09T02:00:00Z");
    let r3 = make_run_log(project_id, "2026-03-09T03:00:00Z");

    storage.save_maintainer_run_log(&r1).expect("save r1");
    storage.save_maintainer_run_log(&r2).expect("save r2");
    storage.save_maintainer_run_log(&r3).expect("save r3");

    let history = storage.maintainer_run_log_history(project_id, 10).expect("history");
    assert_eq!(history.len(), 3);
    assert_eq!(history[0].timestamp, "2026-03-09T03:00:00Z");
    assert_eq!(history[2].timestamp, "2026-03-09T01:00:00Z");
}

#[test]
fn test_clear_maintainer_run_logs() {
    let tmp = TempDir::new().unwrap();
    let storage = make_storage(&tmp);
    let project_id = Uuid::new_v4();

    let r1 = make_run_log(project_id, "2026-03-09T01:00:00Z");
    storage.save_maintainer_run_log(&r1).expect("save");
    assert!(storage.latest_maintainer_run_log(project_id).unwrap().is_some());

    storage.clear_maintainer_run_logs(project_id).expect("clear");
    assert!(storage.latest_maintainer_run_log(project_id).unwrap().is_none());
}

#[test]
fn test_clear_maintainer_run_logs_idempotent() {
    let tmp = TempDir::new().unwrap();
    let storage = make_storage(&tmp);
    let project_id = Uuid::new_v4();
    assert!(storage.clear_maintainer_run_logs(project_id).is_ok());
}
```

**Step 2: Run test to verify it fails**

Run: `cd src-tauri && cargo test test_save_and_load_run_log test_latest_run_log_returns_none_when_empty test_run_log_history test_clear_maintainer_run_logs test_clear_maintainer_run_logs_idempotent 2>&1 | tail -20`
Expected: FAIL — methods don't exist yet

**Step 3: Write minimal implementation**

Update imports at top of `src-tauri/src/storage.rs`:

```rust
use crate::models::{MaintainerRunLog, Project};
```

Replace the maintainer storage methods (lines 158-218) with:

```rust
/// Return the path to a project's maintainer run logs directory.
pub fn maintainer_run_logs_dir(&self, project_id: Uuid) -> PathBuf {
    self.project_dir(project_id).join("maintainer-reports")
}

/// Save a maintainer run log to disk.
pub fn save_maintainer_run_log(&self, log: &MaintainerRunLog) -> std::io::Result<()> {
    let dir = self.maintainer_run_logs_dir(log.project_id);
    fs::create_dir_all(&dir)?;
    let filename = format!("{}.json", log.id);
    let json = serde_json::to_string_pretty(log)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    fs::write(dir.join(filename), json)
}

/// Load the most recent maintainer run log for a project.
pub fn latest_maintainer_run_log(&self, project_id: Uuid) -> std::io::Result<Option<MaintainerRunLog>> {
    let dir = self.maintainer_run_logs_dir(project_id);
    if !dir.exists() {
        return Ok(None);
    }
    let mut logs = self.load_run_logs_from_dir(&dir)?;
    logs.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
    Ok(logs.into_iter().next())
}

/// Load maintainer run log history for a project, most recent first.
pub fn maintainer_run_log_history(&self, project_id: Uuid, limit: usize) -> std::io::Result<Vec<MaintainerRunLog>> {
    let dir = self.maintainer_run_logs_dir(project_id);
    if !dir.exists() {
        return Ok(vec![]);
    }
    let mut logs = self.load_run_logs_from_dir(&dir)?;
    logs.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
    logs.truncate(limit);
    Ok(logs)
}

/// Delete all maintainer run logs for a project.
/// Also deletes any old-format MaintainerReport files in the same directory.
pub fn clear_maintainer_run_logs(&self, project_id: Uuid) -> std::io::Result<()> {
    let dir = self.maintainer_run_logs_dir(project_id);
    if dir.exists() {
        fs::remove_dir_all(&dir)?;
    }
    Ok(())
}

fn load_run_logs_from_dir(&self, dir: &std::path::Path) -> std::io::Result<Vec<MaintainerRunLog>> {
    let mut logs = Vec::new();
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().map_or(false, |e| e == "json") {
            let json = fs::read_to_string(&path)?;
            // Skip old-format files that fail to deserialize
            if let Ok(log) = serde_json::from_str::<MaintainerRunLog>(&json) {
                logs.push(log);
            }
        }
    }
    Ok(logs)
}
```

Note: `maintainer_run_logs_dir` reuses the same `maintainer-reports/` directory name on disk for backward compatibility with the migration cleanup (old files will be silently skipped by `load_run_logs_from_dir` since they fail to deserialize as `MaintainerRunLog`).

Update the test imports at the top of the `mod tests` block in storage.rs to use the new types:

```rust
use crate::models::{
    IssueAction, IssueSummary, MaintainerRunLog, SessionConfig,
};
```

**Step 4: Run test to verify it passes**

Run: `cd src-tauri && cargo test storage::tests 2>&1 | tail -20`
Expected: PASS

**Step 5: Commit**

```bash
git add src-tauri/src/storage.rs
git commit -m "feat: replace maintainer report storage with run log storage"
```

---

### Task 3: Rewrite health check prompt and parsing

**Files:**
- Modify: `src-tauri/src/maintainer.rs` (full rewrite of prompt, parsing, and run_health_check)

**Step 1: Write the failing test**

Replace all tests in `src-tauri/src/maintainer.rs` `mod tests` with:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_issue_filing_prompt_contains_repo_path() {
        let prompt = build_issue_filing_prompt("/tmp/my-project", None);
        assert!(prompt.contains("/tmp/my-project"));
        assert!(prompt.contains("gh issue"));
        assert!(prompt.contains("filed-by-maintainer"));
    }

    #[test]
    fn test_build_issue_filing_prompt_uses_github_repo_override() {
        let prompt = build_issue_filing_prompt("/tmp/my-project", Some("owner/custom-repo"));
        assert!(prompt.contains("owner/custom-repo"));
    }

    #[test]
    fn test_parse_run_log_output_files_and_updates() {
        let output = r#"```json
{
  "issues_filed": [
    {
      "issue_number": 42,
      "title": "Fix flaky test in utils",
      "url": "https://github.com/owner/repo/issues/42",
      "labels": ["filed-by-maintainer", "priority: high", "complexity: low"]
    }
  ],
  "issues_updated": [
    {
      "issue_number": 10,
      "title": "Improve error handling",
      "url": "https://github.com/owner/repo/issues/10",
      "labels": ["filed-by-maintainer", "priority: low"]
    }
  ],
  "issues_unchanged": 2,
  "summary": "Filed 1 new issue, updated 1 existing issue"
}
```"#;
        let project_id = Uuid::new_v4();
        let result = parse_run_log_output(output, project_id);
        assert!(result.is_ok());
        let log = result.unwrap();
        assert_eq!(log.issues_filed.len(), 1);
        assert_eq!(log.issues_filed[0].issue_number, 42);
        assert_eq!(log.issues_filed[0].action, IssueAction::Filed);
        assert_eq!(log.issues_updated.len(), 1);
        assert_eq!(log.issues_updated[0].issue_number, 10);
        assert_eq!(log.issues_updated[0].action, IssueAction::Updated);
        assert_eq!(log.issues_unchanged, 2);
    }

    #[test]
    fn test_parse_run_log_output_no_changes() {
        let output = r#"{"issues_filed":[],"issues_updated":[],"issues_unchanged":5,"summary":"No changes detected"}"#;
        let project_id = Uuid::new_v4();
        let result = parse_run_log_output(output, project_id);
        assert!(result.is_ok());
        let log = result.unwrap();
        assert!(log.issues_filed.is_empty());
        assert!(log.issues_updated.is_empty());
        assert_eq!(log.issues_unchanged, 5);
    }

    #[test]
    fn test_parse_run_log_output_invalid() {
        let output = "I couldn't analyze the project";
        let project_id = Uuid::new_v4();
        let result = parse_run_log_output(output, project_id);
        assert!(result.is_err());
    }

    #[test]
    fn test_has_changes_true_when_issues_filed() {
        let log = MaintainerRunLog {
            id: Uuid::new_v4(),
            project_id: Uuid::new_v4(),
            timestamp: "2026-03-09T00:00:00Z".to_string(),
            issues_filed: vec![IssueSummary {
                issue_number: 1,
                title: "t".to_string(),
                url: "u".to_string(),
                labels: vec![],
                action: IssueAction::Filed,
            }],
            issues_updated: vec![],
            issues_unchanged: 0,
            summary: "s".to_string(),
        };
        assert!(has_changes(&log));
    }

    #[test]
    fn test_has_changes_false_when_only_unchanged() {
        let log = MaintainerRunLog {
            id: Uuid::new_v4(),
            project_id: Uuid::new_v4(),
            timestamp: "2026-03-09T00:00:00Z".to_string(),
            issues_filed: vec![],
            issues_updated: vec![],
            issues_unchanged: 3,
            summary: "s".to_string(),
        };
        assert!(!has_changes(&log));
    }
}
```

**Step 2: Run test to verify it fails**

Run: `cd src-tauri && cargo test maintainer::tests 2>&1 | tail -30`
Expected: FAIL — `build_issue_filing_prompt`, `parse_run_log_output`, `has_changes` don't exist

**Step 3: Write minimal implementation**

Rewrite `src-tauri/src/maintainer.rs` entirely:

```rust
use std::collections::HashMap;
use std::time::{Duration, Instant};

use chrono::Utc;
use serde::Deserialize;
use tauri::{AppHandle, Emitter, Manager};
use uuid::Uuid;

use crate::models::{IssueAction, IssueSummary, MaintainerRunLog};
use crate::state::AppState;

pub struct MaintainerScheduler;

impl MaintainerScheduler {
    /// Start the scheduler loop in a background thread.
    /// Checks every 60 seconds which projects are due for a health check.
    pub fn start(app_handle: AppHandle) {
        std::thread::spawn(move || {
            let mut last_run: HashMap<Uuid, Instant> = HashMap::new();

            loop {
                std::thread::sleep(Duration::from_secs(60));

                let state = match app_handle.try_state::<AppState>() {
                    Some(s) => s,
                    None => continue,
                };

                let projects = {
                    let storage = match state.storage.lock() {
                        Ok(s) => s,
                        Err(_) => continue,
                    };
                    match storage.list_projects() {
                        Ok(p) => p,
                        Err(_) => continue,
                    }
                };

                for project in &projects {
                    if !project.maintainer.enabled || project.archived {
                        continue;
                    }

                    let interval =
                        Duration::from_secs(project.maintainer.interval_minutes * 60);
                    let should_run = last_run
                        .get(&project.id)
                        .map_or(true, |t| t.elapsed() >= interval);

                    if !should_run {
                        continue;
                    }

                    last_run.insert(project.id, Instant::now());

                    let _ = app_handle.emit(
                        &format!("maintainer-status:{}", project.id),
                        "running",
                    );

                    let github_repo = project.maintainer.github_repo.as_deref();
                    let result = run_maintainer_check(&project.repo_path, project.id, github_repo);

                    match result {
                        Ok(log) => {
                            // Only save if something changed (diff-based silence)
                            if has_changes(&log) {
                                if let Ok(storage) = state.storage.lock() {
                                    let _ = storage.save_maintainer_run_log(&log);
                                }
                            }

                            let _ = app_handle.emit(
                                &format!("maintainer-status:{}", project.id),
                                "idle",
                            );
                        }
                        Err(e) => {
                            eprintln!(
                                "Maintainer check failed for {}: {}",
                                project.name, e
                            );
                            let _ = app_handle.emit(
                                &format!("maintainer-status:{}", project.id),
                                "error",
                            );
                        }
                    }
                }
            }
        });
    }
}

pub fn build_issue_filing_prompt(repo_path: &str, github_repo: Option<&str>) -> String {
    let repo_flag = match github_repo {
        Some(repo) => format!(" -R {repo}"),
        None => String::new(),
    };

    format!(
        r#"You are a maintainer agent. Analyze the project at {repo_path} for code quality, test robustness, architecture, and documentation issues.

For each issue you find, you must either file a new GitHub issue or update an existing one.

## Step 1: Ensure labels exist

Run these commands to create labels if they don't already exist (ignore errors if they already exist):

gh label create "filed-by-maintainer" --description "Issue filed by maintainer agent" --color "6c7086" --force{repo_flag}
gh label create "priority: low" --description "Low priority" --color "a6e3a1" --force{repo_flag}
gh label create "priority: high" --description "High priority" --color "f38ba8" --force{repo_flag}
gh label create "complexity: low" --description "Low complexity" --color "89b4fa" --force{repo_flag}
gh label create "complexity: high" --description "High complexity" --color "f9e2af" --force{repo_flag}

## Step 2: Check existing issues

gh issue list --label filed-by-maintainer --state open --json number,title,body,labels{repo_flag}

## Step 3: Analyze and act

For each finding:
1. Determine if it semantically matches an existing open `filed-by-maintainer` issue (same underlying problem, not just string match)
2. If it matches: update the issue body with current analysis via `gh issue edit <number> --body "..."{repo_flag}`, then add a comment noting what changed via `gh issue comment <number> --body "..."{repo_flag}`
3. If no match: file a new issue via `gh issue create --title "..." --body "..." --label filed-by-maintainer --label <priority> --label <complexity>{repo_flag}`

Each issue should be specific and actionable (one issue per finding, not grouped).

Assign `priority: low` or `priority: high` based on impact.
Assign `complexity: low` or `complexity: high` based on effort.

## Step 4: Return summary

After completing all gh commands, respond with ONLY a JSON block:

```json
{{
  "issues_filed": [
    {{
      "issue_number": <number>,
      "title": "<title>",
      "url": "<issue url>",
      "labels": ["filed-by-maintainer", "<priority>", "<complexity>"]
    }}
  ],
  "issues_updated": [
    {{
      "issue_number": <number>,
      "title": "<title>",
      "url": "<issue url>",
      "labels": ["filed-by-maintainer", "<priority>", "<complexity>"]
    }}
  ],
  "issues_unchanged": <count of existing issues with no change needed>,
  "summary": "<one-line summary of actions taken>"
}}
```

If there are no issues to file or update, return empty arrays and a positive summary."#
    )
}

pub fn extract_json(output: &str) -> Option<&str> {
    // Try ```json block first
    if let Some(start) = output.find("```json") {
        let json_start = start + "```json".len();
        if let Some(end) = output[json_start..].find("```") {
            return Some(output[json_start..json_start + end].trim());
        }
    }

    // Best-effort heuristic: find first '{' and last '}'
    if let Some(start) = output.find('{') {
        if let Some(end) = output.rfind('}') {
            if end >= start {
                return Some(&output[start..=end]);
            }
        }
    }

    None
}

#[derive(Deserialize)]
struct RawRunLog {
    issues_filed: Vec<RawIssueSummary>,
    issues_updated: Vec<RawIssueSummary>,
    #[serde(default)]
    issues_unchanged: u32,
    summary: String,
}

#[derive(Deserialize)]
struct RawIssueSummary {
    issue_number: u32,
    title: String,
    url: String,
    #[serde(default)]
    labels: Vec<String>,
}

pub fn parse_run_log_output(output: &str, project_id: Uuid) -> Result<MaintainerRunLog, String> {
    let json_str = extract_json(output).ok_or("No JSON found in output")?;
    let raw: RawRunLog =
        serde_json::from_str(json_str).map_err(|e| format!("Failed to parse JSON: {}", e))?;

    let issues_filed: Vec<IssueSummary> = raw
        .issues_filed
        .into_iter()
        .map(|r| IssueSummary {
            issue_number: r.issue_number,
            title: r.title,
            url: r.url,
            labels: r.labels,
            action: IssueAction::Filed,
        })
        .collect();

    let issues_updated: Vec<IssueSummary> = raw
        .issues_updated
        .into_iter()
        .map(|r| IssueSummary {
            issue_number: r.issue_number,
            title: r.title,
            url: r.url,
            labels: r.labels,
            action: IssueAction::Updated,
        })
        .collect();

    Ok(MaintainerRunLog {
        id: Uuid::new_v4(),
        project_id,
        timestamp: Utc::now().to_rfc3339(),
        issues_filed,
        issues_updated,
        issues_unchanged: raw.issues_unchanged,
        summary: raw.summary,
    })
}

/// Returns true if the run produced any filed or updated issues.
pub fn has_changes(log: &MaintainerRunLog) -> bool {
    !log.issues_filed.is_empty() || !log.issues_updated.is_empty()
}

pub fn run_maintainer_check(
    repo_path: &str,
    project_id: Uuid,
    github_repo: Option<&str>,
) -> Result<MaintainerRunLog, String> {
    let prompt = build_issue_filing_prompt(repo_path, github_repo);
    let output = std::process::Command::new("claude")
        .arg("--print")
        .arg("--allowedTools")
        .arg("Bash")
        .arg(&prompt)
        .current_dir(repo_path)
        .env_remove("CLAUDECODE")
        .output()
        .map_err(|e| format!("Failed to run claude --print: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("claude --print failed: {}", stderr));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    parse_run_log_output(&stdout, project_id)
}
```

Key changes from the old version:
- `build_health_check_prompt` → `build_issue_filing_prompt` with `github_repo` parameter and `--repo` flag support
- `parse_report_output` → `parse_run_log_output` parsing `IssueSummary` instead of `MaintainerFinding`
- `run_health_check` → `run_maintainer_check` with `github_repo` parameter and `--allowedTools Bash` flag so Claude can run `gh` commands
- New `has_changes()` function for diff-based silence
- Scheduler emits "idle" on success instead of "passing"/"warnings"/"failing" (no more status tiers)

**Step 4: Run test to verify it passes**

Run: `cd src-tauri && cargo test maintainer::tests 2>&1 | tail -20`
Expected: PASS

**Step 5: Commit**

```bash
git add src-tauri/src/maintainer.rs
git commit -m "feat: rewrite maintainer to file GitHub issues via claude + gh CLI"
```

---

### Task 4: Update Tauri commands

**Files:**
- Modify: `src-tauri/src/commands.rs:1141-1251` (maintainer commands)

**Step 1: Update commands**

Replace the maintainer commands section (lines 1141-1251) in `src-tauri/src/commands.rs`:

```rust
pub(crate) fn validate_maintainer_interval(minutes: u64) -> Result<(), String> {
    if minutes < 5 {
        return Err("Interval must be at least 5 minutes".to_string());
    }
    Ok(())
}

#[tauri::command]
pub async fn configure_maintainer(
    state: State<'_, AppState>,
    project_id: String,
    enabled: bool,
    interval_minutes: u64,
    github_repo: Option<String>,
) -> Result<(), String> {
    validate_maintainer_interval(interval_minutes)?;
    let project_id = Uuid::parse_str(&project_id).map_err(|e| e.to_string())?;
    let storage = state.storage.lock().map_err(|e| e.to_string())?;
    let mut project = storage.load_project(project_id).map_err(|e| e.to_string())?;
    project.maintainer.enabled = enabled;
    project.maintainer.interval_minutes = interval_minutes;
    project.maintainer.github_repo = github_repo;
    storage.save_project(&project).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub async fn get_maintainer_status(
    state: State<'_, AppState>,
    project_id: String,
) -> Result<Option<crate::models::MaintainerRunLog>, String> {
    let project_id = Uuid::parse_str(&project_id).map_err(|e| e.to_string())?;
    let storage = state.storage.lock().map_err(|e| e.to_string())?;
    storage
        .latest_maintainer_run_log(project_id)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_maintainer_history(
    state: State<'_, AppState>,
    project_id: String,
) -> Result<Vec<crate::models::MaintainerRunLog>, String> {
    let project_id = Uuid::parse_str(&project_id).map_err(|e| e.to_string())?;
    let storage = state.storage.lock().map_err(|e| e.to_string())?;
    storage
        .maintainer_run_log_history(project_id, 20)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn trigger_maintainer_check(
    state: State<'_, AppState>,
    app_handle: AppHandle,
    project_id: String,
) -> Result<crate::models::MaintainerRunLog, String> {
    let project_id = Uuid::parse_str(&project_id).map_err(|e| e.to_string())?;

    let (repo_path, github_repo) = {
        let storage = state.storage.lock().map_err(|e| e.to_string())?;
        let project = storage.load_project(project_id).map_err(|e| e.to_string())?;
        (project.repo_path.clone(), project.maintainer.github_repo.clone())
    };

    let _ = app_handle.emit(&format!("maintainer-status:{}", project_id), "running");

    let log = crate::maintainer::run_maintainer_check(
        &repo_path,
        project_id,
        github_repo.as_deref(),
    )?;

    {
        let storage = state.storage.lock().map_err(|e| e.to_string())?;
        storage
            .save_maintainer_run_log(&log)
            .map_err(|e| e.to_string())?;
    }

    let _ = app_handle.emit(&format!("maintainer-status:{}", project_id), "idle");

    Ok(log)
}

#[tauri::command]
pub async fn clear_maintainer_reports(
    state: State<'_, AppState>,
    app_handle: AppHandle,
    project_id: String,
) -> Result<(), String> {
    let project_id = Uuid::parse_str(&project_id).map_err(|e| e.to_string())?;
    let storage = state.storage.lock().map_err(|e| e.to_string())?;
    storage
        .clear_maintainer_run_logs(project_id)
        .map_err(|e| e.to_string())?;
    let _ = app_handle.emit(&format!("maintainer-status:{}", project_id), "idle");
    Ok(())
}
```

Note: `clear_maintainer_reports` keeps its old name to avoid changing the frontend invocation and command registration in one step.

**Step 2: Verify compilation**

Run: `cd src-tauri && cargo check 2>&1 | tail -20`
Expected: Should compile (may have warnings about unused imports in commands.rs — remove any leftover `ReportStatus` imports)

**Step 3: Run all Rust tests**

Run: `cd src-tauri && cargo test 2>&1 | tail -30`
Expected: PASS

**Step 4: Commit**

```bash
git add src-tauri/src/commands.rs
git commit -m "feat: update maintainer commands to use run logs instead of reports"
```

---

### Task 5: Update frontend types and stores

**Files:**
- Modify: `src/lib/stores.ts:28-53` (replace MaintainerFinding/MaintainerReport with new types)

**Step 1: Update types**

In `src/lib/stores.ts`, replace lines 28-53 with:

```typescript
export interface MaintainerConfig {
  enabled: boolean;
  interval_minutes: number;
  github_repo?: string | null;
}

export interface AutoWorkerConfig {
  enabled: boolean;
}

export interface IssueSummary {
  issue_number: number;
  title: string;
  url: string;
  labels: string[];
  action: "filed" | "updated";
}

export interface MaintainerRunLog {
  id: string;
  project_id: string;
  timestamp: string;
  issues_filed: IssueSummary[];
  issues_updated: IssueSummary[];
  issues_unchanged: number;
  summary: string;
}

export type MaintainerStatus = "idle" | "running" | "error";
```

Note: `MaintainerStatus` is simplified — no more "passing"/"warnings"/"failing" since there's no severity-based status. It's just idle, running, or error.

**Step 2: Commit**

```bash
git add src/lib/stores.ts
git commit -m "feat: replace MaintainerReport types with MaintainerRunLog in frontend"
```

---

### Task 6: Update AgentDashboard UI

**Files:**
- Modify: `src/lib/AgentDashboard.svelte` (full rewrite of maintainer section)

**Step 1: Update the component**

Replace the entire `src/lib/AgentDashboard.svelte` with the updated version. Key changes:

1. Replace `MaintainerReport` with `MaintainerRunLog` throughout
2. Remove `severityColor`, `actionLabel` helper functions (no more findings)
3. Replace the detail view (finding blocks) with a run log detail showing issue summaries with clickable GitHub links
4. Simplify the report list to show run log summaries
5. Remove status-based coloring (no more passing/warnings/failing for reports)

```svelte
<script lang="ts">
  import { fromStore } from "svelte/store";
  import { invoke } from "@tauri-apps/api/core";
  import { focusTarget, projects, maintainerStatuses, autoWorkerStatuses, hotkeyAction, type Project, type FocusTarget, type MaintainerRunLog, type MaintainerStatus, type AutoWorkerStatus } from "./stores";
  import { showToast } from "./toast";

  let runLogs: MaintainerRunLog[] = $state([]);
  let loading = $state(false);
  let triggerLoading = $state(false);
  let currentProjectId: string | null = $state(null);

  // Panel navigation state
  let selectedIndex = $state(0);
  let openLogIndex: number | null = $state(null);
  let detailBlockIndex = $state(0);

  const projectsState = fromStore(projects);
  let projectList: Project[] = $derived(projectsState.current);
  const focusTargetState = fromStore(focusTarget);
  let currentFocus: FocusTarget = $derived(focusTargetState.current);

  let focusedAgent = $derived(
    currentFocus?.type === "agent" || currentFocus?.type === "agent-panel"
      ? currentFocus
      : null
  );

  let panelFocused = $derived(currentFocus?.type === "agent-panel");

  let project = $derived(
    focusedAgent
      ? projectList.find((p) => p.id === focusedAgent!.projectId) ?? null
      : null
  );

  let openLog = $derived(
    openLogIndex !== null ? runLogs[openLogIndex] ?? null : null
  );

  // All issues in the open log (for detail navigation)
  let openLogIssues = $derived(
    openLog ? [...openLog.issues_filed, ...openLog.issues_updated] : []
  );

  // Fetch history when project changes
  $effect(() => {
    const pid = project?.id ?? null;
    if (pid && pid !== currentProjectId) {
      currentProjectId = pid;
      if (focusedAgent?.agentKind === "maintainer") {
        fetchHistory(pid);
      }
    }
  });

  // Reset panel state when switching agents
  let prevAgentKey: string | null = $state(null);
  $effect(() => {
    const key = focusedAgent ? `${focusedAgent.projectId}:${focusedAgent.agentKind}` : null;
    if (key !== prevAgentKey) {
      prevAgentKey = key;
      selectedIndex = 0;
      openLogIndex = null;
      detailBlockIndex = 0;
    }
  });

  // Handle panel navigation actions
  $effect(() => {
    const unsub = hotkeyAction.subscribe((action) => {
      if (!action) return;
      if (action.type === "agent-panel-navigate") {
        handleNavigate(action.direction);
      } else if (action.type === "agent-panel-select") {
        handleSelect();
      } else if (action.type === "agent-panel-escape") {
        handleEscape();
      } else if (action.type === "trigger-maintainer-check") {
        triggerCheck();
      } else if (action.type === "clear-maintainer-reports") {
        clearRunLogs();
      }
    });
    return unsub;
  });

  function handleNavigate(direction: 1 | -1) {
    if (focusedAgent?.agentKind !== "maintainer") return;

    if (openLogIndex !== null) {
      // Detail view: scroll through issue blocks
      const maxBlock = openLogIssues.length; // 0 = summary, 1..N = issues
      detailBlockIndex = Math.max(0, Math.min(maxBlock, detailBlockIndex + direction));
      scrollBlockIntoView();
    } else {
      // List view: move selection
      if (runLogs.length === 0) return;
      selectedIndex = Math.max(0, Math.min(runLogs.length - 1, selectedIndex + direction));
      scrollReportIntoView();
    }
  }

  function handleSelect() {
    if (focusedAgent?.agentKind !== "maintainer") return;
    if (openLogIndex !== null) return;
    if (runLogs.length === 0) return;
    openLogIndex = selectedIndex;
    detailBlockIndex = 0;
  }

  function handleEscape() {
    if (openLogIndex !== null) {
      openLogIndex = null;
      detailBlockIndex = 0;
    } else if (focusedAgent) {
      focusTarget.set({ type: "agent", agentKind: focusedAgent.agentKind, projectId: focusedAgent.projectId });
    }
  }

  function scrollBlockIntoView() {
    requestAnimationFrame(() => {
      const el = document.querySelector(`[data-block-index="${detailBlockIndex}"]`);
      if (el) el.scrollIntoView({ behavior: "smooth", block: "nearest" });
    });
  }

  function scrollReportIntoView() {
    requestAnimationFrame(() => {
      const el = document.querySelector(`[data-report-index="${selectedIndex}"]`);
      if (el) el.scrollIntoView({ behavior: "smooth", block: "nearest" });
    });
  }

  async function fetchHistory(projectId: string) {
    loading = true;
    try {
      runLogs = await invoke<MaintainerRunLog[]>("get_maintainer_history", { projectId });
    } catch {
      runLogs = [];
    } finally {
      loading = false;
    }
  }

  async function triggerCheck() {
    if (!project) return;
    triggerLoading = true;
    try {
      await invoke<MaintainerRunLog>("trigger_maintainer_check", { projectId: project.id });
      runLogs = await invoke<MaintainerRunLog[]>("get_maintainer_history", { projectId: project.id });
      showToast("Maintainer check complete", "info");
    } catch (e) {
      showToast(String(e), "error");
    } finally {
      triggerLoading = false;
    }
  }

  async function clearRunLogs() {
    if (!project) return;
    try {
      await invoke("clear_maintainer_reports", { projectId: project.id });
      runLogs = [];
      openLogIndex = null;
      selectedIndex = 0;
      showToast("Maintainer logs cleared", "info");
    } catch (e) {
      showToast(String(e), "error");
    }
  }

  let nextRunText = $state("");

  function computeNextRunText(): string {
    if (!project?.maintainer.enabled) return "Disabled";
    if (runLogs.length === 0) return "Pending";
    const lastRun = new Date(runLogs[0].timestamp).getTime();
    const intervalMs = project.maintainer.interval_minutes * 60 * 1000;
    const nextRun = lastRun + intervalMs;
    const diffMs = nextRun - Date.now();
    if (diffMs <= 0) return "Due now";
    const totalSecs = Math.floor(diffMs / 1000);
    const mins = Math.floor(totalSecs / 60);
    const secs = totalSecs % 60;
    return mins > 0 ? `${mins}m ${secs}s` : `${secs}s`;
  }

  $effect(() => {
    nextRunText = computeNextRunText();
    const id = setInterval(() => { nextRunText = computeNextRunText(); }, 1_000);
    return () => clearInterval(id);
  });

  const maintainerStatusesState = fromStore(maintainerStatuses);
  let maintainerStatus: MaintainerStatus | null = $derived(
    project ? (maintainerStatusesState.current.get(project.id) ?? null) : null
  );

  const autoWorkerStatusesState = fromStore(autoWorkerStatuses);
  let autoWorkerStatus: AutoWorkerStatus | null = $derived(
    project ? (autoWorkerStatusesState.current.get(project.id) ?? null) : null
  );

  function formatTimestamp(ts: string): string {
    return new Date(ts).toLocaleString();
  }

  function actionColor(action: string): string {
    return action === "filed" ? "#a6e3a1" : "#89b4fa";
  }
</script>

<div class="dashboard">
  {#if !focusedAgent || !project}
    <div class="empty-state">
      <div class="empty-title">No agent selected</div>
      <div class="empty-hint">Navigate to an agent with <kbd>j</kbd> / <kbd>k</kbd> and press <kbd>l</kbd></div>
    </div>
  {:else if focusedAgent.agentKind === "auto-worker"}
    <div class="dashboard-header">
      <h2>{project.name}</h2>
      <span class="header-subtitle">Auto-worker</span>
    </div>
    <section class="section">
      <div class="section-header">
        <span class="section-title">Auto-worker</span>
        <span class="badge" class:enabled={project.auto_worker.enabled}>
          {project.auto_worker.enabled ? "ON" : "OFF"}
        </span>
        {#if autoWorkerStatus?.status === "working"}
          <span class="status-running">Working</span>
        {/if}
      </div>
      <div class="section-body">
        {#if !project.auto_worker.enabled}
          <p class="muted">Disabled — press <kbd>o</kbd> to enable</p>
        {:else if autoWorkerStatus?.status === "working"}
          <div class="worker-info">
            <span class="worker-label">Working on:</span>
            <span class="worker-issue">#{autoWorkerStatus.issue_number} {autoWorkerStatus.issue_title}</span>
          </div>
        {:else}
          <p class="muted">Waiting for eligible issues</p>
        {/if}
      </div>
    </section>
  {:else if focusedAgent.agentKind === "maintainer"}
    <div class="dashboard-header">
      <h2>{project.name}</h2>
      <span class="header-subtitle">Maintainer</span>
    </div>
    <section class="section">
      <div class="section-header">
        <span class="section-title">Maintainer</span>
        <span class="badge" class:enabled={project.maintainer.enabled}>
          {project.maintainer.enabled ? "ON" : "OFF"}
        </span>
        {#if maintainerStatus === "running"}
          <span class="maintainer-status running">running</span>
        {:else if maintainerStatus === "error"}
          <span class="maintainer-status error">error</span>
        {/if}
      </div>

      {#if project.maintainer.enabled}
        <div class="schedule-row">
          <span>Interval: {project.maintainer.interval_minutes}m</span>
          <span>Next: {nextRunText}</span>
        </div>
      {/if}
    </section>

    <section class="section report-section">
      {#if loading}
        <div class="section-body">
          <p class="muted">Loading...</p>
        </div>
      {:else if openLog}
        <div class="detail-view">
          <div class="detail-header">
            <span class="detail-back">Run logs</span>
            <span class="detail-timestamp">{formatTimestamp(openLog.timestamp)}</span>
            <span class="detail-summary">{openLog.summary}</span>
          </div>
          <div class="detail-blocks">
            <div
              class="detail-block"
              class:block-focused={panelFocused && detailBlockIndex === 0}
              data-block-index="0"
            >
              <div class="run-summary">
                <span class="summary-stat">{openLog.issues_filed.length} filed</span>
                <span class="summary-stat">{openLog.issues_updated.length} updated</span>
                <span class="summary-stat">{openLog.issues_unchanged} unchanged</span>
              </div>
            </div>
            {#each openLogIssues as issue, i}
              <div
                class="detail-block"
                class:block-focused={panelFocused && detailBlockIndex === i + 1}
                data-block-index={i + 1}
              >
                <div class="issue-item">
                  <span class="issue-action" style="color: {actionColor(issue.action)}">{issue.action}</span>
                  <span class="issue-number">#{issue.issue_number}</span>
                  <span class="issue-title">{issue.title}</span>
                  <div class="issue-labels">
                    {#each issue.labels.filter(l => l !== "filed-by-maintainer") as label}
                      <span class="issue-label">{label}</span>
                    {/each}
                  </div>
                </div>
              </div>
            {/each}
          </div>
        </div>
      {:else}
        <div class="report-list">
          {#if runLogs.length === 0}
            <div class="section-body">
              <p class="muted">No run logs yet</p>
              {#if project.maintainer.enabled}
                <button class="btn" onclick={triggerCheck} disabled={triggerLoading}>
                  {triggerLoading ? "Running..." : "(r) Run check now"}
                </button>
              {/if}
            </div>
          {:else}
            {#each runLogs as log, i}
              <!-- svelte-ignore a11y_no_static_element_interactions -->
              <!-- svelte-ignore a11y_click_events_have_key_events -->
              <div
                class="report-item"
                class:selected={panelFocused && selectedIndex === i}
                data-report-index={i}
                onclick={() => { selectedIndex = i; openLogIndex = i; detailBlockIndex = 0; }}
              >
                <span class="log-dot"></span>
                <span class="report-timestamp">{formatTimestamp(log.timestamp)}</span>
                <span class="report-summary-preview">{log.summary}</span>
              </div>
            {/each}
          {/if}
        </div>
      {/if}
    </section>

    {#if !panelFocused}
      <div class="panel-hint">
        <span class="muted">Press <kbd>l</kbd> to browse run logs</span>
      </div>
    {/if}
  {/if}
</div>

<style>
  .dashboard { width: 100%; height: 100%; overflow-y: auto; background: #11111b; color: #cdd6f4; }
  .empty-state { display: flex; flex-direction: column; align-items: center; justify-content: center; height: 100%; gap: 8px; }
  .empty-title { font-size: 16px; font-weight: 500; }
  .empty-hint { color: #6c7086; font-size: 13px; }
  .empty-hint kbd, .muted kbd, .panel-hint kbd { background: #313244; color: #89b4fa; padding: 1px 6px; border-radius: 3px; font-family: monospace; font-size: 12px; }
  .dashboard-header { padding: 16px 24px; border-bottom: 1px solid #313244; display: flex; align-items: baseline; }
  .dashboard-header h2 { font-size: 16px; font-weight: 600; margin: 0; }
  .header-subtitle { font-size: 12px; color: #6c7086; margin-left: 8px; }
  .section { border-bottom: 1px solid #313244; }
  .section-header { padding: 12px 24px; display: flex; align-items: center; gap: 8px; border-bottom: 1px solid rgba(49, 50, 68, 0.5); }
  .section-title { font-size: 13px; font-weight: 600; flex: 1; }
  .badge { font-size: 10px; padding: 1px 6px; border-radius: 3px; background: #313244; color: #6c7086; }
  .badge.enabled { background: rgba(166, 227, 161, 0.2); color: #a6e3a1; }
  .status-running { font-size: 11px; color: #89b4fa; }
  .schedule-row { padding: 8px 24px; display: flex; justify-content: space-between; font-size: 11px; color: #6c7086; border-bottom: 1px solid rgba(49, 50, 68, 0.5); }
  .section-body { padding: 16px 24px; }
  .muted { color: #6c7086; font-size: 13px; margin: 0; }
  .worker-info { display: flex; flex-direction: column; gap: 4px; }
  .worker-label { color: #6c7086; font-size: 11px; }
  .worker-issue { font-size: 13px; }
  .maintainer-status { font-size: 11px; font-weight: 500; text-transform: capitalize; }
  .maintainer-status.running { color: #89b4fa; }
  .maintainer-status.error { color: #f38ba8; }
  .btn { background: #313244; border: none; color: #cdd6f4; padding: 6px 12px; border-radius: 4px; font-size: 12px; cursor: pointer; box-shadow: none; margin-top: 8px; }
  .btn:hover { background: #45475a; }
  .btn:disabled { opacity: 0.5; cursor: not-allowed; }

  /* Run log list */
  .report-section { border-bottom: none; flex: 1; }
  .report-list { display: flex; flex-direction: column; }
  .report-item {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 10px 24px;
    cursor: pointer;
    font-size: 12px;
    border-bottom: 1px solid rgba(49, 50, 68, 0.3);
  }
  .report-item:hover { background: rgba(49, 50, 68, 0.3); }
  .report-item.selected {
    background: rgba(137, 180, 250, 0.1);
    outline: 1px solid rgba(137, 180, 250, 0.4);
    outline-offset: -1px;
  }
  .log-dot { width: 8px; height: 8px; border-radius: 50%; flex-shrink: 0; background: #89b4fa; }
  .report-timestamp { color: #6c7086; font-size: 11px; white-space: nowrap; flex-shrink: 0; }
  .report-summary-preview { flex: 1; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; color: #bac2de; }

  /* Detail view */
  .detail-view { display: flex; flex-direction: column; }
  .detail-header {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 10px 24px;
    border-bottom: 1px solid rgba(49, 50, 68, 0.5);
    font-size: 12px;
  }
  .detail-back { color: #6c7086; }
  .detail-timestamp { color: #6c7086; font-size: 11px; }
  .detail-summary { font-size: 11px; color: #bac2de; margin-left: auto; }
  .detail-blocks { padding: 12px 24px; display: flex; flex-direction: column; gap: 8px; }
  .detail-block { border-radius: 6px; transition: outline-color 0.15s; outline: 2px solid transparent; outline-offset: 2px; }
  .detail-block.block-focused { outline-color: rgba(137, 180, 250, 0.5); }

  .run-summary { padding: 12px; border-radius: 6px; background: rgba(49, 50, 68, 0.3); display: flex; gap: 16px; border-left: 3px solid #89b4fa; }
  .summary-stat { font-size: 13px; color: #cdd6f4; }

  .issue-item { padding: 8px 12px; background: rgba(49, 50, 68, 0.2); border-radius: 4px; font-size: 12px; display: flex; flex-direction: column; gap: 2px; }
  .issue-action { font-weight: 600; font-size: 11px; text-transform: uppercase; }
  .issue-number { color: #6c7086; font-size: 11px; }
  .issue-title { color: #cdd6f4; }
  .issue-labels { display: flex; gap: 4px; flex-wrap: wrap; margin-top: 2px; }
  .issue-label { font-size: 10px; padding: 1px 6px; border-radius: 3px; background: #313244; color: #6c7086; }

  .panel-hint { padding: 12px 24px; }
</style>
```

**Step 2: Verify it compiles**

Run: `npx svelte-check --threshold error 2>&1 | tail -20`
Expected: No errors

**Step 3: Commit**

```bash
git add src/lib/AgentDashboard.svelte
git commit -m "feat: replace finding detail view with run log + issue summary UI"
```

---

### Task 7: Update AgentTree sidebar status display

**Files:**
- Modify: `src/lib/sidebar/AgentTree.svelte` (simplify maintainer status display)

**Step 1: Read the current file**

Read `src/lib/sidebar/AgentTree.svelte` to find the maintainer status dot logic.

**Step 2: Update status dot classes**

The status dot currently checks for "running"/"passing"/"warnings"/"failing". Since we now only have "idle"/"running"/"error", update accordingly:

- `working` class → when status is "running"
- `idle` class → when enabled and status is not "running" and not "error"
- `error` class → when status is "error" (add new CSS class if needed)
- `disabled` class → when not enabled

The exact changes depend on the current template. The key is removing references to "passing", "warnings", and "failing" from the status checks.

**Step 3: Commit**

```bash
git add src/lib/sidebar/AgentTree.svelte
git commit -m "feat: simplify maintainer status dot for idle/running/error states"
```

---

### Task 8: Migration — clean up old reports on first run

**Files:**
- Modify: `src-tauri/src/maintainer.rs` (add migration in scheduler startup)

**Step 1: Add migration logic**

In `MaintainerScheduler::start()`, before the main loop, add a one-time migration that clears old report files. The `load_run_logs_from_dir` already silently skips old-format files, so they won't appear in the UI. But we should clean them up to avoid disk clutter.

Add this block after getting the app state for the first time, before the loop:

```rust
// One-time migration: clear old-format report files
{
    if let Some(state) = app_handle.try_state::<AppState>() {
        if let Ok(storage) = state.storage.lock() {
            if let Ok(projects) = storage.list_projects() {
                for project in &projects {
                    // clear_maintainer_run_logs deletes the entire directory,
                    // which removes both old and new format files.
                    // We only want to remove old-format files, so instead
                    // we let load_run_logs_from_dir skip them naturally.
                    // No explicit migration needed — old files are silently ignored.
                }
            }
        }
    }
}
```

Actually, since `load_run_logs_from_dir` already silently skips files that don't deserialize as `MaintainerRunLog`, no explicit migration code is needed. The old files just sit on disk harmlessly until the user clears logs (which deletes the whole directory). This is the simplest approach — no migration code to write or maintain.

**Step 1 (revised): Verify old files are skipped**

Write a test in `src-tauri/src/storage.rs`:

```rust
#[test]
fn test_old_format_reports_are_silently_skipped() {
    let tmp = TempDir::new().unwrap();
    let storage = make_storage(&tmp);
    let project_id = Uuid::new_v4();

    // Write an old-format MaintainerReport file
    let dir = storage.maintainer_run_logs_dir(project_id);
    std::fs::create_dir_all(&dir).unwrap();
    let old_report = r#"{
        "id": "550e8400-e29b-41d4-a716-446655440000",
        "project_id": "550e8400-e29b-41d4-a716-446655440001",
        "timestamp": "2026-03-07T00:00:00Z",
        "status": "passing",
        "findings": [],
        "summary": "old format"
    }"#;
    std::fs::write(dir.join("old-report.json"), old_report).unwrap();

    // Write a new-format run log
    let log = make_run_log(project_id, "2026-03-09T00:00:00Z");
    storage.save_maintainer_run_log(&log).expect("save");

    // History should only contain the new-format log
    let history = storage.maintainer_run_log_history(project_id, 10).unwrap();
    assert_eq!(history.len(), 1);
    assert_eq!(history[0].summary, "Filed 1 issue");
}
```

**Step 2: Run test**

Run: `cd src-tauri && cargo test test_old_format_reports_are_silently_skipped 2>&1 | tail -10`
Expected: PASS (this should already pass with the current storage implementation, since deserialization to `MaintainerRunLog` will fail for old-format files and they'll be skipped)

**Step 3: Commit**

```bash
git add src-tauri/src/storage.rs
git commit -m "test: verify old-format maintainer reports are silently skipped"
```

---

### Task 9: Final verification

**Step 1: Run all Rust tests**

Run: `cd src-tauri && cargo test 2>&1 | tail -30`
Expected: All tests PASS

**Step 2: Run frontend checks**

Run: `npx svelte-check --threshold error 2>&1 | tail -20`
Expected: No errors

**Step 3: Run frontend tests**

Run: `npx vitest run 2>&1 | tail -20`
Expected: All tests PASS

**Step 4: Commit any remaining fixes**

If any tests fail, fix and commit.
