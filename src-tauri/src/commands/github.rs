use tauri::State;

use crate::models::{AssignedIssue, GithubIssue};
use crate::state::AppState;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct WorkerReport {
    pub issue_number: u64,
    pub title: String,
    pub comment_body: String,
    pub updated_at: String,
}

const WORKER_REPORT_FALLBACK_BODY: &str = "No worker report was posted for this issue.";
const LABEL_ASSIGNED_TO_AUTO_WORKER: &str = "assigned-to-auto-worker";

/// Parse a GitHub remote URL into an "owner/repo" string.
/// Handles SSH (git@github.com:owner/repo.git), HTTPS, and HTTP URLs.
fn parse_github_nwo(url: &str) -> Result<String, String> {
    // SSH: git@github.com:owner/repo.git
    if let Some(rest) = url.strip_prefix("git@github.com:") {
        return Ok(rest.trim_end_matches(".git").to_string());
    }
    // HTTPS/HTTP: https://github.com/owner/repo.git
    if let Some(rest) = url
        .strip_prefix("https://github.com/")
        .or_else(|| url.strip_prefix("http://github.com/"))
    {
        return Ok(rest.trim_end_matches(".git").to_string());
    }

    Err(format!("Not a GitHub remote URL: {}", url))
}

/// Parse a GitHub issue URL like "https://github.com/owner/repo/issues/42" and return the issue number.
fn parse_github_issue_url(url: &str) -> Result<u64, String> {
    let url = url.trim();
    let parts: Vec<&str> = url.rsplitn(2, '/').collect();
    if parts.len() == 2 {
        if let Ok(num) = parts[0].parse::<u64>() {
            return Ok(num);
        }
    }
    Err(format!("Could not parse issue number from URL: {}", url))
}

/// Extract the GitHub owner/repo from a local git repository's origin remote.
/// Handles both SSH (git@github.com:owner/repo.git) and HTTPS (https://github.com/owner/repo.git) URLs.
fn extract_github_repo(repo_path: &str) -> Result<String, String> {
    let repo =
        git2::Repository::discover(repo_path).map_err(|e| format!("Failed to open repo: {}", e))?;
    let remote = repo
        .find_remote("origin")
        .map_err(|_| "No 'origin' remote found".to_string())?;
    let url = remote
        .url()
        .ok_or_else(|| "Origin remote URL is not valid UTF-8".to_string())?;

    parse_github_nwo(url)
}

async fn extract_github_repo_async(repo_path: String) -> Result<String, String> {
    tokio::task::spawn_blocking(move || extract_github_repo(&repo_path))
        .await
        .map_err(|e| format!("Task failed: {}", e))?
}

async fn fetch_github_issues(repo_path: String) -> Result<Vec<GithubIssue>, String> {
    let nwo = extract_github_repo_async(repo_path).await?;

    tracing::debug!(repo = %nwo, "fetching issues via gh issue list");
    let output = tokio::process::Command::new("gh")
        .args([
            "issue",
            "list",
            "--repo",
            &nwo,
            "--json",
            "number,title,url,body,labels",
            "--limit",
            "50",
        ])
        .output()
        .await
        .map_err(|e| {
            tracing::error!(repo = %nwo, error = %e, "failed to spawn gh process");
            format!("Failed to run gh: {}", e)
        })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        if stderr.contains("rate limit") || stderr.contains("403") {
            tracing::warn!(repo = %nwo, "GitHub API rate limit detected");
        }
        tracing::error!(repo = %nwo, stderr = %stderr, "gh issue list failed");
        return Err(format!("gh issue list failed: {}", stderr));
    }

    let issues: Vec<GithubIssue> = serde_json::from_slice(&output.stdout).map_err(|e| {
        tracing::error!(repo = %nwo, error = %e, "failed to parse gh issue list output");
        format!("Failed to parse gh output: {}", e)
    })?;

    tracing::debug!(repo = %nwo, count = issues.len(), "fetched issues");
    Ok(issues)
}

pub(crate) async fn list_github_issues(
    repo_path: String,
    state: State<'_, AppState>,
) -> Result<Vec<GithubIssue>, String> {
    // Check cache (lock is dropped at end of block before any .await)
    let cache_result = {
        let cache = state
            .issue_cache
            .lock()
            .map_err(|e| format!("Cache lock error: {}", e))?;
        match cache.get(&repo_path) {
            Some(entry) if entry.is_fresh() => {
                tracing::debug!(repo = %repo_path, "issue cache hit (fresh)");
                return Ok(entry.issues.clone());
            }
            Some(entry) => {
                tracing::debug!(repo = %repo_path, "issue cache hit (stale), refreshing in background");
                // Stale hit: return stale data and refresh in background
                Some(entry.issues.clone())
            }
            None => {
                tracing::debug!(repo = %repo_path, "issue cache miss");
                None
            }
        }
    };

    if let Some(stale_issues) = cache_result {
        // Spawn background refresh
        let cache_arc = state.issue_cache.clone();
        let repo_path_bg = repo_path.clone();
        tokio::spawn(async move {
            if let Ok(fresh_issues) = fetch_github_issues(repo_path_bg.clone()).await {
                if let Ok(mut cache) = cache_arc.lock() {
                    cache.insert(repo_path_bg, fresh_issues);
                }
            }
        });
        return Ok(stale_issues);
    }

    // Cache miss: fetch, cache, and return
    let issues = fetch_github_issues(repo_path.clone()).await?;
    {
        let mut cache = state
            .issue_cache
            .lock()
            .map_err(|e| format!("Cache lock error: {}", e))?;
        cache.insert(repo_path, issues.clone());
    }
    Ok(issues)
}

pub(crate) async fn generate_issue_body(title: String) -> Result<String, String> {
    tracing::debug!("generating issue body via claude CLI");
    let prompt = format!(
        "Write a concise GitHub issue body for an issue titled: \"{}\". \
         Include a Summary section and a Details section. \
         Keep it under 200 words. Return only the markdown body, nothing else.",
        title
    );
    let output = tokio::process::Command::new("claude")
        .args(["--print", &prompt])
        .env_remove("CLAUDECODE")
        .output()
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "failed to spawn claude CLI");
            format!("Failed to run claude: {}", e)
        })?;

    if output.status.success() {
        tracing::debug!("claude CLI generated issue body");
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    } else {
        tracing::warn!("claude CLI returned non-zero exit, using empty body");
        Ok(String::new())
    }
}

pub(crate) async fn create_github_issue(
    state: State<'_, AppState>,
    repo_path: String,
    title: String,
    body: String,
) -> Result<GithubIssue, String> {
    let nwo = extract_github_repo_async(repo_path.clone()).await?;

    tracing::info!(repo = %nwo, title = %title, "creating GitHub issue");
    let output = tokio::process::Command::new("gh")
        .args([
            "issue", "create", "--repo", &nwo, "--title", &title, "--body", &body,
        ])
        .output()
        .await
        .map_err(|e| {
            tracing::error!(repo = %nwo, error = %e, "failed to spawn gh for issue create");
            format!("Failed to run gh: {}", e)
        })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        if stderr.contains("rate limit") || stderr.contains("403") {
            tracing::warn!(repo = %nwo, "GitHub API rate limit detected during issue create");
        }
        tracing::error!(repo = %nwo, stderr = %stderr, "gh issue create failed");
        return Err(format!("gh issue create failed: {}", stderr));
    }

    let url = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let number = parse_github_issue_url(&url)?;

    tracing::info!(repo = %nwo, issue_number = number, "created GitHub issue");

    let issue = GithubIssue {
        number,
        title,
        url,
        body: Some(body),
        labels: vec![],
    };

    if let Ok(mut cache) = state.issue_cache.lock() {
        cache.add_issue(&repo_path, issue.clone());
    }

    Ok(issue)
}

pub(crate) async fn post_github_comment(
    repo_path: String,
    issue_number: u64,
    body: String,
) -> Result<(), String> {
    let nwo = extract_github_repo_async(repo_path).await?;

    tracing::debug!(repo = %nwo, issue_number, "posting comment on issue");
    let output = tokio::process::Command::new("gh")
        .args([
            "issue",
            "comment",
            &issue_number.to_string(),
            "--repo",
            &nwo,
            "--body",
            &body,
        ])
        .output()
        .await
        .map_err(|e| {
            tracing::error!(repo = %nwo, issue_number, error = %e, "failed to spawn gh for comment");
            format!("Failed to run gh: {}", e)
        })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        tracing::error!(repo = %nwo, issue_number, stderr = %stderr, "gh issue comment failed");
        return Err(format!("gh issue comment failed: {}", stderr));
    }

    tracing::debug!(repo = %nwo, issue_number, "comment posted");
    Ok(())
}

pub(crate) async fn add_github_label(
    state: State<'_, AppState>,
    repo_path: String,
    issue_number: u64,
    label: String,
    description: Option<String>,
    color: Option<String>,
) -> Result<(), String> {
    let nwo = extract_github_repo_async(repo_path.clone()).await?;

    let desc = description
        .as_deref()
        .unwrap_or("Issue is being worked on in a session");
    let col = color.as_deref().unwrap_or("F9E2AF");

    tracing::debug!(repo = %nwo, label = %label, "ensuring label exists on repo");
    // Ensure the label exists on the repo (ignore errors if it already exists)
    let _ = tokio::process::Command::new("gh")
        .args([
            "label",
            "create",
            &label,
            "--repo",
            &nwo,
            "--description",
            desc,
            "--color",
            col,
        ])
        .output()
        .await;

    tracing::debug!(repo = %nwo, issue_number, label = %label, "adding label to issue");
    let output = tokio::process::Command::new("gh")
        .args([
            "issue",
            "edit",
            &issue_number.to_string(),
            "--repo",
            &nwo,
            "--add-label",
            &label,
        ])
        .output()
        .await
        .map_err(|e| {
            tracing::error!(repo = %nwo, issue_number, error = %e, "failed to spawn gh for add label");
            format!("Failed to run gh: {}", e)
        })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        tracing::error!(repo = %nwo, issue_number, label = %label, stderr = %stderr, "gh issue edit (add label) failed");
        return Err(format!("gh issue edit failed: {}", stderr));
    }

    tracing::debug!(repo = %nwo, issue_number, label = %label, "label added");
    if let Ok(mut cache) = state.issue_cache.lock() {
        cache.add_label(&repo_path, issue_number, &label);
    }

    Ok(())
}

pub(crate) async fn remove_github_label(
    state: State<'_, AppState>,
    repo_path: String,
    issue_number: u64,
    label: String,
) -> Result<(), String> {
    let nwo = extract_github_repo_async(repo_path.clone()).await?;

    tracing::debug!(repo = %nwo, issue_number, label = %label, "removing label from issue");
    let output = tokio::process::Command::new("gh")
        .args([
            "issue",
            "edit",
            &issue_number.to_string(),
            "--repo",
            &nwo,
            "--remove-label",
            &label,
        ])
        .output()
        .await
        .map_err(|e| {
            tracing::error!(repo = %nwo, issue_number, error = %e, "failed to spawn gh for remove label");
            format!("Failed to run gh: {}", e)
        })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        tracing::error!(repo = %nwo, issue_number, label = %label, stderr = %stderr, "gh issue edit (remove label) failed");
        return Err(format!("gh issue edit failed: {}", stderr));
    }

    tracing::debug!(repo = %nwo, issue_number, label = %label, "label removed");
    if let Ok(mut cache) = state.issue_cache.lock() {
        cache.remove_label(&repo_path, issue_number, &label);
    }

    Ok(())
}

pub(crate) async fn close_github_issue(
    state: State<'_, AppState>,
    repo_path: String,
    issue_number: u64,
    comment: String,
) -> Result<(), String> {
    let nwo = extract_github_repo_async(repo_path.clone()).await?;

    let mut args = vec![
        "issue".to_string(),
        "close".to_string(),
        issue_number.to_string(),
        "--repo".to_string(),
        nwo,
    ];

    if !comment.trim().is_empty() {
        args.push("--comment".to_string());
        args.push(comment);
    }

    let output = tokio::process::Command::new("gh")
        .args(&args)
        .output()
        .await
        .map_err(|e| format!("Failed to run gh: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("gh issue close failed: {}", stderr));
    }

    // Remove from cache since list only shows open issues
    if let Ok(mut cache) = state.issue_cache.lock() {
        cache.remove_issue(&repo_path, issue_number);
    }

    Ok(())
}

pub(crate) async fn delete_github_issue(
    state: State<'_, AppState>,
    repo_path: String,
    issue_number: u64,
) -> Result<(), String> {
    let nwo = extract_github_repo_async(repo_path.clone()).await?;

    let output = tokio::process::Command::new("gh")
        .args([
            "issue",
            "delete",
            &issue_number.to_string(),
            "--repo",
            &nwo,
            "--yes",
        ])
        .output()
        .await
        .map_err(|e| format!("Failed to run gh: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("gh issue delete failed: {}", stderr));
    }

    if let Ok(mut cache) = state.issue_cache.lock() {
        cache.remove_issue(&repo_path, issue_number);
    }

    Ok(())
}

pub(crate) async fn get_maintainer_issues(
    repo_path: String,
    github_repo: Option<String>,
) -> Result<Vec<crate::models::MaintainerIssue>, String> {
    let nwo = match github_repo {
        Some(ref repo) if !repo.is_empty() => repo.clone(),
        _ => extract_github_repo_async(repo_path).await?,
    };

    tracing::debug!(repo = %nwo, "fetching maintainer issues");
    let output = tokio::process::Command::new("gh")
        .args([
            "issue",
            "list",
            "--repo",
            &nwo,
            "--label",
            "filed-by-maintainer",
            "--state",
            "all",
            "--json",
            "number,title,state,url,labels,createdAt,closedAt",
            "--limit",
            "100",
        ])
        .output()
        .await
        .map_err(|e| {
            tracing::error!(repo = %nwo, error = %e, "failed to spawn gh for maintainer issues");
            format!("Failed to run gh: {}", e)
        })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        tracing::error!(repo = %nwo, stderr = %stderr, "gh issue list (maintainer) failed");
        return Err(format!("gh issue list failed: {}", stderr));
    }

    let issues: Vec<crate::models::MaintainerIssue> = serde_json::from_slice(&output.stdout)
        .map_err(|e| {
            tracing::error!(repo = %nwo, error = %e, "failed to parse maintainer issues");
            format!("Failed to parse gh output: {}", e)
        })?;

    tracing::debug!(repo = %nwo, count = issues.len(), "fetched maintainer issues");
    Ok(issues)
}

pub(crate) async fn get_maintainer_issue_detail(
    repo_path: String,
    github_repo: Option<String>,
    issue_number: u32,
) -> Result<crate::models::MaintainerIssueDetail, String> {
    let nwo = match github_repo {
        Some(ref repo) if !repo.is_empty() => repo.clone(),
        _ => extract_github_repo_async(repo_path).await?,
    };

    tracing::debug!(repo = %nwo, issue_number, "fetching maintainer issue detail");
    let output = tokio::process::Command::new("gh")
        .args([
            "issue",
            "view",
            &issue_number.to_string(),
            "--repo",
            &nwo,
            "--json",
            "number,title,state,body,url,labels,createdAt,closedAt",
        ])
        .output()
        .await
        .map_err(|e| {
            tracing::error!(repo = %nwo, issue_number, error = %e, "failed to spawn gh for issue detail");
            format!("Failed to run gh: {}", e)
        })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        tracing::error!(repo = %nwo, issue_number, stderr = %stderr, "gh issue view failed");
        return Err(format!("gh issue view failed: {}", stderr));
    }

    let detail: crate::models::MaintainerIssueDetail = serde_json::from_slice(&output.stdout)
        .map_err(|e| {
            tracing::error!(repo = %nwo, issue_number, error = %e, "failed to parse issue detail");
            format!("Failed to parse gh output: {}", e)
        })?;

    tracing::debug!(repo = %nwo, issue_number, "fetched maintainer issue detail");
    Ok(detail)
}

pub(crate) async fn list_assigned_issues(repo_path: String) -> Result<Vec<AssignedIssue>, String> {
    let nwo = extract_github_repo_async(repo_path).await?;

    tracing::debug!(repo = %nwo, "fetching assigned issues");
    let output = tokio::process::Command::new("gh")
        .args([
            "issue",
            "list",
            "--repo",
            &nwo,
            "--json",
            "number,title,url,assignees,updatedAt,labels",
            "--limit",
            "100",
        ])
        .output()
        .await
        .map_err(|e| {
            tracing::error!(repo = %nwo, error = %e, "failed to spawn gh for assigned issues");
            format!("Failed to run gh: {}", e)
        })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        tracing::error!(repo = %nwo, stderr = %stderr, "gh issue list (assigned) failed");
        return Err(format!("gh issue list failed: {}", stderr));
    }

    let all_issues: Vec<AssignedIssue> = serde_json::from_slice(&output.stdout).map_err(|e| {
        tracing::error!(repo = %nwo, error = %e, "failed to parse assigned issues");
        format!("Failed to parse gh output: {}", e)
    })?;

    // Filter to only issues that have at least one assignee
    let assigned: Vec<AssignedIssue> = all_issues
        .into_iter()
        .filter(|issue| !issue.assignees.is_empty())
        .collect();

    tracing::debug!(repo = %nwo, count = assigned.len(), "fetched assigned issues");
    Ok(assigned)
}

pub(crate) async fn get_worker_reports(repo_path: String) -> Result<Vec<WorkerReport>, String> {
    let nwo = extract_github_repo_async(repo_path).await?;

    tracing::debug!(repo = %nwo, "fetching worker reports");
    let output = tokio::process::Command::new("gh")
        .args([
            "issue",
            "list",
            "--repo",
            &nwo,
            "--label",
            LABEL_ASSIGNED_TO_AUTO_WORKER,
            "--state",
            "all",
            "--json",
            "number,title,state,comments,updatedAt",
            "--limit",
            "50",
        ])
        .output()
        .await
        .map_err(|e| {
            tracing::error!(repo = %nwo, error = %e, "failed to spawn gh for worker reports");
            format!("Failed to run gh: {}", e)
        })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        tracing::error!(repo = %nwo, stderr = %stderr, "gh issue list (worker reports) failed");
        return Err(format!("gh issue list failed: {}", stderr));
    }

    let raw: Vec<serde_json::Value> = serde_json::from_slice(&output.stdout).map_err(|e| {
        tracing::error!(repo = %nwo, error = %e, "failed to parse worker reports");
        format!("Failed to parse gh output: {}", e)
    })?;

    let reports = parse_worker_reports(raw);
    tracing::debug!(repo = %nwo, count = reports.len(), "fetched worker reports");

    Ok(reports)
}

fn parse_worker_reports(raw: Vec<serde_json::Value>) -> Vec<WorkerReport> {
    raw.into_iter()
        .filter_map(|issue| {
            if issue["state"].as_str() != Some("CLOSED") {
                return None;
            }
            let number = issue["number"].as_u64()?;
            let title = issue["title"].as_str()?.to_string();
            let updated_at = issue["updatedAt"].as_str().unwrap_or("").to_string();
            let body = issue["comments"]
                .as_array()
                .and_then(|comments| {
                    comments.iter().rev().find_map(|c| {
                        let text = c["body"].as_str()?;
                        text.contains("<!-- auto-worker-report -->").then_some(text)
                    })
                })
                .unwrap_or(WORKER_REPORT_FALLBACK_BODY)
                .to_string();
            Some(WorkerReport {
                issue_number: number,
                title,
                comment_body: body,
                updated_at,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_github_nwo_ssh() {
        assert_eq!(
            parse_github_nwo("git@github.com:owner/repo.git").unwrap(),
            "owner/repo"
        );
    }

    #[test]
    fn test_parse_github_nwo_https() {
        assert_eq!(
            parse_github_nwo("https://github.com/owner/repo.git").unwrap(),
            "owner/repo"
        );
    }

    #[test]
    fn test_parse_github_nwo_https_no_git_suffix() {
        assert_eq!(
            parse_github_nwo("https://github.com/owner/repo").unwrap(),
            "owner/repo"
        );
    }

    #[test]
    fn test_parse_github_nwo_http() {
        assert_eq!(
            parse_github_nwo("http://github.com/owner/repo.git").unwrap(),
            "owner/repo"
        );
    }

    #[test]
    fn test_parse_github_nwo_non_github_url() {
        let result = parse_github_nwo("https://gitlab.com/owner/repo.git");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Not a GitHub remote URL"));
    }

    #[test]
    fn test_parse_github_issue_url_basic() {
        assert_eq!(
            parse_github_issue_url("https://github.com/owner/repo/issues/42").unwrap(),
            42
        );
    }

    #[test]
    fn test_parse_github_issue_url_trailing_newline() {
        assert_eq!(
            parse_github_issue_url("https://github.com/owner/repo/issues/7\n").unwrap(),
            7
        );
    }

    #[test]
    fn test_parse_github_issue_url_invalid() {
        assert!(parse_github_issue_url("not a url").is_err());
    }

    #[test]
    fn test_parse_github_nwo_ssh_no_git_suffix() {
        assert_eq!(
            parse_github_nwo("git@github.com:owner/repo").unwrap(),
            "owner/repo"
        );
    }

    #[test]
    fn test_parse_github_nwo_empty_string() {
        assert!(parse_github_nwo("").is_err());
    }

    #[test]
    fn test_parse_github_issue_url_large_number() {
        assert_eq!(
            parse_github_issue_url("https://github.com/owner/repo/issues/99999").unwrap(),
            99999
        );
    }

    #[test]
    fn test_parse_github_issue_url_zero() {
        assert_eq!(
            parse_github_issue_url("https://github.com/owner/repo/issues/0").unwrap(),
            0
        );
    }

    #[test]
    fn test_parse_github_issue_url_empty() {
        assert!(parse_github_issue_url("").is_err());
    }

    #[test]
    fn parse_worker_reports_excludes_open_issues() {
        let reports = parse_worker_reports(vec![
            serde_json::json!({
                "number": 42,
                "title": "Closed worker issue",
                "state": "CLOSED",
                "updatedAt": "2026-03-10T00:00:00Z",
                "comments": [],
            }),
            serde_json::json!({
                "number": 43,
                "title": "Open worker issue",
                "state": "OPEN",
                "updatedAt": "2026-03-10T00:00:00Z",
                "comments": [],
            }),
        ]);

        assert_eq!(reports.len(), 1);
        assert_eq!(reports[0].issue_number, 42);
    }

    #[test]
    fn parse_worker_reports_uses_fallback_body_when_report_comment_missing() {
        let reports = parse_worker_reports(vec![serde_json::json!({
            "number": 42,
            "title": "Closed worker issue",
            "state": "CLOSED",
            "updatedAt": "2026-03-10T00:00:00Z",
            "comments": [],
        })]);

        assert_eq!(reports[0].comment_body, WORKER_REPORT_FALLBACK_BODY);
    }
}
