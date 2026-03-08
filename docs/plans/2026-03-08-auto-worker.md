# Auto-Worker Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use the-controller-executing-plans to implement this plan task-by-task.

**Goal:** Add a per-project auto-worker that continuously processes issues labeled `priority: high` + `complexity: low` without manual intervention.

**Architecture:** A new Rust background thread (`AutoWorkerScheduler`) monitors enabled projects, picks eligible issues via the existing GitHub issue cache, spawns full PTY sessions using existing `create_session` infrastructure, and manages session lifecycle (idle detection, auto-nudge, hard timeout). Frontend adds chord keybindings (`o` then `m`/`w`) and extends the maintainer panel with an auto-worker section.

**Tech Stack:** Rust (Tauri v2), Svelte 5, existing PTY/tmux/worktree infrastructure

---

### Task 1: Add AutoWorkerConfig to Project model

**Files:**
- Modify: `src-tauri/src/models.rs`

**Step 1: Add `AutoWorkerConfig` struct and field to `Project`**

In `src-tauri/src/models.rs`, add after `MaintainerConfig` (line 29):

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutoWorkerConfig {
    pub enabled: bool,
}

impl Default for AutoWorkerConfig {
    fn default() -> Self {
        Self { enabled: false }
    }
}
```

Add to `Project` struct after `maintainer` field (line 13):

```rust
    #[serde(default)]
    pub auto_worker: AutoWorkerConfig,
```

**Step 2: Add test for default deserialization**

In the `tests` module in `models.rs`, add:

```rust
#[test]
fn test_auto_worker_config_defaults_when_absent() {
    let json = r#"{
        "id": "550e8400-e29b-41d4-a716-446655440000",
        "name": "test-project",
        "repo_path": "/tmp/test-repo",
        "created_at": "2026-02-28T00:00:00Z",
        "archived": false,
        "sessions": []
    }"#;
    let project: Project = serde_json::from_str(json).expect("deserialize");
    assert!(!project.auto_worker.enabled);
}

#[test]
fn test_auto_worker_config_roundtrip() {
    let project = Project {
        id: Uuid::new_v4(),
        name: "test".to_string(),
        repo_path: "/tmp".to_string(),
        created_at: "2026-03-08T00:00:00Z".to_string(),
        archived: false,
        maintainer: MaintainerConfig::default(),
        auto_worker: AutoWorkerConfig { enabled: true },
        sessions: vec![],
    };
    let json = serde_json::to_string(&project).expect("serialize");
    let deserialized: Project = serde_json::from_str(&json).expect("deserialize");
    assert!(deserialized.auto_worker.enabled);
}
```

**Step 3: Fix existing tests that construct `Project` literals**

Every existing test in `models.rs` and `commands.rs` that constructs a `Project` needs the new `auto_worker` field. Add `auto_worker: AutoWorkerConfig::default()` to each.

**Step 4: Run Rust tests**

Run: `cd src-tauri && cargo test`
Expected: All tests pass, including the two new ones.

**Step 5: Commit**

```bash
git add src-tauri/src/models.rs
git commit -m "feat: add AutoWorkerConfig to Project model"
```

---

### Task 2: Add issue eligibility filter

**Files:**
- Create: `src-tauri/src/auto_worker.rs`
- Modify: `src-tauri/src/lib.rs` (add `pub mod auto_worker;`)

**Step 1: Write failing tests for issue eligibility**

Create `src-tauri/src/auto_worker.rs`:

```rust
use crate::models::GithubIssue;

/// Check if an issue is eligible for auto-worker processing.
/// Eligible = has all of: `priority: high`, `complexity: low`, `triaged`
/// and none of: `in-progress`, `finished-by-worker`
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
            make_issue(&["priority: high", "complexity: low", "triaged"]),
        ];
        let picked = pick_eligible_issue(&issues);
        assert!(picked.is_some());
        assert_eq!(picked.unwrap().number, 1); // all have number 1 from make_issue
    }

    #[test]
    fn pick_eligible_returns_none_when_no_match() {
        let issues = vec![
            make_issue(&["priority: low"]),
            make_issue(&["in-progress", "priority: high", "complexity: low", "triaged"]),
        ];
        assert!(pick_eligible_issue(&issues).is_none());
    }
}
```

**Step 2: Register the module**

In `src-tauri/src/lib.rs`, add after `pub mod commands;` (line 3):

```rust
pub mod auto_worker;
```

**Step 3: Run tests**

Run: `cd src-tauri && cargo test auto_worker`
Expected: All 8 tests pass.

**Step 4: Commit**

```bash
git add src-tauri/src/auto_worker.rs src-tauri/src/lib.rs
git commit -m "feat: add issue eligibility filter for auto-worker"
```

---

### Task 3: Strengthen background worker prompt for autonomy

**Files:**
- Modify: `src-tauri/src/session_args.rs`

**Step 1: Update the `BACKGROUND_WORKFLOW_SUFFIX` constant**

In `src-tauri/src/session_args.rs` (line 3), replace the existing `BACKGROUND_WORKFLOW_SUFFIX`:

```rust
const BACKGROUND_WORKFLOW_SUFFIX: &str = "\n\nYou are an autonomous background worker. Complete the following workflow end-to-end without waiting for user input:\n1. **Design** — Analyze the issue and plan the approach\n2. **Implement** — Write the code changes\n3. **Review** — Self-review the changes for correctness and quality\n4. **Push PR** — Create and push a pull request\n5. **Merge** — Merge the PR once checks pass\n6. **Sync local master** — Pull merged changes to local master\n\nCRITICAL: Never ask questions. Never wait for confirmation or user input. If you are uncertain about anything, make your best judgment and proceed. You must complete the entire workflow autonomously.";
```

**Step 2: Update test**

Update `build_issue_prompt_with_background` test to also check for the new text:

```rust
assert!(prompt.contains("Never ask questions"));
```

**Step 3: Run tests**

Run: `cd src-tauri && cargo test session_args`
Expected: All tests pass.

**Step 4: Commit**

```bash
git add src-tauri/src/session_args.rs
git commit -m "feat: strengthen background worker prompt for full autonomy"
```

---

### Task 4: Add `configure_auto_worker` and `get_auto_worker_status` commands

**Files:**
- Modify: `src-tauri/src/commands.rs`
- Modify: `src-tauri/src/lib.rs` (register commands)

**Step 1: Add the configure command**

In `src-tauri/src/commands.rs`, add after the `configure_maintainer` function (after line 1133):

```rust
#[tauri::command]
pub async fn configure_auto_worker(
    state: State<'_, AppState>,
    project_id: String,
    enabled: bool,
) -> Result<(), String> {
    let project_id = Uuid::parse_str(&project_id).map_err(|e| e.to_string())?;
    let storage = state.storage.lock().map_err(|e| e.to_string())?;
    let mut project = storage.load_project(project_id).map_err(|e| e.to_string())?;
    project.auto_worker.enabled = enabled;
    storage.save_project(&project).map_err(|e| e.to_string())?;
    Ok(())
}
```

**Step 2: Register the command**

In `src-tauri/src/lib.rs`, add `commands::configure_auto_worker` to the `invoke_handler` array (after `commands::trigger_maintainer_check`).

**Step 3: Run Rust tests**

Run: `cd src-tauri && cargo test`
Expected: All tests pass (compile check — no integration test for Tauri commands).

**Step 4: Commit**

```bash
git add src-tauri/src/commands.rs src-tauri/src/lib.rs
git commit -m "feat: add configure_auto_worker Tauri command"
```

---

### Task 5: Build the AutoWorkerScheduler background thread

**Files:**
- Modify: `src-tauri/src/auto_worker.rs`
- Modify: `src-tauri/src/lib.rs` (start scheduler)

This is the core logic. The scheduler:
1. Polls every 30 seconds
2. For each enabled project, checks if there's already an active auto-worker session
3. If not, fetches issues via `gh issue list`, picks an eligible one, spawns a session
4. Monitors active sessions for idle (auto-nudge) and timeout (kill)

**Step 1: Add imports and state tracking to `auto_worker.rs`**

Add at the top of `auto_worker.rs`:

```rust
use std::collections::HashMap;
use std::process::Command;
use std::time::{Duration, Instant};

use tauri::{AppHandle, Emitter, Manager};
use uuid::Uuid;

use crate::models::{GithubIssue, GithubLabel};
use crate::state::AppState;
```

**Step 2: Add AutoWorkerScheduler**

After the `pick_eligible_issue` function, add:

```rust
const POLL_INTERVAL_SECS: u64 = 30;
const SESSION_TIMEOUT_SECS: u64 = 30 * 60; // 30 minutes
const MAX_NUDGES: u32 = 3;
const NUDGE_COOLDOWN_SECS: u64 = 60; // Wait 60s between nudges

struct ActiveSession {
    session_id: Uuid,
    project_id: Uuid,
    issue_number: u64,
    issue_url: String,
    repo_path: String,
    spawned_at: Instant,
    nudge_count: u32,
    last_idle_at: Option<Instant>,
    last_nudge_at: Option<Instant>,
}

pub struct AutoWorkerScheduler;

impl AutoWorkerScheduler {
    pub fn start(app_handle: AppHandle) {
        std::thread::spawn(move || {
            // Track active auto-worker sessions per project
            let mut active_sessions: HashMap<Uuid, ActiveSession> = HashMap::new();

            loop {
                std::thread::sleep(Duration::from_secs(POLL_INTERVAL_SECS));

                let state = match app_handle.try_state::<AppState>() {
                    Some(s) => s,
                    None => continue,
                };

                // Check for timed-out or stuck sessions
                let timed_out: Vec<Uuid> = active_sessions
                    .iter()
                    .filter(|(_, s)| s.spawned_at.elapsed() > Duration::from_secs(SESSION_TIMEOUT_SECS))
                    .map(|(project_id, _)| *project_id)
                    .collect();

                for project_id in timed_out {
                    if let Some(session) = active_sessions.remove(&project_id) {
                        kill_session(&state, &app_handle, &session);
                        let _ = app_handle.emit(
                            &format!("auto-worker-status:{}", project_id),
                            serde_json::json!({ "status": "idle", "message": "Session timed out" }).to_string(),
                        );
                    }
                }

                // Handle nudging idle sessions
                let idle_sessions: Vec<Uuid> = active_sessions
                    .iter()
                    .filter(|(_, s)| {
                        s.last_idle_at.is_some()
                            && s.last_nudge_at.map_or(true, |t| t.elapsed() > Duration::from_secs(NUDGE_COOLDOWN_SECS))
                    })
                    .map(|(project_id, _)| *project_id)
                    .collect();

                for project_id in idle_sessions {
                    if let Some(session) = active_sessions.get_mut(&project_id) {
                        if session.nudge_count >= MAX_NUDGES {
                            let session = active_sessions.remove(&project_id).unwrap();
                            kill_session(&state, &app_handle, &session);
                            let _ = app_handle.emit(
                                &format!("auto-worker-status:{}", project_id),
                                serde_json::json!({ "status": "idle", "message": "Killed after max nudges" }).to_string(),
                            );
                        } else {
                            nudge_session(&state, session);
                        }
                    }
                }

                // Check completed sessions (PTY exited)
                let exited: Vec<Uuid> = active_sessions
                    .iter()
                    .filter(|(_, s)| {
                        let pty_manager = match state.pty_manager.lock() {
                            Ok(m) => m,
                            Err(_) => return false,
                        };
                        !pty_manager.is_alive(s.session_id) && !pty_manager.sessions.contains_key(&s.session_id)
                    })
                    .map(|(project_id, _)| *project_id)
                    .collect();

                for project_id in exited {
                    if let Some(session) = active_sessions.remove(&project_id) {
                        // Mark issue as finished
                        mark_issue_finished(&session);
                        // Clean up worktree and session config
                        cleanup_session(&state, &session);
                        let _ = app_handle.emit(
                            &format!("auto-worker-status:{}", project_id),
                            serde_json::json!({ "status": "idle", "message": format!("Completed #{}", session.issue_number) }).to_string(),
                        );
                    }
                }

                // For each enabled project without an active session, try to pick an issue
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

                    // Fetch issues
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

                    // Spawn session
                    match spawn_auto_worker_session(&state, &app_handle, project, &eligible) {
                        Ok(session_id) => {
                            active_sessions.insert(project.id, ActiveSession {
                                session_id,
                                project_id: project.id,
                                issue_number: eligible.number,
                                issue_url: eligible.url.clone(),
                                repo_path: project.repo_path.clone(),
                                spawned_at: Instant::now(),
                                nudge_count: 0,
                                last_idle_at: None,
                                last_nudge_at: None,
                            });
                            // Add in-progress label (fire and forget)
                            let _ = add_label_sync(&project.repo_path, eligible.number, "in-progress");
                            let _ = app_handle.emit(
                                &format!("auto-worker-status:{}", project.id),
                                serde_json::json!({
                                    "status": "working",
                                    "issue_number": eligible.number,
                                    "issue_title": eligible.title,
                                }).to_string(),
                            );
                        }
                        Err(e) => {
                            eprintln!("Auto-worker: failed to spawn session for #{}: {}", eligible.number, e);
                        }
                    }
                }

                // Listen for idle hook events on active sessions
                // We check via the PTY manager's alive status and session-status-hook events
                // The status socket sends idle/working messages — we detect idle by checking
                // if the session is alive but not in the PTY session map (tmux detached = exited)
                // For idle detection, we use a simpler heuristic: if the PTY is alive but
                // hasn't sent a "working" hook in NUDGE_COOLDOWN_SECS, consider it idle.
                // Since we can't directly listen to events from this thread, we check PTY aliveness.
            }
        });
    }
}

/// Fetch issues synchronously using `gh issue list`.
fn fetch_issues_sync(repo_path: &str) -> Result<Vec<GithubIssue>, String> {
    let output = Command::new("gh")
        .args([
            "issue", "list", "--json", "number,title,url,body,labels", "--limit", "50",
        ])
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

/// Add a label to an issue synchronously.
fn add_label_sync(repo_path: &str, issue_number: u64, label: &str) -> Result<(), String> {
    let output = Command::new("gh")
        .args([
            "issue", "edit", &issue_number.to_string(), "--add-label", label,
        ])
        .current_dir(repo_path)
        .output()
        .map_err(|e| format!("Failed to run gh: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("gh issue edit failed: {}", stderr));
    }
    Ok(())
}

/// Remove a label from an issue synchronously.
fn remove_label_sync(repo_path: &str, issue_number: u64, label: &str) -> Result<(), String> {
    let output = Command::new("gh")
        .args([
            "issue", "edit", &issue_number.to_string(), "--remove-label", label,
        ])
        .current_dir(repo_path)
        .output()
        .map_err(|e| format!("Failed to run gh: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("gh issue edit failed: {}", stderr));
    }
    Ok(())
}

/// Spawn a new auto-worker session for an issue.
fn spawn_auto_worker_session(
    state: &AppState,
    app_handle: &AppHandle,
    project: &crate::models::Project,
    issue: &GithubIssue,
) -> Result<Uuid, String> {
    let session_id = Uuid::new_v4();

    let label = crate::commands::next_session_label(&project.sessions);

    // Create worktree
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

    // Save session config (marked as auto-worker via a convention — not shown in sidebar)
    {
        let storage = state.storage.lock().map_err(|e| e.to_string())?;
        let mut proj = storage.load_project(project.id).map_err(|e| e.to_string())?;
        let session_config = crate::models::SessionConfig {
            id: session_id,
            label: label.clone(),
            worktree_path: wt_path,
            worktree_branch: wt_branch,
            archived: false,
            kind: "claude".to_string(),
            github_issue: Some(issue.clone()),
            initial_prompt: Some(initial_prompt.clone()),
            done_commits: vec![],
        };
        proj.sessions.push(session_config);
        storage.save_project(&proj).map_err(|e| e.to_string())?;
    }

    // Spawn the PTY
    let mut pty_manager = state.pty_manager.lock().map_err(|e| e.to_string())?;
    pty_manager.spawn_session(
        session_id,
        &session_dir,
        "claude",
        app_handle.clone(),
        false,
        Some(&initial_prompt),
        24,
        80,
    )?;

    Ok(session_id)
}

/// Send a nudge message to a stuck session.
fn nudge_session(state: &AppState, session: &mut ActiveSession) {
    let nudge_msg = "\nContinue working autonomously. Do not ask questions or wait for input. Complete the task.\n";
    if let Ok(mut pty_manager) = state.pty_manager.lock() {
        let _ = pty_manager.write_to_session(session.session_id, nudge_msg.as_bytes());
    }
    session.nudge_count += 1;
    session.last_nudge_at = Some(Instant::now());
}

/// Kill a timed-out or stuck session.
fn kill_session(state: &AppState, _app_handle: &AppHandle, session: &ActiveSession) {
    // Close PTY/tmux
    if let Ok(mut pty_manager) = state.pty_manager.lock() {
        let _ = pty_manager.close_session(session.session_id);
    }
    // Remove in-progress label
    let _ = remove_label_sync(&session.repo_path, session.issue_number, "in-progress");
    // Cleanup session config and worktree
    cleanup_session(state, session);
}

/// Mark an issue as finished by the worker.
fn mark_issue_finished(session: &ActiveSession) {
    let _ = add_label_sync(&session.repo_path, session.issue_number, "finished-by-worker");
    let _ = remove_label_sync(&session.repo_path, session.issue_number, "in-progress");
}

/// Clean up session config and worktree.
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
```

**Step 3: Start the scheduler in `lib.rs`**

In `src-tauri/src/lib.rs`, in the `setup` closure (after line 25 `maintainer::MaintainerScheduler::start`):

```rust
auto_worker::AutoWorkerScheduler::start(app.handle().clone());
```

**Step 4: Run Rust tests**

Run: `cd src-tauri && cargo test`
Expected: All tests pass (no new unit tests for scheduler itself — it requires app runtime).

**Step 5: Commit**

```bash
git add src-tauri/src/auto_worker.rs src-tauri/src/lib.rs
git commit -m "feat: add AutoWorkerScheduler background thread"
```

---

### Task 6: Add idle detection via status socket events

The auto-worker needs to know when a session goes idle (Claude stopped and is waiting for input). The status socket already emits `session-status-hook:{session_id}` events. We need to bridge these to the auto-worker thread.

**Files:**
- Modify: `src-tauri/src/auto_worker.rs`
- Modify: `src-tauri/src/status_socket.rs`

**Step 1: Add a shared idle-notification channel**

In `src-tauri/src/auto_worker.rs`, add a static channel at the top:

```rust
use std::sync::mpsc;
use once_cell::sync::Lazy;

/// Channel for the status socket to notify the auto-worker of idle sessions.
static IDLE_CHANNEL: Lazy<(Mutex<mpsc::Sender<Uuid>>, Mutex<mpsc::Receiver<Uuid>>)> = Lazy::new(|| {
    let (tx, rx) = mpsc::channel();
    (Mutex::new(tx), Mutex::new(rx))
});

pub fn notify_session_idle(session_id: Uuid) {
    if let Ok(tx) = IDLE_CHANNEL.0.lock() {
        let _ = tx.send(session_id);
    }
}
```

Note: Add `once_cell` to `Cargo.toml` dependencies if not already present. Check first:

Run: `grep once_cell src-tauri/Cargo.toml`

If not present, add `once_cell = "1"` to `[dependencies]`.

**Step 2: Call `notify_session_idle` from the status socket**

In `src-tauri/src/status_socket.rs`, in `handle_connection` (around line 84-86), after the `else` branch that emits `session-status-hook`, add:

```rust
if status == "idle" {
    crate::auto_worker::notify_session_idle(session_id);
}
```

**Step 3: Drain the idle channel in the scheduler loop**

In the scheduler loop in `auto_worker.rs`, right after `std::thread::sleep(Duration::from_secs(POLL_INTERVAL_SECS));`, add:

```rust
// Drain idle notifications
if let Ok(rx) = IDLE_CHANNEL.1.lock() {
    while let Ok(session_id) = rx.try_recv() {
        for session in active_sessions.values_mut() {
            if session.session_id == session_id {
                session.last_idle_at = Some(Instant::now());
            }
        }
    }
}
```

**Step 4: Run Rust tests**

Run: `cd src-tauri && cargo test`
Expected: All tests pass.

**Step 5: Commit**

```bash
git add src-tauri/src/auto_worker.rs src-tauri/src/status_socket.rs src-tauri/Cargo.toml
git commit -m "feat: bridge status socket idle events to auto-worker"
```

---

### Task 7: Add frontend AutoWorkerConfig to stores

**Files:**
- Modify: `src/lib/stores.ts`

**Step 1: Add the types and store**

In `src/lib/stores.ts`, add `AutoWorkerConfig` interface after `MaintainerConfig` (around line 30):

```typescript
export interface AutoWorkerConfig {
  enabled: boolean;
}
```

Add `auto_worker` to the `Project` interface (after `maintainer: MaintainerConfig`):

```typescript
  auto_worker: AutoWorkerConfig;
```

Add a new store for auto-worker statuses (after `maintainerPanelVisible`):

```typescript
export type AutoWorkerStatus = {
  status: "idle" | "working";
  message?: string;
  issue_number?: number;
  issue_title?: string;
};
export const autoWorkerStatuses = writable<Map<string, AutoWorkerStatus>>(new Map());
```

**Step 2: Run frontend tests**

Run: `npx vitest run`
Expected: All tests pass. Some tests that reference the `Project` type may need updating if they construct mock projects — check for failures and fix by adding `auto_worker: { enabled: false }` to mock data.

**Step 3: Commit**

```bash
git add src/lib/stores.ts
git commit -m "feat: add AutoWorkerConfig and status store to frontend"
```

---

### Task 8: Add chord keybinding mode (`o` prefix)

**Files:**
- Modify: `src/lib/commands.ts`
- Modify: `src/lib/HotkeyManager.svelte`
- Modify: `src/lib/stores.ts` (add hotkey actions)
- Test: `src/lib/commands.test.ts`

**Step 1: Update commands.ts**

Replace the `toggle-maintainer` entry (line 84) with:

```typescript
  { id: "toggle-mode", key: "o", section: "Panels", description: "Toggle: (m)aintainer / (w)orker" },
```

Add new CommandId types. In the `CommandId` union (around line 25), replace `"toggle-maintainer"` with `"toggle-mode"`.

**Step 2: Add toggle mode to HotkeyManager**

In `src/lib/HotkeyManager.svelte`, add toggle mode state (after jump mode state, around line 33):

```typescript
let toggleModeActive = $state(false);
```

Add a handler for toggle mode keys (after `handleJumpKey`, around line 144):

```typescript
function handleToggleKey(key: string) {
    toggleModeActive = false;
    if (key === "m") {
      dispatchAction({ type: "toggle-maintainer-enabled" });
      return;
    }
    if (key === "w") {
      dispatchAction({ type: "toggle-auto-worker-enabled" });
      return;
    }
    // Any other key cancels toggle mode
}
```

In `handleHotkey`, replace the `toggle-maintainer` case with:

```typescript
case "toggle-mode":
    toggleModeActive = true;
    return true;
```

In `onKeydown`, add toggle mode intercept (after the jump mode block, around line 394):

```typescript
if (toggleModeActive) {
    e.stopPropagation();
    e.preventDefault();
    handleToggleKey(e.key);
    pushKeystroke("o" + e.key);
    return;
}
```

**Step 3: Add the new HotkeyAction type**

In `src/lib/stores.ts`, add to the `HotkeyAction` union:

```typescript
| { type: "toggle-auto-worker-enabled" }
```

**Step 4: Update the commands test**

In `src/lib/commands.test.ts`, the help section entry counts may change. Update:
- The `buildKeyMap` test to expect `map.get("o")` to be `"toggle-mode"` (not `"toggle-maintainer"`)
- The panels help section count if it changed

**Step 5: Run frontend tests**

Run: `npx vitest run`
Expected: All tests pass.

**Step 6: Commit**

```bash
git add src/lib/commands.ts src/lib/HotkeyManager.svelte src/lib/stores.ts src/lib/commands.test.ts
git commit -m "feat: add chord keybinding mode (o+m/o+w) for toggles"
```

---

### Task 9: Wire auto-worker toggle in App.svelte

**Files:**
- Modify: `src/App.svelte`

**Step 1: Add the toggle handler**

In the `$effect` block that subscribes to `hotkeyAction` (around line 34), add a new case after the `toggle-maintainer-enabled` handler:

```typescript
} else if (action?.type === "toggle-auto-worker-enabled") {
    toggleAutoWorkerEnabled();
}
```

Add the function (after `toggleMaintainerEnabled`):

```typescript
async function toggleAutoWorkerEnabled() {
    const focus = focusTargetState.current;
    if (!focus || (focus.type !== "project" && focus.type !== "session")) return;
    const project = projectsState.current.find((p) => p.id === focus.projectId);
    if (!project) return;
    const newEnabled = !project.auto_worker.enabled;
    try {
        await invoke("configure_auto_worker", {
            projectId: project.id,
            enabled: newEnabled,
        });
        const result: Project[] = await invoke("list_projects");
        projects.set(result);
        showToast(`Auto-worker ${newEnabled ? "enabled" : "disabled"}`, "info");
    } catch (e) {
        showToast(String(e), "error");
    }
}
```

**Step 2: Commit**

```bash
git add src/App.svelte
git commit -m "feat: wire auto-worker toggle to App.svelte"
```

---

### Task 10: Add auto-worker section to MaintainerPanel

**Files:**
- Modify: `src/lib/MaintainerPanel.svelte`
- Modify: `src/lib/Sidebar.svelte` (listen for `auto-worker-status` events)

**Step 1: Listen for auto-worker-status events in Sidebar**

In `src/lib/Sidebar.svelte`, in the `$effect` that sets up event listeners (around line 243), after the `maintainer-status` listener, add:

```typescript
listen<string>(`auto-worker-status:${project.id}`, (event) => {
    try {
        const status = JSON.parse(event.payload);
        autoWorkerStatuses.update(m => {
            const next = new Map(m);
            next.set(project.id, status);
            return next;
        });
    } catch { /* ignore parse errors */ }
}).then(unlisten => { if (!cancelled) unlisteners.push(unlisten); else unlisten(); });
```

Add `autoWorkerStatuses` to the imports from `./stores`.

**Step 2: Add auto-worker section to MaintainerPanel**

In `src/lib/MaintainerPanel.svelte`, import `autoWorkerStatuses` and `type AutoWorkerStatus` from stores.

Add derived state:

```typescript
const autoWorkerStatusesState = fromStore(autoWorkerStatuses);
let autoWorkerStatusMap: Map<string, AutoWorkerStatus> = $derived(autoWorkerStatusesState.current);
let autoWorkerStatus: AutoWorkerStatus | null = $derived(
    project ? (autoWorkerStatusMap.get(project.id) ?? null) : null
);
```

Add the toggle function:

```typescript
async function toggleAutoWorker() {
    if (!project) return;
    const newEnabled = !project.auto_worker.enabled;
    try {
        await invoke("configure_auto_worker", {
            projectId: project.id,
            enabled: newEnabled,
        });
        const result: Project[] = await invoke("list_projects");
        projects.set(result);
    } catch (e) {
        showToast(String(e), "error");
    }
}
```

Add the HTML section after the maintainer panel-actions div (before the closing `</aside>`):

```svelte
<div class="panel-divider"></div>
<div class="panel-header">
    <span class="panel-title">Auto-worker</span>
    {#if autoWorkerStatus?.status === "working"}
        <span class="maintainer-status running">Working</span>
    {/if}
    {#if project}
        <button class="btn-toggle" class:enabled={project.auto_worker.enabled} onclick={toggleAutoWorker}>
            {project.auto_worker.enabled ? "ON" : "OFF"}
        </button>
    {/if}
</div>

{#if project?.auto_worker.enabled}
    <div class="auto-worker-info">
        {#if autoWorkerStatus?.status === "working"}
            <div class="worker-current">
                <span class="worker-label">Working on:</span>
                <span class="worker-issue">#{autoWorkerStatus.issue_number} {autoWorkerStatus.issue_title}</span>
            </div>
        {:else}
            <div class="status">Waiting for eligible issues (priority: high + complexity: low)</div>
        {/if}
    </div>
{:else if project}
    <div class="status">Disabled — press o then w to enable</div>
{/if}
```

Add styles:

```css
.panel-divider {
    border-top: 2px solid #313244;
    margin: 8px 0;
}

.auto-worker-info {
    padding: 8px 16px;
    font-size: 12px;
}

.worker-current {
    display: flex;
    flex-direction: column;
    gap: 4px;
}

.worker-label {
    color: #6c7086;
    font-size: 11px;
}

.worker-issue {
    color: #cdd6f4;
}
```

**Step 3: Commit**

```bash
git add src/lib/MaintainerPanel.svelte src/lib/Sidebar.svelte
git commit -m "feat: add auto-worker section to maintainer panel"
```

---

### Task 11: Hide auto-worker sessions from sidebar

**Files:**
- Modify: `src/lib/Sidebar.svelte`
- Modify: `src-tauri/src/models.rs` (add `auto_worker_session` field to `SessionConfig`)

**Step 1: Add `auto_worker_session` flag to SessionConfig**

In `src-tauri/src/models.rs`, add to `SessionConfig`:

```rust
    #[serde(default)]
    pub auto_worker_session: bool,
```

In `src/lib/stores.ts`, add to `SessionConfig` interface:

```typescript
  auto_worker_session: boolean;
```

**Step 2: Set the flag when spawning auto-worker sessions**

In `src-tauri/src/auto_worker.rs`, in `spawn_auto_worker_session`, set `auto_worker_session: true` in the `SessionConfig` construction.

**Step 3: Filter auto-worker sessions from sidebar display**

In `src/lib/Sidebar.svelte`, wherever sessions are filtered for display (the `$effect` that builds visible items), add:

```typescript
.filter(s => !s.auto_worker_session)
```

to the session filtering chain. Find where sessions are iterated for display and exclude `auto_worker_session: true`.

**Step 4: Fix tests**

Update any tests that construct `SessionConfig` to include `auto_worker_session: false`.

**Step 5: Run all tests**

Run: `cd src-tauri && cargo test` and `npx vitest run`
Expected: All tests pass.

**Step 6: Commit**

```bash
git add src-tauri/src/models.rs src-tauri/src/auto_worker.rs src/lib/stores.ts src/lib/Sidebar.svelte
git commit -m "feat: hide auto-worker sessions from sidebar"
```

---

### Task 12: Final integration test and cleanup

**Step 1: Run full test suites**

```bash
cd src-tauri && cargo test
npx vitest run
```

Verify all tests pass.

**Step 2: Manual smoke test**

If `npm run tauri dev` is available:
1. Start the app
2. Create/select a project with a GitHub repo
3. Press `b` to open maintainer panel — verify auto-worker section appears
4. Press `o` then `w` — verify auto-worker toggles on, toast shows
5. Press `o` then `m` — verify maintainer toggles (existing functionality preserved)
6. If there are eligible issues (priority: high + complexity: low + triaged), verify a session is auto-spawned within 30 seconds
7. Verify the session does NOT appear in the sidebar
8. Verify the maintainer panel shows "Working on: #N title"

**Step 3: Final commit with closes tag**

```bash
git add -A
git commit -m "feat: add background coding agent for simple high-priority tasks

closes #176"
```
