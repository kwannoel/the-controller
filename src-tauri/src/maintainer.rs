use std::collections::HashMap;
use std::time::{Duration, Instant};

use chrono::Utc;
use serde::Deserialize;
use tauri::{AppHandle, Emitter, Manager};
use uuid::Uuid;

use crate::models::{
    FindingAction, FindingSeverity, MaintainerFinding, MaintainerReport, ReportStatus,
};
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

                    let result = run_health_check(&project.repo_path, project.id);

                    match result {
                        Ok(report) => {
                            let status_str = match report.status {
                                ReportStatus::Passing => "passing",
                                ReportStatus::Warnings => "warnings",
                                ReportStatus::Failing => "failing",
                            };

                            if let Ok(storage) = state.storage.lock() {
                                let _ = storage.save_maintainer_report(&report);
                            }

                            let _ = app_handle.emit(
                                &format!("maintainer-status:{}", project.id),
                                status_str,
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

pub fn build_health_check_prompt(repo_path: &str) -> String {
    format!(
        r#"Analyze the project at {repo_path} for code quality, test robustness, architecture, and documentation.

Respond with JSON only, using this exact structure:

{{
  "findings": [
    {{
      "severity": "Info" | "Warning" | "Error",
      "category": "<category like tests, build, dependencies, docs, architecture>",
      "description": "<description of the finding>",
      "action": "reported" | "fixed" | {{"pr_created": "<url>"}}
    }}
  ],
  "summary": "<brief summary of findings>"
}}

If there are no issues, return an empty findings array with a positive summary."#
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

    // Best-effort heuristic: find first '{' and last '}'. If the output contains
    // extra '}' characters in trailing prose, this may extract an invalid slice.
    // The ```json block path above is more reliable.
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
struct RawReport {
    findings: Vec<RawFinding>,
    summary: String,
}

#[derive(Deserialize)]
struct RawFinding {
    severity: String,
    category: String,
    description: String,
    action: serde_json::Value,
}

fn map_severity(s: &str) -> Result<FindingSeverity, String> {
    match s.to_lowercase().as_str() {
        "info" => Ok(FindingSeverity::Info),
        "warning" => Ok(FindingSeverity::Warning),
        "error" => Ok(FindingSeverity::Error),
        other => Err(format!("Unknown severity: {}", other)),
    }
}

fn map_action(value: &serde_json::Value) -> Result<FindingAction, String> {
    match value {
        serde_json::Value::String(s) => match s.as_str() {
            "reported" => Ok(FindingAction::Reported),
            "fixed" => Ok(FindingAction::Fixed),
            other => Err(format!("Unknown action string: {}", other)),
        },
        serde_json::Value::Object(obj) => {
            if let Some(url) = obj.get("pr_created").and_then(|v| v.as_str()) {
                Ok(FindingAction::PrCreated {
                    url: url.to_string(),
                })
            } else {
                Err(format!("Unknown action object: {:?}", obj))
            }
        }
        _ => Err(format!("Unexpected action value: {:?}", value)),
    }
}

pub fn parse_report_output(output: &str, project_id: Uuid) -> Result<MaintainerReport, String> {
    let json_str = extract_json(output).ok_or("No JSON found in output")?;
    let raw: RawReport =
        serde_json::from_str(json_str).map_err(|e| format!("Failed to parse JSON: {}", e))?;

    let mut findings = Vec::new();
    for raw_finding in &raw.findings {
        let severity = map_severity(&raw_finding.severity)?;
        let action = map_action(&raw_finding.action)?;
        findings.push(MaintainerFinding {
            severity,
            category: raw_finding.category.clone(),
            description: raw_finding.description.clone(),
            action_taken: action,
        });
    }

    let status = if findings
        .iter()
        .any(|f| f.severity == FindingSeverity::Error)
    {
        ReportStatus::Failing
    } else if findings
        .iter()
        .any(|f| f.severity == FindingSeverity::Warning)
    {
        ReportStatus::Warnings
    } else {
        ReportStatus::Passing
    };

    Ok(MaintainerReport {
        id: Uuid::new_v4(),
        project_id,
        timestamp: Utc::now().to_rfc3339(),
        status,
        findings,
        summary: raw.summary,
    })
}

pub fn run_health_check(repo_path: &str, project_id: Uuid) -> Result<MaintainerReport, String> {
    let prompt = build_health_check_prompt(repo_path);
    let output = std::process::Command::new("claude")
        .arg("--print")
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
    parse_report_output(&stdout, project_id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_health_check_prompt() {
        let prompt = build_health_check_prompt("/tmp/my-project");
        assert!(prompt.contains("/tmp/my-project"));
        assert!(prompt.contains("JSON"));
        assert!(prompt.contains("findings"));
    }

    #[test]
    fn test_parse_report_output_valid_json() {
        let output = r#"{"findings":[{"severity":"Warning","category":"tests","description":"flaky test","action":"reported"}],"summary":"1 warning"}"#;
        let project_id = Uuid::new_v4();
        let result = parse_report_output(output, project_id);
        assert!(result.is_ok());
        let report = result.unwrap();
        assert_eq!(report.findings.len(), 1);
        assert_eq!(report.summary, "1 warning");
        assert_eq!(report.status, ReportStatus::Warnings);
    }

    #[test]
    fn test_parse_report_output_no_findings() {
        let output = r#"{"findings":[],"summary":"All clear"}"#;
        let project_id = Uuid::new_v4();
        let result = parse_report_output(output, project_id);
        assert!(result.is_ok());
        let report = result.unwrap();
        assert_eq!(report.status, ReportStatus::Passing);
    }

    #[test]
    fn test_parse_report_output_with_errors() {
        let output = r#"{"findings":[{"severity":"Error","category":"build","description":"build fails","action":"reported"}],"summary":"build broken"}"#;
        let project_id = Uuid::new_v4();
        let result = parse_report_output(output, project_id);
        assert!(result.is_ok());
        assert_eq!(result.unwrap().status, ReportStatus::Failing);
    }

    #[test]
    fn test_parse_report_output_extracts_json_from_surrounding_text() {
        let output =
            "Here is the analysis:\n```json\n{\"findings\":[],\"summary\":\"ok\"}\n```\nDone.";
        let project_id = Uuid::new_v4();
        let result = parse_report_output(output, project_id);
        assert!(result.is_ok());
        assert_eq!(result.unwrap().summary, "ok");
    }

    #[test]
    fn test_parse_report_output_pr_created_action() {
        let output = r#"{"findings":[{"severity":"Info","category":"ci","description":"opened fix","action":{"pr_created":"https://github.com/owner/repo/pull/1"}}],"summary":"pr opened"}"#;
        let project_id = Uuid::new_v4();
        let result = parse_report_output(output, project_id);
        assert!(result.is_ok());
        let report = result.unwrap();
        assert!(matches!(
            &report.findings[0].action_taken,
            FindingAction::PrCreated { url } if url == "https://github.com/owner/repo/pull/1"
        ));
    }

    #[test]
    fn test_parse_report_output_invalid() {
        let output = "I couldn't analyze the project";
        let project_id = Uuid::new_v4();
        let result = parse_report_output(output, project_id);
        assert!(result.is_err());
    }
}
