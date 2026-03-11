use std::env;
use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::net::ToSocketAddrs;
use std::sync::Mutex;
use std::time::Duration;

use tempfile::TempDir;
use the_controller_lib::controller_chat::{send_message, update_focus_snapshot, ControllerChatItemKind, ControllerFocusUpdate};
use the_controller_lib::emitter::NoopEmitter;
use the_controller_lib::models::{AutoWorkerConfig, MaintainerConfig, Project};
use the_controller_lib::state::AppState;
use the_controller_lib::storage::Storage;
use uuid::Uuid;

static ENV_LOCK: Mutex<()> = Mutex::new(());
const RUNS_PER_SCENARIO: usize = 3;
const TURN_TIMEOUT_SECS: u64 = 90;

struct PathGuard {
    original_path: Option<OsString>,
}

impl Drop for PathGuard {
    fn drop(&mut self) {
        match &self.original_path {
            Some(path) => env::set_var("PATH", path),
            None => env::remove_var("PATH"),
        }
    }
}

fn run_command(command: &str, args: &[&str], cwd: Option<&Path>) -> Result<String, String> {
    let mut cmd = Command::new(command);
    cmd.args(args);
    if let Some(cwd) = cwd {
        cmd.current_dir(cwd);
    }

    let output = cmd
        .output()
        .map_err(|e| format!("Failed to run {}: {}", command, e))?;
    if !output.status.success() {
        return Err(format!(
            "{} {} failed: {}",
            command,
            args.join(" "),
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }

    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

fn prepend_path(bin_dir: &Path) -> PathGuard {
    let original_path = env::var_os("PATH");
    let mut paths = vec![bin_dir.to_path_buf()];
    if let Some(existing) = &original_path {
        paths.extend(env::split_paths(existing));
    }
    let joined = env::join_paths(paths).expect("join PATH");
    env::set_var("PATH", joined);
    PathGuard { original_path }
}

fn write_gh_stub(bin_dir: &Path) -> Result<(), String> {
    let gh_path = bin_dir.join("gh");
    fs::write(
        &gh_path,
        r#"#!/bin/sh
set -eu
if [ "$#" -ge 3 ] && [ "$1" = "issue" ] && [ "$2" = "view" ] && [ "$3" = "123" ]; then
  if printf '%s ' "$@" | grep -q -- '--json'; then
    printf '{"number":123,"title":"Seeded issue title","body":"Seeded issue body","url":"https://github.com/example/repo/issues/123"}'
  else
    printf 'title:\tSeeded issue title\nbody:\tSeeded issue body\nurl:\thttps://github.com/example/repo/issues/123\n'
  fi
  exit 0
fi
if [ "$#" -ge 3 ] && [ "$1" = "issue" ] && [ "$2" = "view" ] && [ "$3" = "404" ]; then
  echo "issue not found" >&2
  exit 1
fi
echo "unsupported gh invocation: $*" >&2
exit 1
"#,
    )
    .map_err(|e| format!("Failed to write gh stub: {}", e))?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&gh_path)
            .map_err(|e| format!("Failed to stat gh stub: {}", e))?
            .permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&gh_path, perms)
            .map_err(|e| format!("Failed to chmod gh stub: {}", e))?;
    }

    Ok(())
}

fn setup_repo(repo_dir: &Path) -> Result<(), String> {
    fs::create_dir_all(repo_dir).map_err(|e| format!("Failed to create repo dir: {}", e))?;
    fs::write(repo_dir.join("README.md"), "# controller chat validation\n")
        .map_err(|e| format!("Failed to write README: {}", e))?;
    run_command("git", &["init"], Some(repo_dir)).map(|_| ())
}

fn make_project(project_id: Uuid, project_name: &str, repo_path: &Path) -> Project {
    Project {
        id: project_id,
        name: project_name.to_string(),
        repo_path: repo_path.to_string_lossy().to_string(),
        created_at: "2026-03-10T00:00:00Z".to_string(),
        archived: false,
        sessions: vec![],
        maintainer: MaintainerConfig::default(),
        auto_worker: AutoWorkerConfig::default(),
        prompts: vec![],
        staged_session: None,
    }
}

fn make_state(storage_dir: &Path, project_name: &str, repo_dir: &Path) -> Result<(AppState, Uuid), String> {
    let project_id = Uuid::new_v4();
    let storage = Storage::new(storage_dir.to_path_buf());
    let state = AppState::from_storage(storage, NoopEmitter::new()).map_err(|e| e.to_string())?;

    {
        let storage = state.storage.lock().map_err(|e| e.to_string())?;
        storage
            .save_project(&make_project(project_id, project_name, repo_dir))
            .map_err(|e| e.to_string())?;
    }

    update_focus_snapshot(
        &state,
        ControllerFocusUpdate {
            project_id: Some(project_id),
            project_name: Some(project_name.to_string()),
            session_id: None,
            note_filename: None,
            workspace_mode: Some("notes".to_string()),
        },
    )?;

    Ok((state, project_id))
}

fn note_path(storage_dir: &Path, project_name: &str, filename: &str) -> PathBuf {
    storage_dir.join("notes").join(project_name).join(filename)
}

async fn send_message_with_timeout(state: &AppState, message: &str) -> Result<the_controller_lib::controller_chat::ControllerChatSession, String> {
    tokio::time::timeout(Duration::from_secs(TURN_TIMEOUT_SECS), send_message(state, message.to_string()))
        .await
        .map_err(|_| format!("controller chat turn timed out after {}s", TURN_TIMEOUT_SECS))?
}

async fn run_happy_path_once() -> Result<(), String> {
    let temp = TempDir::new().map_err(|e| e.to_string())?;
    let bin_dir = temp.path().join("bin");
    let repo_dir = temp.path().join("repo");
    let storage_dir = temp.path().join("storage");
    let project_name = "controller-chat-happy";
    fs::create_dir_all(&bin_dir).map_err(|e| e.to_string())?;
    write_gh_stub(&bin_dir)?;
    let _path_guard = prepend_path(&bin_dir);
    setup_repo(&repo_dir)?;
    let (state, _) = make_state(&storage_dir, project_name, &repo_dir)?;

    let session = send_message_with_timeout(
        &state,
        "Use gh to fetch GitHub issue 123. Create a note named issue-123.md containing the fetched title and body in markdown, then open that note. Return a short summary.",
    )
    .await?;

    let note = fs::read_to_string(note_path(&storage_dir, project_name, "issue-123.md"))
        .map_err(|e| format!("Failed to read created note: {}", e))?;
    if !note.contains("Seeded issue title") || !note.contains("Seeded issue body") {
        return Err("note content did not include seeded gh data".to_string());
    }

    let tool_rows: Vec<&str> = session
        .items
        .iter()
        .filter(|item| item.kind == ControllerChatItemKind::Tool)
        .map(|item| item.text.as_str())
        .collect();
    if !tool_rows
        .iter()
        .any(|row| row.contains("controller.create_note(issue-123.md)"))
    {
        return Err("missing create_note tool row".to_string());
    }
    if !tool_rows
        .iter()
        .any(|row| row.contains("controller.open_note(issue-123.md)"))
    {
        return Err("missing open_note tool row".to_string());
    }

    Ok(())
}

async fn run_multi_turn_once() -> Result<(), String> {
    let temp = TempDir::new().map_err(|e| e.to_string())?;
    let bin_dir = temp.path().join("bin");
    let repo_dir = temp.path().join("repo");
    let storage_dir = temp.path().join("storage");
    let project_name = "controller-chat-multi";
    fs::create_dir_all(&bin_dir).map_err(|e| e.to_string())?;
    write_gh_stub(&bin_dir)?;
    let _path_guard = prepend_path(&bin_dir);
    setup_repo(&repo_dir)?;
    let (state, _) = make_state(&storage_dir, project_name, &repo_dir)?;

    send_message_with_timeout(
        &state,
        "Use gh to fetch GitHub issue 123. Create a note named issue-123.md containing the fetched title and body in markdown, then open that note.",
    )
    .await?;

    let session = send_message_with_timeout(
        &state,
        "Keep using the currently focused note. Rewrite it so the last line is exactly 'Follow-up summary added.' and open that same note again.",
    )
    .await?;

    let note = fs::read_to_string(note_path(&storage_dir, project_name, "issue-123.md"))
        .map_err(|e| format!("Failed to read rewritten note: {}", e))?;
    if !note.contains("Follow-up summary added.") {
        return Err("second turn did not update the same note".to_string());
    }
    if session.focus.note_filename.as_deref() != Some("issue-123.md") {
        return Err("controller focus did not stay on the same note".to_string());
    }

    Ok(())
}

async fn run_error_recovery_once() -> Result<(), String> {
    let temp = TempDir::new().map_err(|e| e.to_string())?;
    let bin_dir = temp.path().join("bin");
    let repo_dir = temp.path().join("repo");
    let storage_dir = temp.path().join("storage");
    let project_name = "controller-chat-error";
    fs::create_dir_all(&bin_dir).map_err(|e| e.to_string())?;
    write_gh_stub(&bin_dir)?;
    let _path_guard = prepend_path(&bin_dir);
    setup_repo(&repo_dir)?;
    let (state, _) = make_state(&storage_dir, project_name, &repo_dir)?;

    let session = send_message_with_timeout(
        &state,
        "Use gh to fetch GitHub issue 404. If it is missing, say ISSUE 404 NOT FOUND exactly and do not create or open any note.",
    )
    .await?;

    let notes_dir = storage_dir.join("notes").join(project_name);
    if notes_dir.exists() {
        let note_count = fs::read_dir(&notes_dir)
            .map_err(|e| e.to_string())?
            .filter_map(|entry| entry.ok())
            .count();
        if note_count > 0 {
            return Err("expected no notes to be created for the missing issue".to_string());
        }
    }

    if session
        .items
        .iter()
        .any(|item| item.kind == ControllerChatItemKind::Tool)
    {
        return Err("agent emitted note tool rows for a missing issue".to_string());
    }

    let assistant = session
        .items
        .iter()
        .rev()
        .find(|item| item.kind == ControllerChatItemKind::Assistant)
        .map(|item| item.text.clone())
        .unwrap_or_default();
    if !assistant.contains("ISSUE 404 NOT FOUND") {
        return Err("assistant did not report the missing issue clearly".to_string());
    }

    Ok(())
}

fn run_diagnostics() -> Result<(), String> {
    run_command("codex", &["--version"], None)?;
    run_command("git", &["--version"], None)?;
    ("chatgpt.com", 443)
        .to_socket_addrs()
        .map_err(|e| format!("Codex backend host is not reachable from this environment: {}", e))?;
    Ok(())
}

#[tokio::test(flavor = "current_thread")]
#[ignore = "runs real codex validation scenarios"]
async fn controller_chat_validation_suite() {
    let _env_guard = ENV_LOCK.lock().unwrap();

    run_diagnostics().expect("diagnostics should pass");

    for run in 1..=RUNS_PER_SCENARIO {
        run_happy_path_once()
            .await
            .unwrap_or_else(|error| panic!("happy-path run {run} failed: {error}"));
    }

    for run in 1..=RUNS_PER_SCENARIO {
        run_multi_turn_once()
            .await
            .unwrap_or_else(|error| panic!("multi-turn run {run} failed: {error}"));
    }

    for run in 1..=RUNS_PER_SCENARIO {
        run_error_recovery_once()
            .await
            .unwrap_or_else(|error| panic!("error-recovery run {run} failed: {error}"));
    }
}
