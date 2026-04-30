use serde_json::json;
use the_controller_lib::{
    commands::chat_workspace_snapshot,
    models::{AutoWorkerConfig, ChatWorkspaceSnapshot, MaintainerConfig, Project, SessionConfig},
};
use uuid::Uuid;

#[test]
fn workspace_snapshot_contains_controller_owned_fields_only() {
    let snapshot = ChatWorkspaceSnapshot {
        project_id: "p1".into(),
        workspace_id: "w1".into(),
        path: "/tmp/worktree".into(),
        label: "feature-worktree".into(),
        branch: Some("codex/feature".into()),
        focused: true,
    };

    assert_eq!(snapshot.path, "/tmp/worktree");
}

#[test]
fn workspace_snapshot_serializes_for_daemon_workspace_link_api() {
    let snapshot = ChatWorkspaceSnapshot {
        project_id: "p1".into(),
        workspace_id: "w1".into(),
        path: "/tmp/worktree".into(),
        label: "feature-worktree".into(),
        branch: Some("codex/feature".into()),
        focused: true,
    };

    assert_eq!(
        serde_json::to_value(snapshot).unwrap(),
        json!({
            "project_id": "p1",
            "workspace_id": "w1",
            "path": "/tmp/worktree",
            "label": "feature-worktree",
            "branch": "codex/feature",
            "focused": true
        })
    );
}

#[test]
fn workspace_snapshot_helper_uses_session_worktree_fields() {
    let project_id = Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap();
    let session_id = Uuid::parse_str("550e8400-e29b-41d4-a716-446655440001").unwrap();
    let project = project(project_id, "/repo/main");
    let session = session(
        session_id,
        "session-1-ab12cd",
        Some("/tmp/worktree"),
        Some("session-1-ab12cd"),
    );

    let snapshot = chat_workspace_snapshot(&project, &session, true);

    assert_eq!(snapshot.project_id, project_id.to_string());
    assert_eq!(snapshot.workspace_id, session_id.to_string());
    assert_eq!(snapshot.path, "/tmp/worktree");
    assert_eq!(snapshot.label, "session-1-ab12cd");
    assert_eq!(snapshot.branch.as_deref(), Some("session-1-ab12cd"));
    assert!(snapshot.focused);
}

#[test]
fn workspace_snapshot_helper_falls_back_to_repo_path_for_unborn_repo_sessions() {
    let project = project(Uuid::new_v4(), "/repo/unborn");
    let session = session(Uuid::new_v4(), "session-1-ab12cd", None, None);

    let snapshot = chat_workspace_snapshot(&project, &session, false);

    assert_eq!(snapshot.path, "/repo/unborn");
    assert_eq!(snapshot.branch, None);
    assert!(!snapshot.focused);
}

#[test]
fn controller_route_posts_workspace_snapshots_through_daemon_gateway() {
    let main_source = include_str!("../src/main.rs");

    assert!(main_source.contains("/api/create_chat_workspace"));
    assert!(main_source.contains("post(create_chat_workspace)"));
    assert!(main_source.contains("/api/daemon/chats/{chat_id}/workspace-links"));
    assert!(main_source.contains("proxy_http_gateway("));
    assert!(main_source
        .contains("cleanup_chat_workspace_session(state, project_id, session_id.clone()).await"));
    assert!(main_source
        .contains("commands::close_session_impl(&state.app, project_id, session_id, true)"));
    assert!(!main_source.contains("127.0.0.1:4867"));
}

fn project(id: Uuid, repo_path: &str) -> Project {
    Project {
        id,
        name: "test-project".into(),
        repo_path: repo_path.into(),
        created_at: "2026-04-30T00:00:00Z".into(),
        archived: false,
        sessions: vec![],
        maintainer: MaintainerConfig::default(),
        auto_worker: AutoWorkerConfig::default(),
        prompts: vec![],
        staged_sessions: vec![],
    }
}

fn session(
    id: Uuid,
    label: &str,
    worktree_path: Option<&str>,
    worktree_branch: Option<&str>,
) -> SessionConfig {
    SessionConfig {
        id,
        label: label.into(),
        worktree_path: worktree_path.map(str::to_string),
        worktree_branch: worktree_branch.map(str::to_string),
        archived: false,
        kind: "claude".into(),
        github_issue: None,
        initial_prompt: None,
        done_commits: vec![],
        auto_worker_session: false,
    }
}
