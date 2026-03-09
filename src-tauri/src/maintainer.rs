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
                            let _ = app_handle.emit(
                                &format!("maintainer-error:{}", project.id),
                                e.to_string(),
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
gh label create "priority:low" --description "Low priority" --color "a6e3a1" --force{repo_flag}
gh label create "priority:high" --description "High priority" --color "f38ba8" --force{repo_flag}
gh label create "complexity:simple" --description "Simple fix" --color "89b4fa" --force{repo_flag}
gh label create "complexity:high" --description "Significant effort" --color "f9e2af" --force{repo_flag}

## Step 2: Check existing issues

gh issue list --label filed-by-maintainer --state open --json number,title,body,labels{repo_flag}

## Step 3: Analyze and act

For each finding:
1. Determine if it semantically matches an existing open `filed-by-maintainer` issue (same underlying problem, not just string match)
2. If it matches: update the issue body with current analysis via `gh issue edit <number> --body "..."{repo_flag}`, then add a comment noting what changed via `gh issue comment <number> --body "..."{repo_flag}`
3. If no match: file a new issue via `gh issue create --title "..." --body "..." --label filed-by-maintainer --label <priority> --label <complexity>{repo_flag}`

Each issue should be specific and actionable (one issue per finding, not grouped).

Assign priority:low or priority:high based on impact.
Assign complexity:simple or complexity:high based on effort.

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
        .arg("--allowedTools=Bash")
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
      "labels": ["filed-by-maintainer", "priority:high", "complexity:simple"]
    }
  ],
  "issues_updated": [
    {
      "issue_number": 10,
      "title": "Improve error handling",
      "url": "https://github.com/owner/repo/issues/10",
      "labels": ["filed-by-maintainer", "priority:low"]
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
