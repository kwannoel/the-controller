use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant};

use chrono::Utc;
use serde::Deserialize;
use tauri::{AppHandle, Emitter, Manager};
use uuid::Uuid;

use crate::models::{IssueAction, IssueSummary, MaintainerRunLog};
use crate::state::AppState;

const DEDUP_SIMILARITY_THRESHOLD: f32 = 0.6;
const MIN_FINGERPRINT_TOKENS: usize = 3;

const STOPWORDS: &[&str] = &[
    "a",
    "an",
    "and",
    "are",
    "as",
    "at",
    "be",
    "by",
    "for",
    "from",
    "in",
    "into",
    "is",
    "it",
    "of",
    "on",
    "or",
    "that",
    "the",
    "this",
    "to",
    "with",
    "when",
    "during",
    "issue",
    "issues",
    "problem",
    "fix",
    "needs",
    "need",
    "code",
    "project",
    "maintainer",
    "filed",
    "update",
    "updated",
    "new",
    "run",
    "agent",
    "src",
    "lib",
    "tauri",
    "rs",
    "svelte",
];

#[derive(Debug, Clone, Deserialize)]
struct CandidateFinding {
    title: String,
    body: String,
    #[serde(default)]
    priority: String,
    #[serde(default)]
    complexity: String,
    #[serde(default)]
    affected_files: Vec<String>,
    #[serde(default)]
    symptom_type: String,
    #[serde(default)]
    keywords: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct RawFindingsOutput {
    findings: Vec<CandidateFinding>,
    #[serde(default)]
    summary: String,
}

#[derive(Debug)]
struct FindingsOutput {
    findings: Vec<CandidateFinding>,
    summary: String,
}

#[derive(Debug, Clone)]
struct ExistingIssue {
    number: u32,
    title: String,
    body: String,
    url: String,
    labels: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct RawExistingIssue {
    number: u32,
    title: String,
    #[serde(default)]
    body: String,
    url: String,
    #[serde(default)]
    labels: Vec<RawIssueLabel>,
}

#[derive(Debug, Deserialize)]
struct RawIssueLabel {
    name: String,
}

#[derive(Debug, Clone)]
struct DuplicateMatch {
    issue: ExistingIssue,
    similarity: f32,
}

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

                    let interval = Duration::from_secs(project.maintainer.interval_minutes * 60);
                    let should_run = last_run
                        .get(&project.id)
                        .map_or(true, |t| t.elapsed() >= interval);

                    if !should_run {
                        continue;
                    }

                    last_run.insert(project.id, Instant::now());

                    let _ =
                        app_handle.emit(&format!("maintainer-status:{}", project.id), "running");

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

                            let _ = app_handle
                                .emit(&format!("maintainer-status:{}", project.id), "idle");
                        }
                        Err(e) => {
                            eprintln!("Maintainer check failed for {}: {}", project.name, e);
                            let _ = app_handle
                                .emit(&format!("maintainer-status:{}", project.id), "error");
                            let _ = app_handle
                                .emit(&format!("maintainer-error:{}", project.id), e.to_string());
                        }
                    }
                }
            }
        });
    }
}

pub fn build_issue_filing_prompt(repo_path: &str, _github_repo: Option<&str>) -> String {
    format!(
        r#"You are a maintainer agent. Analyze the project at {repo_path} for code quality, test robustness, architecture, and documentation issues.

IMPORTANT:
- Do NOT run any `gh issue create`, `gh issue edit`, or `gh issue comment` commands.
- The deterministic issue dedup and issue filing/update logic is handled by the Rust pipeline.
- Return findings only in structured JSON.

For each finding, provide:
- `title`: short actionable issue title
- `body`: markdown body with concrete evidence and remediation
- `priority`: `high` or `low` (see criteria below)
- `complexity`: `high` or `simple`
- `affected_files`: list of file paths (when applicable)
- `symptom_type`: short invariant symptom phrase (e.g. `startup panic`, `missing coverage`)
- `keywords`: stable semantic keywords (e.g. `appstate`, `filesystem`, `initialization`)

PRIORITY CRITERIA — most findings should be low priority:
- `high`: Affects correctness, reliability, or data integrity. Examples: logic bugs, panics, race conditions, data loss, security vulnerabilities, broken core functionality.
- `low`: Everything else. Examples: missing tests, style issues, documentation gaps, minor code quality improvements, refactoring opportunities, non-critical TODOs.

When in doubt, use `low`. Reserve `high` for issues that could cause wrong behavior or failures in production.

Return ONLY this JSON object:

```json
{{
  "findings": [
    {{
      "title": "<title>",
      "body": "<markdown body>",
      "priority": "low",
      "complexity": "simple",
      "affected_files": ["<path>"],
      "symptom_type": "<symptom>",
      "keywords": ["<keyword>"]
    }}
  ],
  "summary": "<one-line summary>"
}}
```

If no issues are found, return:

```json
{{
  "findings": [],
  "summary": "No actionable maintainer issues found"
}}
```
"#
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

fn parse_findings_output(output: &str) -> Result<FindingsOutput, String> {
    let json_str = extract_json(output).ok_or("No JSON found in output")?;
    let raw: RawFindingsOutput =
        serde_json::from_str(json_str).map_err(|e| format!("Failed to parse JSON: {}", e))?;

    let mut findings = Vec::with_capacity(raw.findings.len());
    for (idx, finding) in raw.findings.into_iter().enumerate() {
        let sanitized =
            sanitize_finding(finding).ok_or_else(|| format!("Invalid finding at index {}", idx))?;
        findings.push(sanitized);
    }

    Ok(FindingsOutput {
        findings,
        summary: raw.summary,
    })
}

fn sanitize_finding(finding: CandidateFinding) -> Option<CandidateFinding> {
    let title = finding.title.trim();
    let body = finding.body.trim();
    if title.is_empty() || body.is_empty() {
        return None;
    }

    Some(CandidateFinding {
        title: title.to_string(),
        body: body.to_string(),
        priority: normalize_priority(&finding.priority),
        complexity: normalize_complexity(&finding.complexity),
        affected_files: finding
            .affected_files
            .into_iter()
            .map(|p| p.trim().to_string())
            .filter(|p| !p.is_empty())
            .collect(),
        symptom_type: finding.symptom_type.trim().to_string(),
        keywords: finding
            .keywords
            .into_iter()
            .map(|k| k.trim().to_string())
            .filter(|k| !k.is_empty())
            .collect(),
    })
}

fn normalize_priority(priority: &str) -> String {
    let p = priority.to_ascii_lowercase();
    if p.contains("high") {
        "high".to_string()
    } else {
        "low".to_string()
    }
}

fn normalize_complexity(complexity: &str) -> String {
    let c = complexity.to_ascii_lowercase();
    if c.contains("high") {
        "high".to_string()
    } else {
        "simple".to_string()
    }
}

fn normalize_priority_label(priority: &str) -> String {
    format!("priority:{}", normalize_priority(priority))
}

fn normalize_complexity_label(complexity: &str) -> String {
    format!("complexity:{}", normalize_complexity(complexity))
}

fn normalize_tokens(input: &str) -> Vec<String> {
    let lowered = input
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() {
                ch.to_ascii_lowercase()
            } else {
                ' '
            }
        })
        .collect::<String>();

    lowered
        .split_whitespace()
        .filter(|token| token.len() >= 2)
        .filter(|token| !STOPWORDS.contains(token))
        .map(|token| token.to_string())
        .collect()
}

fn unique_sorted_tokens(mut tokens: Vec<String>) -> Vec<String> {
    tokens.sort();
    tokens.dedup();
    tokens
}

fn finding_fingerprint_tokens(finding: &CandidateFinding) -> Vec<String> {
    let mut tokens = Vec::new();

    for file in &finding.affected_files {
        tokens.extend(normalize_tokens(file));
    }
    tokens.extend(normalize_tokens(&finding.symptom_type));

    for keyword in &finding.keywords {
        tokens.extend(normalize_tokens(keyword));
    }

    if tokens.is_empty() {
        tokens.extend(normalize_tokens(&finding.title));
        tokens.extend(normalize_tokens(&finding.body));
    }

    unique_sorted_tokens(tokens)
}

fn build_finding_fingerprint(finding: &CandidateFinding) -> String {
    finding_fingerprint_tokens(finding).join("|")
}

fn extract_embedded_fingerprint_tokens(body: &str) -> Option<Vec<String>> {
    for line in body.lines() {
        let lowercase = line.to_ascii_lowercase();
        if !lowercase.contains("fingerprint") {
            continue;
        }

        if let Some(start) = line.find('`') {
            if let Some(end_rel) = line[start + 1..].find('`') {
                let raw = &line[start + 1..start + 1 + end_rel];
                let tokens = unique_sorted_tokens(
                    raw.split(|ch: char| ch == '|' || ch == ',' || ch.is_whitespace())
                        .flat_map(normalize_tokens)
                        .collect(),
                );
                if !tokens.is_empty() {
                    return Some(tokens);
                }
            }
        }

        if let Some(colon_idx) = line.find(':') {
            let raw = line[colon_idx + 1..].trim();
            let tokens = unique_sorted_tokens(
                raw.split(|ch: char| ch == '|' || ch == ',' || ch.is_whitespace())
                    .flat_map(normalize_tokens)
                    .collect(),
            );
            if !tokens.is_empty() {
                return Some(tokens);
            }
        }
    }

    None
}

fn existing_issue_tokens(issue: &ExistingIssue) -> Vec<String> {
    if let Some(tokens) = extract_embedded_fingerprint_tokens(&issue.body) {
        return tokens;
    }

    let mut tokens = normalize_tokens(&issue.title);
    tokens.extend(normalize_tokens(&issue.body));
    unique_sorted_tokens(tokens)
}

fn similarity_score(candidate_tokens: &[String], existing_tokens: &[String]) -> f32 {
    if candidate_tokens.is_empty() || existing_tokens.is_empty() {
        return 0.0;
    }

    let existing_set = existing_tokens.iter().collect::<HashSet<_>>();
    let overlap = candidate_tokens
        .iter()
        .filter(|token| existing_set.contains(token))
        .count();

    overlap as f32 / candidate_tokens.len() as f32
}

fn find_duplicate_issue(
    finding: &CandidateFinding,
    existing_issues: &[ExistingIssue],
    threshold: f32,
) -> Option<DuplicateMatch> {
    let candidate_tokens = finding_fingerprint_tokens(finding);
    if candidate_tokens.len() < MIN_FINGERPRINT_TOKENS {
        return None;
    }
    let mut best: Option<DuplicateMatch> = None;

    for issue in existing_issues {
        let issue_tokens = existing_issue_tokens(issue);
        let similarity = similarity_score(&candidate_tokens, &issue_tokens);
        if similarity < threshold {
            continue;
        }

        let should_replace = match &best {
            None => true,
            Some(current) => {
                similarity > current.similarity
                    || ((similarity - current.similarity).abs() < f32::EPSILON
                        && issue.number < current.issue.number)
            }
        };

        if should_replace {
            best = Some(DuplicateMatch {
                issue: issue.clone(),
                similarity,
            });
        }
    }

    best
}

fn format_issue_body(finding: &CandidateFinding, fingerprint: &str) -> String {
    let affected_files = if finding.affected_files.is_empty() {
        "(none provided)".to_string()
    } else {
        finding.affected_files.join(", ")
    };

    let keywords = if finding.keywords.is_empty() {
        "(none provided)".to_string()
    } else {
        finding.keywords.join(", ")
    };

    let symptom_type = if finding.symptom_type.trim().is_empty() {
        "unspecified".to_string()
    } else {
        finding.symptom_type.trim().to_string()
    };

    format!(
        "## Summary\n\n{}\n\n## Maintainer Metadata\n\n- Fingerprint: `{}`\n- Symptom Type: `{}`\n- Affected Files: {}\n- Keywords: {}\n",
        finding.body.trim(),
        fingerprint,
        symptom_type,
        affected_files,
        keywords
    )
}

fn format_update_comment(finding: &CandidateFinding, similarity: f32, fingerprint: &str) -> String {
    format!(
        "Maintainer rerun matched this issue as a semantic duplicate (similarity {:.2}). Updated with the latest analysis for **{}**.\n\nFingerprint: `{}`",
        similarity,
        finding.title,
        fingerprint
    )
}

fn gh_command(repo_path: &str, github_repo: Option<&str>) -> std::process::Command {
    let mut cmd = std::process::Command::new("gh");
    if let Some(repo) = github_repo {
        cmd.args(["-R", repo]);
    }
    cmd.current_dir(repo_path);
    cmd
}

fn run_gh_checked(
    mut command: std::process::Command,
    failure_prefix: &str,
) -> Result<std::process::Output, String> {
    let output = command
        .output()
        .map_err(|e| format!("Failed to run gh: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("{}: {}", failure_prefix, stderr.trim()));
    }

    Ok(output)
}

fn ensure_labels_exist(repo_path: &str, github_repo: Option<&str>) -> Result<(), String> {
    let labels = [
        (
            "filed-by-maintainer",
            "Issue filed by maintainer agent",
            "6c7086",
        ),
        ("priority:low", "Low priority", "a6e3a1"),
        ("priority:high", "High priority", "f38ba8"),
        ("complexity:simple", "Simple fix", "89b4fa"),
        ("complexity:high", "Significant effort", "f9e2af"),
    ];

    for (label, description, color) in labels {
        let mut cmd = gh_command(repo_path, github_repo);
        cmd.args([
            "label",
            "create",
            label,
            "--description",
            description,
            "--color",
            color,
            "--force",
        ]);
        run_gh_checked(cmd, "gh label create failed")?;
    }

    Ok(())
}

fn list_open_maintainer_issues(
    repo_path: &str,
    github_repo: Option<&str>,
) -> Result<Vec<ExistingIssue>, String> {
    let mut cmd = gh_command(repo_path, github_repo);
    cmd.args([
        "issue",
        "list",
        "--label",
        "filed-by-maintainer",
        "--state",
        "open",
        "--json",
        "number,title,body,url,labels",
        "--limit",
        "200",
    ]);

    let output = run_gh_checked(cmd, "gh issue list failed")?;
    let raw_issues: Vec<RawExistingIssue> = serde_json::from_slice(&output.stdout)
        .map_err(|e| format!("Failed to parse gh issue list output: {}", e))?;

    Ok(raw_issues
        .into_iter()
        .map(|raw| ExistingIssue {
            number: raw.number,
            title: raw.title,
            body: raw.body,
            url: raw.url,
            labels: raw.labels.into_iter().map(|l| l.name).collect(),
        })
        .collect())
}

fn list_closed_maintainer_issues(
    repo_path: &str,
    github_repo: Option<&str>,
) -> Result<Vec<ExistingIssue>, String> {
    let mut cmd = gh_command(repo_path, github_repo);
    cmd.args([
        "issue",
        "list",
        "--label",
        "filed-by-maintainer",
        "--state",
        "closed",
        "--json",
        "number,title,body,url,labels",
        "--limit",
        "200",
    ]);

    let output = run_gh_checked(cmd, "gh issue list (closed) failed")?;
    let raw_issues: Vec<RawExistingIssue> = serde_json::from_slice(&output.stdout)
        .map_err(|e| format!("Failed to parse gh issue list output: {}", e))?;

    Ok(raw_issues
        .into_iter()
        .map(|raw| ExistingIssue {
            number: raw.number,
            title: raw.title,
            body: raw.body,
            url: raw.url,
            labels: raw.labels.into_iter().map(|l| l.name).collect(),
        })
        .collect())
}

fn parse_issue_number_from_url(url: &str) -> Result<u32, String> {
    let trimmed = url.trim().trim_end_matches('/');
    let last = trimmed
        .rsplit('/')
        .next()
        .ok_or_else(|| format!("Could not parse issue number from URL: {}", url))?;
    last.parse::<u32>()
        .map_err(|_| format!("Could not parse issue number from URL: {}", url))
}

fn create_issue(
    repo_path: &str,
    github_repo: Option<&str>,
    finding: &CandidateFinding,
    body: &str,
    labels: &[String],
) -> Result<IssueSummary, String> {
    let mut cmd = gh_command(repo_path, github_repo);
    cmd.arg("issue")
        .arg("create")
        .arg("--title")
        .arg(&finding.title)
        .arg("--body")
        .arg(body);

    for label in labels {
        cmd.arg("--label").arg(label);
    }

    let output = run_gh_checked(cmd, "gh issue create failed")?;
    let url = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let issue_number = parse_issue_number_from_url(&url)?;

    Ok(IssueSummary {
        issue_number,
        title: finding.title.clone(),
        url,
        labels: labels.to_vec(),
        action: IssueAction::Filed,
    })
}

fn update_issue(
    repo_path: &str,
    github_repo: Option<&str>,
    issue_number: u32,
    body: &str,
    labels: &[String],
    labels_to_remove: &[String],
) -> Result<(), String> {
    let issue_number_arg = issue_number.to_string();
    let mut cmd = gh_command(repo_path, github_repo);
    cmd.arg("issue")
        .arg("edit")
        .arg(&issue_number_arg)
        .arg("--body")
        .arg(body);

    for label in labels {
        cmd.arg("--add-label").arg(label);
    }
    for label in labels_to_remove {
        cmd.arg("--remove-label").arg(label);
    }

    run_gh_checked(cmd, "gh issue edit failed")?;
    Ok(())
}

fn comment_issue(
    repo_path: &str,
    github_repo: Option<&str>,
    issue_number: u32,
    body: &str,
) -> Result<(), String> {
    let issue_number_arg = issue_number.to_string();
    let mut cmd = gh_command(repo_path, github_repo);
    cmd.arg("issue")
        .arg("comment")
        .arg(&issue_number_arg)
        .arg("--body")
        .arg(body);

    run_gh_checked(cmd, "gh issue comment failed")?;
    Ok(())
}

fn dedup_labels_for_finding(finding: &CandidateFinding) -> Vec<String> {
    let mut labels = vec![
        "filed-by-maintainer".to_string(),
        normalize_priority_label(&finding.priority),
        normalize_complexity_label(&finding.complexity),
    ];
    labels.sort();
    labels.dedup();
    labels
}

fn labels_to_remove(existing_labels: &[String], desired_labels: &[String]) -> Vec<String> {
    let desired = desired_labels.iter().collect::<HashSet<_>>();
    existing_labels
        .iter()
        .filter(|label| label.starts_with("priority:") || label.starts_with("complexity:"))
        .filter(|label| !desired.contains(label))
        .cloned()
        .collect()
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
    let output = std::process::Command::new("codex")
        .arg("exec")
        .arg("--sandbox")
        .arg("danger-full-access")
        .arg(&prompt)
        .current_dir(repo_path)
        .env_remove("CLAUDECODE")
        .output()
        .map_err(|e| format!("Failed to run codex exec: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("codex exec failed: {}", stderr));
    }

    let findings_output = parse_findings_output(&String::from_utf8_lossy(&output.stdout))?;

    ensure_labels_exist(repo_path, github_repo)?;

    let mut existing_issues = list_open_maintainer_issues(repo_path, github_repo)?;
    let closed_issues = list_closed_maintainer_issues(repo_path, github_repo)?;
    let existing_issue_count = existing_issues.len();

    let mut issues_filed = Vec::new();
    let mut issues_updated = Vec::new();
    let mut updated_issue_numbers = HashSet::new();
    let mut issues_skipped: u32 = 0;

    for finding in &findings_output.findings {
        let fingerprint = build_finding_fingerprint(finding);
        let labels = dedup_labels_for_finding(finding);
        let body = format_issue_body(finding, &fingerprint);

        // Skip findings that match a closed issue (already resolved)
        if find_duplicate_issue(finding, &closed_issues, DEDUP_SIMILARITY_THRESHOLD).is_some() {
            issues_skipped += 1;
            continue;
        }

        if let Some(duplicate_match) =
            find_duplicate_issue(finding, &existing_issues, DEDUP_SIMILARITY_THRESHOLD)
        {
            if updated_issue_numbers.contains(&duplicate_match.issue.number) {
                continue;
            }
            let remove_labels = labels_to_remove(&duplicate_match.issue.labels, &labels);
            update_issue(
                repo_path,
                github_repo,
                duplicate_match.issue.number,
                &body,
                &labels,
                &remove_labels,
            )?;
            comment_issue(
                repo_path,
                github_repo,
                duplicate_match.issue.number,
                &format_update_comment(finding, duplicate_match.similarity, &fingerprint),
            )?;

            updated_issue_numbers.insert(duplicate_match.issue.number);
            issues_updated.push(IssueSummary {
                issue_number: duplicate_match.issue.number,
                title: finding.title.clone(),
                url: duplicate_match.issue.url.clone(),
                labels: labels.clone(),
                action: IssueAction::Updated,
            });

            if let Some(existing_issue) = existing_issues
                .iter_mut()
                .find(|issue| issue.number == duplicate_match.issue.number)
            {
                existing_issue.title = finding.title.clone();
                existing_issue.body = body.clone();
                existing_issue.labels = labels.clone();
            }
        } else {
            let filed = create_issue(repo_path, github_repo, finding, &body, &labels)?;
            existing_issues.push(ExistingIssue {
                number: filed.issue_number,
                title: finding.title.clone(),
                body,
                url: filed.url.clone(),
                labels: labels.clone(),
            });
            issues_filed.push(filed);
        }
    }

    let issues_unchanged = existing_issue_count.saturating_sub(updated_issue_numbers.len()) as u32;

    let summary = if issues_filed.is_empty() && issues_updated.is_empty() {
        if findings_output.summary.trim().is_empty() {
            "No actionable maintainer issues found".to_string()
        } else {
            findings_output.summary
        }
    } else {
        let mut parts = vec![
            format!("Filed {} issue(s)", issues_filed.len()),
            format!("updated {} issue(s)", issues_updated.len()),
            format!("unchanged {}", issues_unchanged),
        ];
        if issues_skipped > 0 {
            parts.push(format!("skipped {} (closed)", issues_skipped));
        }
        parts.join(", ")
    };

    Ok(MaintainerRunLog {
        id: Uuid::new_v4(),
        project_id,
        timestamp: Utc::now().to_rfc3339(),
        issues_filed,
        issues_updated,
        issues_unchanged,
        issues_skipped,
        summary,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_issue_filing_prompt_contains_repo_path() {
        let prompt = build_issue_filing_prompt("/tmp/my-project", None);
        assert!(prompt.contains("/tmp/my-project"));
        assert!(prompt.contains("Return ONLY this JSON object"));
        assert!(prompt.contains("Do NOT run any `gh issue create`"));
    }

    #[test]
    fn test_extract_json_from_fenced_block() {
        let output = "before\n```json\n{\"a\":1}\n```\nafter";
        assert_eq!(extract_json(output), Some("{\"a\":1}"));
    }

    #[test]
    fn test_parse_findings_output_parses_valid_json() {
        let output = r#"```json
{
  "findings": [
    {
      "title": "AppState startup panic",
      "body": "Startup panics when storage init fails.",
      "priority": "high",
      "complexity": "simple",
      "affected_files": ["src-tauri/src/state.rs"],
      "symptom_type": "startup panic",
      "keywords": ["appstate", "filesystem", "initialization"]
    }
  ],
  "summary": "Found one issue"
}
```"#;

        let parsed = parse_findings_output(output).expect("should parse");
        assert_eq!(parsed.findings.len(), 1);
        assert_eq!(parsed.findings[0].priority, "high");
        assert_eq!(parsed.findings[0].complexity, "simple");
        assert_eq!(parsed.summary, "Found one issue");
    }

    #[test]
    fn test_parse_findings_output_invalid_finding_returns_error() {
        let output = r#"{
          "findings": [
            {
              "title": "",
              "body": "",
              "priority": "high",
              "complexity": "simple",
              "affected_files": [],
              "symptom_type": "",
              "keywords": []
            }
          ],
          "summary": "nothing"
        }"#;

        let parsed = parse_findings_output(output);
        assert!(parsed.is_err());
    }

    #[test]
    fn test_parse_findings_output_missing_findings_key_returns_error() {
        let output = r#"{"summary":"missing findings"}"#;
        let parsed = parse_findings_output(output);
        assert!(parsed.is_err());
    }

    #[test]
    fn test_parse_findings_output_invalid_payload_returns_error() {
        let output = "I couldn't analyze the project";
        let result = parse_findings_output(output);
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
            issues_skipped: 0,
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
            issues_skipped: 0,
            summary: "s".to_string(),
        };
        assert!(!has_changes(&log));
    }

    #[test]
    fn test_build_finding_fingerprint_normalizes_and_sorts_tokens() {
        let finding = CandidateFinding {
            title: "AppState startup panic".to_string(),
            body: "App fails at startup when storage init fails.".to_string(),
            priority: "high".to_string(),
            complexity: "simple".to_string(),
            affected_files: vec![
                "src-tauri/src/state.rs".to_string(),
                "src-tauri/src/state.rs".to_string(),
            ],
            symptom_type: "Startup Panic".to_string(),
            keywords: vec![
                "AppState::new".to_string(),
                "filesystem".to_string(),
                "panic".to_string(),
            ],
        };

        let fingerprint = build_finding_fingerprint(&finding);

        assert!(fingerprint.contains("appstate"));
        assert!(fingerprint.contains("startup"));
        assert!(fingerprint.contains("panic"));
        assert!(fingerprint.contains("state"));
        assert!(!fingerprint.contains("::"));
    }

    #[test]
    fn test_find_duplicate_issue_matches_semantic_duplicate_with_mocked_issues() {
        let finding = CandidateFinding {
            title: "AppState startup panic when init fails".to_string(),
            body: "App crashes at startup if filesystem init fails in AppState::new.".to_string(),
            priority: "high".to_string(),
            complexity: "simple".to_string(),
            affected_files: vec!["src-tauri/src/state.rs".to_string()],
            symptom_type: "startup panic".to_string(),
            keywords: vec![
                "appstate".to_string(),
                "filesystem".to_string(),
                "initialization".to_string(),
            ],
        };

        let existing = vec![
            ExistingIssue {
                number: 285,
                title: "AppState::new() panics on startup when storage init fails".to_string(),
                body: "Summary: startup panic in AppState::new during filesystem initialization."
                    .to_string(),
                url: "https://github.com/owner/repo/issues/285".to_string(),
                labels: vec!["filed-by-maintainer".to_string()],
            },
            ExistingIssue {
                number: 266,
                title: "Sidebar.svelte is too large and needs refactor".to_string(),
                body: "Monolith component architecture concern.".to_string(),
                url: "https://github.com/owner/repo/issues/266".to_string(),
                labels: vec!["filed-by-maintainer".to_string()],
            },
        ];

        let duplicate = find_duplicate_issue(&finding, &existing, 0.6);
        assert!(duplicate.is_some());
        let duplicate = duplicate.unwrap();
        assert_eq!(duplicate.issue.number, 285);
        assert!(duplicate.similarity >= 0.6);
    }

    #[test]
    fn test_find_duplicate_issue_returns_none_when_similarity_below_threshold() {
        let finding = CandidateFinding {
            title: "Improve onboarding modal copy".to_string(),
            body: "Minor UX improvements for onboarding instructions.".to_string(),
            priority: "low".to_string(),
            complexity: "simple".to_string(),
            affected_files: vec!["src/lib/Onboarding.svelte".to_string()],
            symptom_type: "copy tweak".to_string(),
            keywords: vec![
                "ux".to_string(),
                "copy".to_string(),
                "onboarding".to_string(),
            ],
        };

        let existing = vec![ExistingIssue {
            number: 267,
            title: "Missing tests for Sidebar.svelte components".to_string(),
            body: "Need better component test coverage for sidebar tree interactions.".to_string(),
            url: "https://github.com/owner/repo/issues/267".to_string(),
            labels: vec!["filed-by-maintainer".to_string()],
        }];

        let duplicate = find_duplicate_issue(&finding, &existing, 0.75);
        assert!(duplicate.is_none());
    }

    #[test]
    fn test_find_duplicate_issue_returns_none_for_sparse_fingerprint() {
        let finding = CandidateFinding {
            title: "panic".to_string(),
            body: "panic".to_string(),
            priority: "high".to_string(),
            complexity: "simple".to_string(),
            affected_files: vec![],
            symptom_type: "panic".to_string(),
            keywords: vec![],
        };

        let existing = vec![ExistingIssue {
            number: 285,
            title: "Startup panic in AppState::new".to_string(),
            body: "Summary panic during startup.".to_string(),
            url: "https://github.com/owner/repo/issues/285".to_string(),
            labels: vec!["filed-by-maintainer".to_string()],
        }];

        let duplicate = find_duplicate_issue(&finding, &existing, 0.6);
        assert!(duplicate.is_none());
    }

    #[test]
    fn test_find_duplicate_issue_uses_embedded_fingerprint_metadata() {
        let finding = CandidateFinding {
            title: "AppState startup panic when init fails".to_string(),
            body: "Body".to_string(),
            priority: "high".to_string(),
            complexity: "simple".to_string(),
            affected_files: vec!["src-tauri/src/state.rs".to_string()],
            symptom_type: "startup panic".to_string(),
            keywords: vec!["appstate".to_string(), "filesystem".to_string()],
        };

        let existing = vec![ExistingIssue {
            number: 277,
            title: "Old title".to_string(),
            body: "## Maintainer Metadata\n- Fingerprint: `appstate|filesystem|panic|src|state|startup|tauri`"
                .to_string(),
            url: "https://github.com/owner/repo/issues/277".to_string(),
            labels: vec!["filed-by-maintainer".to_string()],
        }];

        let duplicate = find_duplicate_issue(&finding, &existing, 0.6).expect("match expected");
        assert_eq!(duplicate.issue.number, 277);
    }

    #[test]
    fn test_find_duplicate_issue_tie_breaks_by_lowest_issue_number() {
        let finding = CandidateFinding {
            title: "AppState startup panic when init fails".to_string(),
            body: "Body".to_string(),
            priority: "high".to_string(),
            complexity: "simple".to_string(),
            affected_files: vec!["src-tauri/src/state.rs".to_string()],
            symptom_type: "startup panic".to_string(),
            keywords: vec!["appstate".to_string(), "filesystem".to_string()],
        };

        let existing = vec![
            ExistingIssue {
                number: 301,
                title: "A".to_string(),
                body: "Fingerprint: `appstate|filesystem|panic|state|startup`".to_string(),
                url: "https://github.com/owner/repo/issues/301".to_string(),
                labels: vec!["filed-by-maintainer".to_string()],
            },
            ExistingIssue {
                number: 277,
                title: "B".to_string(),
                body: "Fingerprint: `appstate|filesystem|panic|state|startup`".to_string(),
                url: "https://github.com/owner/repo/issues/277".to_string(),
                labels: vec!["filed-by-maintainer".to_string()],
            },
        ];

        let duplicate = find_duplicate_issue(&finding, &existing, 0.5).expect("match expected");
        assert_eq!(duplicate.issue.number, 277);
    }

    #[test]
    fn test_labels_to_remove_replaces_conflicting_priority_and_complexity() {
        let existing = vec![
            "filed-by-maintainer".to_string(),
            "priority:low".to_string(),
            "complexity:simple".to_string(),
            "triaged".to_string(),
        ];
        let desired = vec![
            "filed-by-maintainer".to_string(),
            "priority:high".to_string(),
            "complexity:high".to_string(),
        ];

        let remove = labels_to_remove(&existing, &desired);
        assert_eq!(
            remove,
            vec!["priority:low".to_string(), "complexity:simple".to_string()]
        );
    }
}
