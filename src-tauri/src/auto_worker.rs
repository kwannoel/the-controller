use std::collections::{HashMap, HashSet};
use std::process::Command;
use std::sync::mpsc;
use std::time::{Duration, Instant};

use once_cell::sync::Lazy;
use tauri::{AppHandle, Manager};
use uuid::Uuid;

use crate::labels;
use crate::models::GithubIssue;
use crate::state::AppState;

#[allow(clippy::type_complexity)]
static IDLE_CHANNEL: Lazy<(
    std::sync::Mutex<mpsc::Sender<Uuid>>,
    std::sync::Mutex<mpsc::Receiver<Uuid>>,
)> = Lazy::new(|| {
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
const LABEL_PRIORITY_HIGH: &str = labels::PRIORITY_HIGH;
const LABEL_COMPLEXITY_LOW: &str = labels::COMPLEXITY_LOW;
const LABEL_IN_PROGRESS: &str = labels::IN_PROGRESS;
const LABEL_ASSIGNED_TO_AUTO_WORKER: &str = labels::ASSIGNED_TO_AUTO_WORKER;
const LABEL_FINISHED_BY_WORKER: &str = labels::FINISHED_BY_WORKER;

#[derive(Debug, Default, PartialEq, Eq)]
struct WorkerLabelPlan {
    add: Vec<&'static str>,
    remove: Vec<&'static str>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct LabelDefinition {
    description: &'static str,
    color: &'static str,
}

fn worker_claim_label_plan() -> WorkerLabelPlan {
    WorkerLabelPlan {
        add: vec![LABEL_IN_PROGRESS, LABEL_ASSIGNED_TO_AUTO_WORKER],
        remove: vec![],
    }
}

fn worker_cleanup_label_plan(issue_closed: Option<bool>) -> WorkerLabelPlan {
    let mut remove = vec![LABEL_IN_PROGRESS];
    if issue_closed == Some(false) {
        remove.push(LABEL_ASSIGNED_TO_AUTO_WORKER);
    }
    WorkerLabelPlan {
        add: vec![],
        remove,
    }
}

fn label_definition(label: &str) -> Option<LabelDefinition> {
    match label {
        LABEL_IN_PROGRESS => Some(LabelDefinition {
            description: "Issue is being worked on in a session",
            color: "F9E2AF",
        }),
        LABEL_ASSIGNED_TO_AUTO_WORKER => Some(LabelDefinition {
            description: "Issue has been handled by the auto-worker",
            color: "94E2D5",
        }),
        LABEL_FINISHED_BY_WORKER => Some(LabelDefinition {
            description: "Issue was completed by the auto-worker",
            color: "A6E3A1",
        }),
        _ => None,
    }
}

fn apply_worker_label_plan_sync(
    state: &AppState,
    repo_path: &str,
    issue_number: u64,
    plan: &WorkerLabelPlan,
) {
    for label in &plan.add {
        let _ = add_label_sync(state, repo_path, issue_number, label);
    }
    for label in &plan.remove {
        let _ = remove_label_sync(state, repo_path, issue_number, label);
    }
}

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

#[derive(Debug, Clone, PartialEq, Eq)]
struct StartupWorkerCandidate {
    session_id: Uuid,
    project_id: Uuid,
    issue_number: u64,
    issue_title: String,
    repo_path: String,
    session_dir: String,
    kind: String,
    ordinal: usize,
    live_session: bool,
}

#[derive(Debug, Default, PartialEq, Eq)]
struct StartupReconciliation {
    restore: Vec<StartupWorkerCandidate>,
    cleanup: Vec<StartupWorkerCandidate>,
}

pub struct AutoWorkerScheduler;

impl AutoWorkerScheduler {
    /// Remove stale `in-progress` labels from all enabled projects.
    /// Called on startup before any sessions exist — any `in-progress` label
    /// is orphaned from a previous run and must be cleaned up so the issue
    /// becomes eligible again.
    fn cleanup_stale_labels(
        app_handle: &AppHandle,
        protected_issues: &HashMap<String, HashSet<u64>>,
    ) {
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
                Ok(inventory) => {
                    inventory.warn_if_corrupt("auto-worker stale label cleanup");
                    inventory.projects
                }
                Err(_) => return,
            }
        };

        for project in &projects {
            if !project.auto_worker.enabled {
                continue;
            }
            let issues = match fetch_issues_sync(&project.repo_path) {
                Ok(issues) => issues,
                Err(_) => continue,
            };
            for issue in &issues {
                let labels: Vec<&str> = issue.labels.iter().map(|l| l.name.as_str()).collect();
                let protected = protected_issues
                    .get(&project.repo_path)
                    .map(|issues| issues.contains(&issue.number))
                    .unwrap_or(false);
                if labels.contains(&LABEL_IN_PROGRESS) && !protected {
                    tracing::info!("removing stale in-progress label from #{}", issue.number);
                    let _ = remove_label_sync(
                        state.inner(),
                        &project.repo_path,
                        issue.number,
                        LABEL_IN_PROGRESS,
                    );
                }
            }
        }
    }

    pub fn start(app_handle: AppHandle) {
        std::thread::spawn(move || {
            let mut active_sessions = Self::restore_startup_state(&app_handle);

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
                    .filter(|(_, s)| {
                        s.spawned_at.elapsed() > Duration::from_secs(SESSION_TIMEOUT_SECS)
                    })
                    .map(|(pid, _)| *pid)
                    .collect();

                for project_id in timed_out {
                    if let Some(session) = active_sessions.remove(&project_id) {
                        tracing::info!("session timed out for #{}", session.issue_number);
                        let (issue_number, issue_title) = session_issue_context(&session);
                        kill_session(&state, &session);
                        emit_status(
                            &state,
                            project_id,
                            "idle",
                            Some("Session timed out"),
                            issue_number,
                            issue_title,
                        );
                    }
                }

                // 2. Nudge idle sessions
                let idle_to_nudge: Vec<Uuid> = active_sessions
                    .iter()
                    .filter(|(_, s)| {
                        s.last_idle_at.is_some()
                            && s.last_nudge_at.is_none_or(|t| {
                                t.elapsed() > Duration::from_secs(NUDGE_COOLDOWN_SECS)
                            })
                    })
                    .map(|(pid, _)| *pid)
                    .collect();

                for project_id in idle_to_nudge {
                    if let Some(session) = active_sessions.get_mut(&project_id) {
                        if session.nudge_count >= MAX_NUDGES {
                            if let Some(session) = active_sessions.remove(&project_id) {
                                tracing::info!(
                                    "killed after {} nudges for #{}",
                                    MAX_NUDGES,
                                    session.issue_number
                                );
                                let (issue_number, issue_title) = session_issue_context(&session);
                                kill_session(&state, &session);
                                emit_status(
                                    &state,
                                    project_id,
                                    "idle",
                                    Some("Killed after max nudges"),
                                    issue_number,
                                    issue_title,
                                );
                            }
                        } else {
                            nudge_session(&state, session);
                        }
                    }
                }

                // 3. Check for completed sessions (PTY no longer alive and removed from sessions map)
                let exited: Vec<Uuid> = if let Ok(pty_manager) = state.pty_manager.lock() {
                    active_sessions
                        .iter()
                        .filter(|(_, s)| !pty_manager.is_alive(s.session_id))
                        .map(|(pid, _)| *pid)
                        .collect()
                } else {
                    Vec::new()
                };

                for project_id in exited {
                    if let Some(session) = active_sessions.remove(&project_id) {
                        tracing::info!("session completed for #{}", session.issue_number);
                        let (issue_number, issue_title) = session_issue_context(&session);
                        mark_issue_finished(state.inner(), &session);
                        cleanup_session(&state, &session);
                        emit_status(
                            &state,
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
                        Ok(inventory) => {
                            inventory.warn_if_corrupt("auto-worker scheduler");
                            inventory.projects
                        }
                        Err(_) => continue,
                    }
                };

                for project in &projects {
                    if !project.auto_worker.enabled {
                        continue;
                    }
                    if active_sessions.contains_key(&project.id) {
                        continue;
                    }

                    let issues = match fetch_issues_sync(&project.repo_path) {
                        Ok(issues) => issues,
                        Err(e) => {
                            tracing::error!("failed to fetch issues for {}: {}", project.name, e);
                            continue;
                        }
                    };

                    let eligible = match pick_eligible_issue(&issues) {
                        Some(issue) => issue.clone(),
                        None => continue,
                    };

                    match spawn_auto_worker_session(&state, project, &eligible) {
                        Ok(session_id) => {
                            apply_worker_label_plan_sync(
                                state.inner(),
                                &project.repo_path,
                                eligible.number,
                                &worker_claim_label_plan(),
                            );
                            emit_status(
                                &state,
                                project.id,
                                "working",
                                None,
                                Some(eligible.number),
                                Some(&eligible.title),
                            );
                            active_sessions.insert(
                                project.id,
                                ActiveSession {
                                    session_id,
                                    project_id: project.id,
                                    issue_number: eligible.number,
                                    issue_title: eligible.title.clone(),
                                    repo_path: project.repo_path.clone(),
                                    spawned_at: Instant::now(),
                                    nudge_count: 0,
                                    last_idle_at: None,
                                    last_nudge_at: None,
                                },
                            );
                        }
                        Err(e) => {
                            tracing::error!(
                                "failed to spawn session for #{}: {}",
                                eligible.number,
                                e
                            );
                        }
                    }
                }
            }
        });
    }

    fn restore_startup_state(app_handle: &AppHandle) -> HashMap<Uuid, ActiveSession> {
        let state = match app_handle.try_state::<AppState>() {
            Some(s) => s,
            None => return HashMap::new(),
        };

        let projects = {
            let storage = match state.storage.lock() {
                Ok(s) => s,
                Err(_) => return HashMap::new(),
            };
            match storage.list_projects() {
                Ok(p) => p,
                Err(_) => return HashMap::new(),
            }
        };

        let mut candidates = Vec::new();

        for project in &projects {
            if !project.auto_worker.enabled {
                continue;
            }

            for (ordinal, session) in project.sessions.iter().enumerate() {
                if !session.auto_worker_session {
                    continue;
                }
                let issue = match &session.github_issue {
                    Some(issue) => issue,
                    None => continue,
                };
                let session_dir = session
                    .worktree_path
                    .clone()
                    .unwrap_or_else(|| project.repo_path.clone());

                candidates.push(StartupWorkerCandidate {
                    session_id: session.id,
                    project_id: project.id,
                    issue_number: issue.number,
                    issue_title: issue.title.clone(),
                    repo_path: project.repo_path.clone(),
                    session_dir,
                    kind: session.kind.clone(),
                    ordinal,
                    live_session: crate::broker_client::BrokerClient::new().has_session(session.id),
                });
            }
        }

        let mut active_sessions = HashMap::new();
        let mut attached_session_ids = HashSet::new();

        let reconciliation = reconcile_startup_workers(candidates);

        for candidate in &reconciliation.restore {
            let restored = startup_candidate_to_active_session(candidate);
            let attach_result = {
                let mut pty_manager = match state.pty_manager.lock() {
                    Ok(manager) => manager,
                    Err(_) => continue,
                };
                pty_manager.spawn_session(
                    candidate.session_id,
                    &candidate.session_dir,
                    &candidate.kind,
                    state.emitter.clone(),
                    true,
                    None,
                    24,
                    80,
                )
            };

            if let Err(error) = attach_result {
                tracing::error!(
                    "failed to restore session {} for #{}: {}",
                    candidate.session_id,
                    candidate.issue_number,
                    error
                );
                continue;
            }

            attached_session_ids.insert(candidate.session_id);
            active_sessions.insert(candidate.project_id, restored);
        }

        let finalized = finalize_startup_restoration(reconciliation, &attached_session_ids);
        let protected_issues = protected_issue_numbers_by_repo(&finalized.restore);

        Self::cleanup_stale_labels(app_handle, &protected_issues);

        for candidate in &finalized.cleanup {
            cleanup_startup_worker(&state, candidate, &protected_issues);
        }

        active_sessions
    }
}

fn emit_status(
    state: &AppState,
    project_id: Uuid,
    status: &str,
    message: Option<&str>,
    issue_number: Option<u64>,
    issue_title: Option<&str>,
) {
    let payload = serde_json::json!({
        "status": status,
        "message": message.unwrap_or(""),
        "issue_number": issue_number,
        "issue_title": issue_title.unwrap_or(""),
    });
    let _ = state.emitter.emit(
        &format!("auto-worker-status:{}", project_id),
        &payload.to_string(),
    );
}

fn session_issue_context(session: &ActiveSession) -> (Option<u64>, Option<&str>) {
    (
        Some(session.issue_number),
        Some(session.issue_title.as_str()),
    )
}

fn startup_candidate_to_active_session(candidate: &StartupWorkerCandidate) -> ActiveSession {
    ActiveSession {
        session_id: candidate.session_id,
        project_id: candidate.project_id,
        issue_number: candidate.issue_number,
        issue_title: candidate.issue_title.clone(),
        repo_path: candidate.repo_path.clone(),
        spawned_at: Instant::now(),
        nudge_count: 0,
        last_idle_at: None,
        last_nudge_at: None,
    }
}

fn reconcile_startup_workers(candidates: Vec<StartupWorkerCandidate>) -> StartupReconciliation {
    // Per project, pick the best candidate to restore:
    // - Prefer a live broker session (reattach)
    // - Otherwise pick the highest-ordinal candidate (resume with --continue)
    // All other candidates go to cleanup.
    let mut best_by_project: HashMap<Uuid, usize> = HashMap::new();

    for (idx, candidate) in candidates.iter().enumerate() {
        match best_by_project.get(&candidate.project_id) {
            Some(&prev_idx) => {
                let prev = &candidates[prev_idx];
                // Prefer live over non-live; among same liveness, prefer higher ordinal
                if candidate.live_session && !prev.live_session
                    || (candidate.live_session == prev.live_session
                        && candidate.ordinal > prev.ordinal)
                {
                    best_by_project.insert(candidate.project_id, idx);
                }
            }
            None => {
                best_by_project.insert(candidate.project_id, idx);
            }
        }
    }

    let mut reconciliation = StartupReconciliation::default();

    for (idx, candidate) in candidates.into_iter().enumerate() {
        if best_by_project.get(&candidate.project_id) == Some(&idx) {
            reconciliation.restore.push(candidate);
        } else {
            reconciliation.cleanup.push(candidate);
        }
    }

    reconciliation
}

fn finalize_startup_restoration(
    reconciliation: StartupReconciliation,
    attached_session_ids: &HashSet<Uuid>,
) -> StartupReconciliation {
    let mut finalized = StartupReconciliation {
        restore: Vec::new(),
        cleanup: reconciliation.cleanup,
    };

    for candidate in reconciliation.restore {
        if attached_session_ids.contains(&candidate.session_id) {
            finalized.restore.push(candidate);
        } else {
            finalized.cleanup.push(candidate);
        }
    }

    finalized
}

fn protected_issue_numbers_by_repo(
    candidates: &[StartupWorkerCandidate],
) -> HashMap<String, HashSet<u64>> {
    let mut protected = HashMap::new();

    for candidate in candidates {
        protected
            .entry(candidate.repo_path.clone())
            .or_insert_with(HashSet::new)
            .insert(candidate.issue_number);
    }

    protected
}

fn fetch_issues_sync(repo_path: &str) -> Result<Vec<GithubIssue>, String> {
    let output = Command::new("gh")
        .args([
            "issue",
            "list",
            "--json",
            "number,title,url,body,labels",
            "--limit",
            "50",
        ])
        .current_dir(repo_path)
        .output()
        .map_err(|e| format!("Failed to run gh: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("gh issue list failed: {}", stderr));
    }

    serde_json::from_slice(&output.stdout).map_err(|e| format!("Failed to parse gh output: {}", e))
}

fn finish_label_edit<F>(state: &AppState, repo_path: &str, edit: F) -> Result<(), String>
where
    F: FnOnce() -> Result<(), String>,
{
    edit()?;

    if let Ok(mut cache) = state.issue_cache.lock() {
        cache.invalidate(repo_path);
    }

    Ok(())
}

fn edit_label_sync(
    repo_path: &str,
    issue_number: u64,
    mode: &str,
    label: &str,
) -> Result<(), String> {
    if mode == "--add-label" {
        if let Some(definition) = label_definition(label) {
            let _ = Command::new("gh")
                .args([
                    "label",
                    "create",
                    label,
                    "--description",
                    definition.description,
                    "--color",
                    definition.color,
                    "--force",
                ])
                .current_dir(repo_path)
                .output();
        }
    }

    let output = Command::new("gh")
        .args(["issue", "edit", &issue_number.to_string(), mode, label])
        .current_dir(repo_path)
        .output()
        .map_err(|e| format!("Failed to run gh: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("gh issue edit failed: {}", stderr));
    }
    Ok(())
}

fn add_label_sync(
    state: &AppState,
    repo_path: &str,
    issue_number: u64,
    label: &str,
) -> Result<(), String> {
    finish_label_edit(state, repo_path, || {
        edit_label_sync(repo_path, issue_number, "--add-label", label)
    })
}

fn remove_label_sync(
    state: &AppState,
    repo_path: &str,
    issue_number: u64,
    label: &str,
) -> Result<(), String> {
    finish_label_edit(state, repo_path, || {
        edit_label_sync(repo_path, issue_number, "--remove-label", label)
    })
}

fn issue_is_closed_sync(repo_path: &str, issue_number: u64) -> Result<bool, String> {
    let output = Command::new("gh")
        .args([
            "issue",
            "view",
            &issue_number.to_string(),
            "--json",
            "state",
        ])
        .current_dir(repo_path)
        .output()
        .map_err(|e| format!("Failed to run gh: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("gh issue view failed: {}", stderr));
    }

    let raw: serde_json::Value = serde_json::from_slice(&output.stdout)
        .map_err(|e| format!("Failed to parse gh output: {}", e))?;

    Ok(raw["state"].as_str() == Some("CLOSED"))
}

fn spawn_auto_worker_session(
    state: &AppState,
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

    let (session_dir, wt_path, wt_branch) = match crate::worktree::WorktreeManager::create_worktree(
        &project.repo_path,
        &label,
        &worktree_dir,
    ) {
        Ok(worktree_path) => {
            let wt_str = worktree_path
                .to_str()
                .ok_or_else(|| "worktree path is not valid UTF-8".to_string())?
                .to_string();
            (wt_str.clone(), Some(wt_str), Some(label.clone()))
        }
        Err(e) if e == "unborn_branch" => (project.repo_path.clone(), None, None),
        Err(e) => return Err(e),
    };

    let initial_prompt =
        crate::session_args::build_issue_prompt(issue.number, &issue.title, &issue.url, true);

    {
        let storage = state.storage.lock().map_err(|e| e.to_string())?;
        let mut proj = storage
            .load_project(project.id)
            .map_err(|e| e.to_string())?;
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
        state.emitter.clone(),
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
    tracing::info!(
        "nudged session for #{} (nudge {})",
        session.issue_number,
        session.nudge_count
    );
}

fn kill_session(state: &AppState, session: &ActiveSession) {
    if let Ok(mut pty_manager) = state.pty_manager.lock() {
        let _ = pty_manager.close_session(session.session_id);
    }
    mark_issue_finished(state, session);
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
            "pr",
            "list",
            "--search",
            &search_query,
            "--state",
            "merged",
            "--json",
            "number",
            "--limit",
            "1",
        ])
        .current_dir(repo_path)
        .output();

    match output {
        Ok(o) if o.status.success() => json_has_results(&String::from_utf8_lossy(&o.stdout)),
        Ok(o) => {
            tracing::error!(
                "gh pr list failed for #{}: {}",
                issue_number,
                String::from_utf8_lossy(&o.stderr)
            );
            false
        }
        Err(e) => {
            tracing::error!("failed to run gh pr list for #{}: {}", issue_number, e);
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

fn mark_issue_finished(state: &AppState, session: &ActiveSession) {
    let mut issue_closed = issue_is_closed_sync(&session.repo_path, session.issue_number).ok();

    if issue_closed != Some(true)
        && has_merged_pr_sync(&session.repo_path, session.issue_number)
        && close_issue_sync(&session.repo_path, session.issue_number).is_ok()
    {
        issue_closed = Some(true);
    }

    apply_worker_label_plan_sync(
        state,
        &session.repo_path,
        session.issue_number,
        &worker_cleanup_label_plan(issue_closed),
    );

    if issue_closed == Some(true) {
        let _ = add_label_sync(
            state,
            &session.repo_path,
            session.issue_number,
            LABEL_FINISHED_BY_WORKER,
        );
        tracing::info!("finalized #{} as completed", session.issue_number);
    } else if issue_closed == Some(false) {
        let _ = remove_label_sync(
            state,
            &session.repo_path,
            session.issue_number,
            LABEL_FINISHED_BY_WORKER,
        );
        tracing::info!("#{} exited while still open", session.issue_number);
    } else {
        tracing::warn!(
            "#{} cleanup could not confirm issue state",
            session.issue_number
        );
    }
}
fn cleanup_startup_worker(
    state: &AppState,
    candidate: &StartupWorkerCandidate,
    protected_issues: &HashMap<String, HashSet<u64>>,
) {
    if let Ok(mut pty_manager) = state.pty_manager.lock() {
        let _ = pty_manager.close_session(candidate.session_id);
    }

    let preserve_label = protected_issues
        .get(&candidate.repo_path)
        .map(|issues| issues.contains(&candidate.issue_number))
        .unwrap_or(false);

    let session = startup_candidate_to_active_session(candidate);
    if !preserve_label {
        mark_issue_finished(state, &session);
    }
    cleanup_session(state, &session);
}

fn cleanup_session(state: &AppState, session: &ActiveSession) {
    if let Ok(storage) = state.storage.lock() {
        if let Ok(mut project) = storage.load_project(session.project_id) {
            let sess = project
                .sessions
                .iter()
                .find(|s| s.id == session.session_id)
                .cloned();
            project.sessions.retain(|s| s.id != session.session_id);
            let _ = storage.save_project(&project);

            if let Some(sess) = sess {
                if let (Some(wt_path), Some(branch)) = (sess.worktree_path, sess.worktree_branch) {
                    let _ = crate::worktree::WorktreeManager::remove_worktree(
                        &wt_path,
                        &project.repo_path,
                        &branch,
                    );
                }
            }
        }
    }
}

/// Check if an issue is eligible for auto-worker processing.
pub fn is_eligible(issue: &GithubIssue) -> bool {
    let labels: Vec<&str> = issue.labels.iter().map(|l| l.name.as_str()).collect();
    labels.contains(&LABEL_PRIORITY_HIGH)
        && labels.contains(&LABEL_COMPLEXITY_LOW)
        && !labels.contains(&LABEL_IN_PROGRESS)
        && !labels.contains(&LABEL_FINISHED_BY_WORKER)
        && !labels.contains(&LABEL_ASSIGNED_TO_AUTO_WORKER)
}

/// Pick the first eligible issue from a list.
pub fn pick_eligible_issue(issues: &[GithubIssue]) -> Option<&GithubIssue> {
    issues.iter().find(|i| is_eligible(i))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::GithubLabel;
    use crate::storage::Storage;
    use tempfile::TempDir;

    fn make_issue(labels: &[&str]) -> GithubIssue {
        GithubIssue {
            number: 1,
            title: "Test".to_string(),
            url: "https://github.com/o/r/issues/1".to_string(),
            body: None,
            labels: labels
                .iter()
                .map(|l| GithubLabel {
                    name: l.to_string(),
                })
                .collect(),
        }
    }

    #[test]
    fn eligible_issue_has_all_required_labels() {
        let issue = make_issue(&["priority:high", "complexity:low", "triaged"]);
        assert!(is_eligible(&issue));
    }

    #[test]
    fn standardized_issue_is_eligible() {
        let issue = make_issue(&["priority:high", "complexity:low"]);
        assert!(is_eligible(&issue));
    }

    #[test]
    fn missing_priority_high_not_eligible() {
        let issue = make_issue(&["complexity:low", "triaged"]);
        assert!(!is_eligible(&issue));
    }

    #[test]
    fn missing_complexity_low_not_eligible() {
        let issue = make_issue(&["priority:high", "triaged"]);
        assert!(!is_eligible(&issue));
    }

    #[test]
    fn legacy_labels_are_not_eligible() {
        let issue = make_issue(&["priority: high", "complexity: low", "triaged"]);
        assert!(!is_eligible(&issue));
    }

    #[test]
    fn triaged_not_required_for_eligibility() {
        let issue = make_issue(&["priority:high", "complexity:low"]);
        assert!(is_eligible(&issue));
    }

    #[test]
    fn in_progress_not_eligible() {
        let issue = make_issue(&["priority:high", "complexity:low", "triaged", "in-progress"]);
        assert!(!is_eligible(&issue));
    }

    #[test]
    fn finished_by_worker_not_eligible() {
        let issue = make_issue(&[
            "priority:high",
            "complexity:low",
            "triaged",
            "finished-by-worker",
        ]);
        assert!(!is_eligible(&issue));
    }

    #[test]
    fn pick_eligible_returns_first_match() {
        let issues = vec![
            make_issue(&["priority:low"]),
            make_issue(&["priority:high", "complexity:low"]),
        ];
        let picked = pick_eligible_issue(&issues);
        assert!(picked.is_some());
    }

    #[test]
    fn pick_eligible_returns_none_when_no_match() {
        let issues = vec![
            make_issue(&["priority:low"]),
            make_issue(&["in-progress", "priority:high", "complexity:low"]),
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

    #[test]
    fn successful_label_edit_invalidates_issue_cache() {
        let tmp = TempDir::new().unwrap();
        let state = AppState::from_storage(
            Storage::new(tmp.path().to_path_buf()),
            crate::emitter::NoopEmitter::new(),
        )
        .unwrap();
        let repo_path = "/tmp/repo";

        {
            let mut cache = state.issue_cache.lock().unwrap();
            cache.insert(repo_path.to_string(), vec![make_issue(&["in-progress"])]);
        }

        finish_label_edit(&state, repo_path, || Ok(())).unwrap();

        let cache = state.issue_cache.lock().unwrap();
        assert!(cache.get(repo_path).is_none());
    }

    #[test]
    fn failed_label_edit_keeps_issue_cache() {
        let tmp = TempDir::new().unwrap();
        let state = AppState::from_storage(
            Storage::new(tmp.path().to_path_buf()),
            crate::emitter::NoopEmitter::new(),
        )
        .unwrap();
        let repo_path = "/tmp/repo";

        {
            let mut cache = state.issue_cache.lock().unwrap();
            cache.insert(repo_path.to_string(), vec![make_issue(&["in-progress"])]);
        }

        let error = finish_label_edit(&state, repo_path, || Err("boom".to_string())).unwrap_err();

        assert_eq!(error, "boom");
        let cache = state.issue_cache.lock().unwrap();
        assert!(cache.get(repo_path).is_some());
    }

    #[test]
    fn worker_claim_label_plan_adds_assigned_to_auto_worker() {
        let plan = worker_claim_label_plan();

        assert!(plan.add.contains(&LABEL_IN_PROGRESS));
        assert!(plan.add.contains(&LABEL_ASSIGNED_TO_AUTO_WORKER));
    }

    #[test]
    fn worker_cleanup_label_plan_keeps_assignment_when_issue_closed() {
        let plan = worker_cleanup_label_plan(Some(true));

        assert!(plan.remove.contains(&LABEL_IN_PROGRESS));
        assert!(!plan.remove.contains(&LABEL_ASSIGNED_TO_AUTO_WORKER));
    }

    #[test]
    fn worker_cleanup_label_plan_removes_assignment_when_issue_still_open() {
        let plan = worker_cleanup_label_plan(Some(false));

        assert!(plan.remove.contains(&LABEL_IN_PROGRESS));
        assert!(plan.remove.contains(&LABEL_ASSIGNED_TO_AUTO_WORKER));
    }

    #[test]
    fn worker_cleanup_label_plan_preserves_assignment_when_issue_state_unknown() {
        let plan = worker_cleanup_label_plan(None);

        assert!(plan.remove.contains(&LABEL_IN_PROGRESS));
        assert!(!plan.remove.contains(&LABEL_ASSIGNED_TO_AUTO_WORKER));
    }

    #[test]
    fn label_definition_includes_assigned_to_auto_worker() {
        assert_eq!(
            label_definition(LABEL_ASSIGNED_TO_AUTO_WORKER),
            Some(LabelDefinition {
                description: "Issue has been handled by the auto-worker",
                color: "94E2D5",
            })
        );
    }

    #[test]
    fn startup_restoration_keeps_one_live_worker_and_cleans_stale_duplicates() {
        let project_id = Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap();
        let repo_path = "/tmp/the-controller".to_string();

        let reconciliation = reconcile_startup_workers(vec![
            StartupWorkerCandidate {
                session_id: Uuid::parse_str("11111111-1111-1111-1111-111111111111").unwrap(),
                project_id,
                issue_number: 328,
                issue_title: "Already finished duplicate".to_string(),
                repo_path: repo_path.clone(),
                session_dir: "/tmp/the-controller/session-46".to_string(),
                kind: "codex".to_string(),
                ordinal: 0,
                live_session: false,
            },
            StartupWorkerCandidate {
                session_id: Uuid::parse_str("22222222-2222-2222-2222-222222222222").unwrap(),
                project_id,
                issue_number: 327,
                issue_title: "Current live task".to_string(),
                repo_path: repo_path.clone(),
                session_dir: "/tmp/the-controller/session-51".to_string(),
                kind: "codex".to_string(),
                ordinal: 1,
                live_session: true,
            },
        ]);

        assert_eq!(reconciliation.restore.len(), 1);
        assert_eq!(reconciliation.restore[0].issue_number, 327);

        assert_eq!(reconciliation.cleanup.len(), 1);
        assert_eq!(reconciliation.cleanup[0].issue_number, 328);
    }

    #[test]
    fn startup_restoration_resumes_non_live_session_when_no_live_exists() {
        let project_id = Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap();
        let repo_path = "/tmp/the-controller".to_string();

        let reconciliation = reconcile_startup_workers(vec![
            StartupWorkerCandidate {
                session_id: Uuid::parse_str("11111111-1111-1111-1111-111111111111").unwrap(),
                project_id,
                issue_number: 328,
                issue_title: "Older session".to_string(),
                repo_path: repo_path.clone(),
                session_dir: "/tmp/the-controller/session-46".to_string(),
                kind: "codex".to_string(),
                ordinal: 0,
                live_session: false,
            },
            StartupWorkerCandidate {
                session_id: Uuid::parse_str("22222222-2222-2222-2222-222222222222").unwrap(),
                project_id,
                issue_number: 327,
                issue_title: "Most recent session".to_string(),
                repo_path: repo_path.clone(),
                session_dir: "/tmp/the-controller/session-51".to_string(),
                kind: "codex".to_string(),
                ordinal: 1,
                live_session: false,
            },
        ]);

        // Should restore the highest-ordinal candidate even though none are live
        assert_eq!(reconciliation.restore.len(), 1);
        assert_eq!(reconciliation.restore[0].issue_number, 327);

        assert_eq!(reconciliation.cleanup.len(), 1);
        assert_eq!(reconciliation.cleanup[0].issue_number, 328);
    }

    #[test]
    fn startup_restoration_failed_attach_moves_worker_to_cleanup() {
        let project_id = Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap();
        let candidate = StartupWorkerCandidate {
            session_id: Uuid::parse_str("22222222-2222-2222-2222-222222222222").unwrap(),
            project_id,
            issue_number: 327,
            issue_title: "Current live task".to_string(),
            repo_path: "/tmp/the-controller".to_string(),
            session_dir: "/tmp/the-controller/session-51".to_string(),
            kind: "codex".to_string(),
            ordinal: 1,
            live_session: true,
        };
        let reconciliation = StartupReconciliation {
            restore: vec![candidate.clone()],
            cleanup: vec![],
        };

        let finalized = finalize_startup_restoration(reconciliation, &HashSet::new());

        assert!(finalized.restore.is_empty());
        assert_eq!(finalized.cleanup, vec![candidate]);
    }
}
