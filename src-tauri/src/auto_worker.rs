use std::collections::HashMap;
use std::process::Command;
use std::sync::mpsc;
use std::time::{Duration, Instant};

use once_cell::sync::Lazy;
use tauri::{AppHandle, Emitter, Manager};
use uuid::Uuid;

use crate::models::GithubIssue;
use crate::state::AppState;

static IDLE_CHANNEL: Lazy<(std::sync::Mutex<mpsc::Sender<Uuid>>, std::sync::Mutex<mpsc::Receiver<Uuid>>)> = Lazy::new(|| {
    let (tx, rx) = mpsc::channel();
    (std::sync::Mutex::new(tx), std::sync::Mutex::new(rx))
});

pub fn notify_session_idle(session_id: Uuid) {
    if let Ok(tx) = IDLE_CHANNEL.0.lock() {
        let _ = tx.send(session_id);
    }
}

const POLL_INTERVAL_SECS: u64 = 30;
const SESSION_TIMEOUT_SECS: u64 = 30 * 60; // 30 minutes
const MAX_NUDGES: u32 = 3;
const NUDGE_COOLDOWN_SECS: u64 = 60;

struct ActiveSession {
    session_id: Uuid,
    project_id: Uuid,
    issue_number: u64,
    issue_title: String,
    repo_path: String,
    spawned_at: Instant,
    nudge_count: u32,
    last_idle_at: Option<Instant>,
    last_nudge_at: Option<Instant>,
}

pub struct AutoWorkerScheduler;

impl AutoWorkerScheduler {
    /// Remove stale `in-progress` labels from all enabled projects.
    /// Called on startup before any sessions exist — any `in-progress` label
    /// is orphaned from a previous run and must be cleaned up so the issue
    /// becomes eligible again.
    fn cleanup_stale_labels(app_handle: &AppHandle) {
        let state = match app_handle.try_state::<AppState>() {
            Some(s) => s,
            None => return,
        };

        let projects = {
            let storage = match state.storage.lock() {
                Ok(s) => s,
                Err(_) => return,
            };
            match storage.list_projects() {
                Ok(p) => p,
                Err(_) => return,
            }
        };

        for project in &projects {
            if !project.auto_worker.enabled || project.archived {
                continue;
            }
            let issues = match fetch_issues_sync(&project.repo_path) {
                Ok(issues) => issues,
                Err(_) => continue,
            };
            for issue in &issues {
                let labels: Vec<&str> = issue.labels.iter().map(|l| l.name.as_str()).collect();
                if labels.contains(&"in-progress") {
                    eprintln!("Auto-worker: removing stale in-progress label from #{}", issue.number);
                    let _ = remove_label_sync(&project.repo_path, issue.number, "in-progress");
                }
            }
        }
    }

    pub fn start(app_handle: AppHandle) {
        std::thread::spawn(move || {
            let mut active_sessions: HashMap<Uuid, ActiveSession> = HashMap::new();

            // On startup, clean up stale `in-progress` labels from any previous run.
            // No sessions are active yet, so any `in-progress` label is orphaned.
            Self::cleanup_stale_labels(&app_handle);

            loop {
                std::thread::sleep(Duration::from_secs(POLL_INTERVAL_SECS));

                // Drain idle notifications from status socket
                if let Ok(rx) = IDLE_CHANNEL.1.lock() {
                    while let Ok(session_id) = rx.try_recv() {
                        for session in active_sessions.values_mut() {
                            if session.session_id == session_id {
                                session.last_idle_at = Some(Instant::now());
                            }
                        }
                    }
                }

                let state = match app_handle.try_state::<AppState>() {
                    Some(s) => s,
                    None => continue,
                };

                // 1. Check timed-out sessions
                let timed_out: Vec<Uuid> = active_sessions
                    .iter()
                    .filter(|(_, s)| s.spawned_at.elapsed() > Duration::from_secs(SESSION_TIMEOUT_SECS))
                    .map(|(pid, _)| *pid)
                    .collect();

                for project_id in timed_out {
                    if let Some(session) = active_sessions.remove(&project_id) {
                        eprintln!("Auto-worker: session timed out for #{}", session.issue_number);
                        let (issue_number, issue_title) = session_issue_context(&session);
                        kill_session(&state, &session);
                        emit_status(&app_handle, project_id, "idle", Some("Session timed out"), issue_number, issue_title);
                    }
                }

                // 2. Nudge idle sessions
                let idle_to_nudge: Vec<Uuid> = active_sessions
                    .iter()
                    .filter(|(_, s)| {
                        s.last_idle_at.is_some()
                            && s.last_nudge_at.map_or(true, |t| t.elapsed() > Duration::from_secs(NUDGE_COOLDOWN_SECS))
                    })
                    .map(|(pid, _)| *pid)
                    .collect();

                for project_id in idle_to_nudge {
                    if let Some(session) = active_sessions.get_mut(&project_id) {
                        if session.nudge_count >= MAX_NUDGES {
                            let session = active_sessions.remove(&project_id).unwrap();
                            eprintln!("Auto-worker: killed after {} nudges for #{}", MAX_NUDGES, session.issue_number);
                            let (issue_number, issue_title) = session_issue_context(&session);
                            kill_session(&state, &session);
                            emit_status(&app_handle, project_id, "idle", Some("Killed after max nudges"), issue_number, issue_title);
                        } else {
                            nudge_session(&state, session);
                        }
                    }
                }

                // 3. Check for completed sessions (PTY no longer alive and removed from sessions map)
                let exited: Vec<Uuid> = active_sessions
                    .iter()
                    .filter(|(_, s)| {
                        if let Ok(pty_manager) = state.pty_manager.lock() {
                            !pty_manager.is_alive(s.session_id)
                        } else {
                            false
                        }
                    })
                    .map(|(pid, _)| *pid)
                    .collect();

                for project_id in exited {
                    if let Some(session) = active_sessions.remove(&project_id) {
                        eprintln!("Auto-worker: session completed for #{}", session.issue_number);
                        let (issue_number, issue_title) = session_issue_context(&session);
                        mark_issue_finished(&session);
                        cleanup_session(&state, &session);
                        emit_status(
                            &app_handle,
                            project_id,
                            "idle",
                            Some(&format!("Completed #{}", session.issue_number)),
                            issue_number,
                            issue_title,
                        );
                    }
                }

                // 4. For enabled projects without active sessions, try to pick an issue
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
                    if !project.auto_worker.enabled || project.archived {
                        continue;
                    }
                    if active_sessions.contains_key(&project.id) {
                        continue;
                    }

                    let issues = match fetch_issues_sync(&project.repo_path) {
                        Ok(issues) => issues,
                        Err(e) => {
                            eprintln!("Auto-worker: failed to fetch issues for {}: {}", project.name, e);
                            continue;
                        }
                    };

                    let eligible = match pick_eligible_issue(&issues) {
                        Some(issue) => issue.clone(),
                        None => continue,
                    };

                    match spawn_auto_worker_session(&state, &app_handle, project, &eligible) {
                        Ok(session_id) => {
                            let _ = add_label_sync(&project.repo_path, eligible.number, "in-progress");
                            emit_status(&app_handle, project.id, "working", None, Some(eligible.number), Some(&eligible.title));
                            active_sessions.insert(project.id, ActiveSession {
                                session_id,
                                project_id: project.id,
                                issue_number: eligible.number,
                                issue_title: eligible.title.clone(),
                                repo_path: project.repo_path.clone(),
                                spawned_at: Instant::now(),
                                nudge_count: 0,
                                last_idle_at: None,
                                last_nudge_at: None,
                            });
                        }
                        Err(e) => {
                            eprintln!("Auto-worker: failed to spawn session for #{}: {}", eligible.number, e);
                        }
                    }
                }
            }
        });
    }
}

fn emit_status(app_handle: &AppHandle, project_id: Uuid, status: &str, message: Option<&str>, issue_number: Option<u64>, issue_title: Option<&str>) {
    let payload = serde_json::json!({
        "status": status,
        "message": message.unwrap_or(""),
        "issue_number": issue_number,
        "issue_title": issue_title.unwrap_or(""),
    });
    let _ = app_handle.emit(&format!("auto-worker-status:{}", project_id), payload.to_string());
}

fn session_issue_context(session: &ActiveSession) -> (Option<u64>, Option<&str>) {
    (Some(session.issue_number), Some(session.issue_title.as_str()))
}

fn fetch_issues_sync(repo_path: &str) -> Result<Vec<GithubIssue>, String> {
    let output = Command::new("gh")
        .args(["issue", "list", "--json", "number,title,url,body,labels", "--limit", "50"])
        .current_dir(repo_path)
        .output()
        .map_err(|e| format!("Failed to run gh: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("gh issue list failed: {}", stderr));
    }

    serde_json::from_slice(&output.stdout)
        .map_err(|e| format!("Failed to parse gh output: {}", e))
}

fn add_label_sync(repo_path: &str, issue_number: u64, label: &str) -> Result<(), String> {
    let output = Command::new("gh")
        .args(["issue", "edit", &issue_number.to_string(), "--add-label", label])
        .current_dir(repo_path)
        .output()
        .map_err(|e| format!("Failed to run gh: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("gh issue edit failed: {}", stderr));
    }
    Ok(())
}

fn remove_label_sync(repo_path: &str, issue_number: u64, label: &str) -> Result<(), String> {
    let output = Command::new("gh")
        .args(["issue", "edit", &issue_number.to_string(), "--remove-label", label])
        .current_dir(repo_path)
        .output()
        .map_err(|e| format!("Failed to run gh: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("gh issue edit failed: {}", stderr));
    }
    Ok(())
}

fn spawn_auto_worker_session(
    state: &AppState,
    app_handle: &AppHandle,
    project: &crate::models::Project,
    issue: &GithubIssue,
) -> Result<Uuid, String> {
    let session_id = Uuid::new_v4();
    let label = crate::commands::next_session_label(&project.sessions);

    let base_dir = {
        let storage = state.storage.lock().map_err(|e| e.to_string())?;
        storage.base_dir()
    };
    let worktree_dir = base_dir.join("worktrees").join(&project.name).join(&label);

    let (session_dir, wt_path, wt_branch) =
        match crate::worktree::WorktreeManager::create_worktree(&project.repo_path, &label, &worktree_dir) {
            Ok(worktree_path) => {
                let wt_str = worktree_path
                    .to_str()
                    .ok_or_else(|| "worktree path is not valid UTF-8".to_string())?
                    .to_string();
                (wt_str.clone(), Some(wt_str), Some(label.clone()))
            }
            Err(e) if e == "unborn_branch" => {
                (project.repo_path.clone(), None, None)
            }
            Err(e) => return Err(e),
        };

    let initial_prompt = crate::session_args::build_issue_prompt(
        issue.number, &issue.title, &issue.url, true,
    );

    {
        let storage = state.storage.lock().map_err(|e| e.to_string())?;
        let mut proj = storage.load_project(project.id).map_err(|e| e.to_string())?;
        proj.sessions.push(crate::models::SessionConfig {
            id: session_id,
            label: label.clone(),
            worktree_path: wt_path,
            worktree_branch: wt_branch,
            archived: false,
            kind: "codex".to_string(),
            github_issue: Some(issue.clone()),
            initial_prompt: Some(initial_prompt.clone()),
            done_commits: vec![],
            auto_worker_session: true,
        });
        storage.save_project(&proj).map_err(|e| e.to_string())?;
    }

    let mut pty_manager = state.pty_manager.lock().map_err(|e| e.to_string())?;
    pty_manager.spawn_session(
        session_id,
        &session_dir,
        "codex",
        app_handle.clone(),
        false,
        Some(&initial_prompt),
        24,
        80,
    )?;

    Ok(session_id)
}

fn nudge_session(state: &AppState, session: &mut ActiveSession) {
    let nudge_msg = "\nContinue working autonomously. Do not ask questions or wait for input. Complete the task.\n";
    if let Ok(mut pty_manager) = state.pty_manager.lock() {
        let _ = pty_manager.write_to_session(session.session_id, nudge_msg.as_bytes());
    }
    session.nudge_count += 1;
    session.last_nudge_at = Some(Instant::now());
    eprintln!("Auto-worker: nudged session for #{} (nudge {})", session.issue_number, session.nudge_count);
}

fn kill_session(state: &AppState, session: &ActiveSession) {
    if let Ok(mut pty_manager) = state.pty_manager.lock() {
        let _ = pty_manager.close_session(session.session_id);
    }
    let _ = remove_label_sync(&session.repo_path, session.issue_number, "in-progress");
    cleanup_session(state, session);
}

fn json_has_results(json: &str) -> bool {
    serde_json::from_str::<Vec<serde_json::Value>>(json)
        .map(|v| !v.is_empty())
        .unwrap_or(false)
}

fn has_merged_pr_sync(repo_path: &str, issue_number: u64) -> bool {
    let search_query = format!("#{}", issue_number);
    let output = Command::new("gh")
        .args([
            "pr", "list",
            "--search", &search_query,
            "--state", "merged",
            "--json", "number",
            "--limit", "1",
        ])
        .current_dir(repo_path)
        .output();

    match output {
        Ok(o) if o.status.success() => {
            json_has_results(&String::from_utf8_lossy(&o.stdout))
        }
        Ok(o) => {
            eprintln!("Auto-worker: gh pr list failed for #{}: {}", issue_number, String::from_utf8_lossy(&o.stderr));
            false
        }
        Err(e) => {
            eprintln!("Auto-worker: failed to run gh pr list for #{}: {}", issue_number, e);
            false
        }
    }
}

fn close_issue_sync(repo_path: &str, issue_number: u64) -> Result<(), String> {
    let output = Command::new("gh")
        .args(["issue", "close", &issue_number.to_string()])
        .current_dir(repo_path)
        .output()
        .map_err(|e| format!("Failed to run gh: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("gh issue close failed: {}", stderr));
    }
    Ok(())
}

fn mark_issue_finished(session: &ActiveSession) {
    let _ = remove_label_sync(&session.repo_path, session.issue_number, "in-progress");
    if has_merged_pr_sync(&session.repo_path, session.issue_number) {
        let _ = add_label_sync(&session.repo_path, session.issue_number, "finished-by-worker");
        let _ = close_issue_sync(&session.repo_path, session.issue_number);
        eprintln!("Auto-worker: closed #{} (merged PR verified)", session.issue_number);
    } else {
        eprintln!("Auto-worker: #{} exited without merged PR, not closing", session.issue_number);
    }
}

fn cleanup_session(state: &AppState, session: &ActiveSession) {
    if let Ok(storage) = state.storage.lock() {
        if let Ok(mut project) = storage.load_project(session.project_id) {
            let sess = project.sessions.iter().find(|s| s.id == session.session_id).cloned();
            project.sessions.retain(|s| s.id != session.session_id);
            let _ = storage.save_project(&project);

            if let Some(sess) = sess {
                if let (Some(wt_path), Some(branch)) = (sess.worktree_path, sess.worktree_branch) {
                    let _ = crate::worktree::WorktreeManager::remove_worktree(
                        &wt_path, &project.repo_path, &branch,
                    );
                }
            }
        }
    }
}

/// Check if an issue is eligible for auto-worker processing.
pub fn is_eligible(issue: &GithubIssue) -> bool {
    let labels: Vec<&str> = issue.labels.iter().map(|l| l.name.as_str()).collect();
    labels.contains(&"priority: high")
        && labels.contains(&"complexity: low")
        && labels.contains(&"triaged")
        && !labels.contains(&"in-progress")
        && !labels.contains(&"finished-by-worker")
}

/// Pick the first eligible issue from a list.
pub fn pick_eligible_issue(issues: &[GithubIssue]) -> Option<&GithubIssue> {
    issues.iter().find(|i| is_eligible(i))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::GithubLabel;

    fn make_issue(labels: &[&str]) -> GithubIssue {
        GithubIssue {
            number: 1,
            title: "Test".to_string(),
            url: "https://github.com/o/r/issues/1".to_string(),
            body: None,
            labels: labels.iter().map(|l| GithubLabel { name: l.to_string() }).collect(),
        }
    }

    #[test]
    fn eligible_issue_has_all_required_labels() {
        let issue = make_issue(&["priority: high", "complexity: low", "triaged"]);
        assert!(is_eligible(&issue));
    }

    #[test]
    fn missing_priority_high_not_eligible() {
        let issue = make_issue(&["complexity: low", "triaged"]);
        assert!(!is_eligible(&issue));
    }

    #[test]
    fn missing_complexity_low_not_eligible() {
        let issue = make_issue(&["priority: high", "triaged"]);
        assert!(!is_eligible(&issue));
    }

    #[test]
    fn missing_triaged_not_eligible() {
        let issue = make_issue(&["priority: high", "complexity: low"]);
        assert!(!is_eligible(&issue));
    }

    #[test]
    fn in_progress_not_eligible() {
        let issue = make_issue(&["priority: high", "complexity: low", "triaged", "in-progress"]);
        assert!(!is_eligible(&issue));
    }

    #[test]
    fn finished_by_worker_not_eligible() {
        let issue = make_issue(&["priority: high", "complexity: low", "triaged", "finished-by-worker"]);
        assert!(!is_eligible(&issue));
    }

    #[test]
    fn pick_eligible_returns_first_match() {
        let issues = vec![
            make_issue(&["priority: low"]),
            make_issue(&["priority: high", "complexity: low", "triaged"]),
        ];
        let picked = pick_eligible_issue(&issues);
        assert!(picked.is_some());
    }

    #[test]
    fn pick_eligible_returns_none_when_no_match() {
        let issues = vec![
            make_issue(&["priority: low"]),
            make_issue(&["in-progress", "priority: high", "complexity: low", "triaged"]),
        ];
        assert!(pick_eligible_issue(&issues).is_none());
    }

    #[test]
    fn session_issue_context_includes_issue_number_and_title() {
        let session = ActiveSession {
            session_id: Uuid::new_v4(),
            project_id: Uuid::new_v4(),
            issue_number: 289,
            issue_title: "Dead issue_title field".to_string(),
            repo_path: "/tmp/repo".to_string(),
            spawned_at: Instant::now(),
            nudge_count: 0,
            last_idle_at: None,
            last_nudge_at: None,
        };

        let (issue_number, issue_title) = session_issue_context(&session);

        assert_eq!(issue_number, Some(289));
        assert_eq!(issue_title, Some("Dead issue_title field"));
    }

    #[test]
    fn json_has_results_with_result() {
        let json = r#"[{"number":42}]"#;
        assert!(json_has_results(json));
    }

    #[test]
    fn json_has_results_empty() {
        let json = "[]";
        assert!(!json_has_results(json));
    }

    #[test]
    fn json_has_results_invalid_json() {
        assert!(!json_has_results("not json"));
    }
}
