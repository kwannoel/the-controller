use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Project {
    pub id: Uuid,
    pub name: String,
    pub repo_path: String,
    pub created_at: String,
    pub archived: bool,
    pub sessions: Vec<SessionConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionConfig {
    pub id: Uuid,
    pub label: String,
    pub worktree_path: Option<String>,
    pub worktree_branch: Option<String>,
    #[serde(default)]
    pub archived: bool,
    #[serde(default = "default_kind")]
    pub kind: String,
    #[serde(default)]
    pub github_issue: Option<GithubIssue>,
    #[serde(default)]
    pub initial_prompt: Option<String>,
    /// Accumulated commit summaries — persisted so they survive merge/rebase.
    #[serde(default)]
    pub done_commits: Vec<CommitInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CommitInfo {
    pub hash: String,
    pub message: String,
}

fn default_kind() -> String {
    "claude".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum SessionStatus {
    Running,
    Idle,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionInfo {
    pub id: Uuid,
    pub label: String,
    pub project_id: Uuid,
    pub worktree_path: Option<String>,
    pub worktree_branch: Option<String>,
    pub status: SessionStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GithubIssue {
    pub number: u64,
    pub title: String,
    pub url: String,
    pub labels: Vec<GithubLabel>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GithubLabel {
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum MergeResponse {
    #[serde(rename = "pr_created")]
    PrCreated { url: String },
    #[serde(rename = "rebase_conflicts")]
    RebaseConflicts,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_project_serialization_roundtrip() {
        let project = Project {
            id: Uuid::new_v4(),
            name: "test-project".to_string(),
            repo_path: "/tmp/test-repo".to_string(),
            created_at: "2026-02-28T00:00:00Z".to_string(),
            archived: false,
            sessions: vec![SessionConfig {
                id: Uuid::new_v4(),
                label: "main".to_string(),
                worktree_path: None,
                worktree_branch: None,
                archived: false,
                kind: "claude".to_string(),
                github_issue: None,
                initial_prompt: None,
                done_commits: vec![],
            }],
        };

        let json = serde_json::to_string(&project).expect("serialize");
        let deserialized: Project = serde_json::from_str(&json).expect("deserialize");

        assert_eq!(deserialized.id, project.id);
        assert_eq!(deserialized.name, project.name);
        assert_eq!(deserialized.repo_path, project.repo_path);
        assert_eq!(deserialized.created_at, project.created_at);
        assert_eq!(deserialized.archived, project.archived);
        assert_eq!(deserialized.sessions.len(), 1);
        assert_eq!(deserialized.sessions[0].id, project.sessions[0].id);
        assert_eq!(deserialized.sessions[0].label, project.sessions[0].label);
        assert!(deserialized.sessions[0].worktree_path.is_none());
        assert!(deserialized.sessions[0].worktree_branch.is_none());
        assert!(!deserialized.sessions[0].archived);
        assert_eq!(deserialized.sessions[0].kind, "claude");
    }

    #[test]
    fn test_session_config_kind_defaults_to_claude() {
        let json = r#"{"id":"550e8400-e29b-41d4-a716-446655440000","label":"session-1","worktree_path":null,"worktree_branch":null,"archived":false}"#;
        let session: SessionConfig = serde_json::from_str(json).expect("deserialize");
        assert_eq!(session.kind, "claude");
    }

    #[test]
    fn test_session_config_kind_codex() {
        let json = r#"{"id":"550e8400-e29b-41d4-a716-446655440000","label":"session-1","worktree_path":null,"worktree_branch":null,"archived":false,"kind":"codex"}"#;
        let session: SessionConfig = serde_json::from_str(json).expect("deserialize");
        assert_eq!(session.kind, "codex");
    }

    #[test]
    fn test_project_with_worktree_session() {
        let session_id = Uuid::new_v4();
        let project = Project {
            id: Uuid::new_v4(),
            name: "worktree-project".to_string(),
            repo_path: "/tmp/worktree-repo".to_string(),
            created_at: "2026-02-28T12:00:00Z".to_string(),
            archived: false,
            sessions: vec![SessionConfig {
                id: session_id,
                label: "feature-branch".to_string(),
                worktree_path: Some("/tmp/worktree-repo/.worktrees/feature".to_string()),
                worktree_branch: Some("feature/new-thing".to_string()),
                archived: false,
                kind: "claude".to_string(),
                github_issue: None,
                initial_prompt: None,
                done_commits: vec![],
            }],
        };

        let json = serde_json::to_string(&project).expect("serialize");
        let deserialized: Project = serde_json::from_str(&json).expect("deserialize");

        assert_eq!(deserialized.sessions.len(), 1);
        let session = &deserialized.sessions[0];
        assert_eq!(session.id, session_id);
        assert_eq!(session.label, "feature-branch");
        assert_eq!(
            session.worktree_path.as_deref(),
            Some("/tmp/worktree-repo/.worktrees/feature")
        );
        assert_eq!(
            session.worktree_branch.as_deref(),
            Some("feature/new-thing")
        );
    }

    #[test]
    fn test_session_config_github_issue_roundtrip() {
        let session = SessionConfig {
            id: Uuid::new_v4(),
            label: "session-1".to_string(),
            worktree_path: None,
            worktree_branch: None,
            archived: false,
            kind: "claude".to_string(),
            github_issue: Some(GithubIssue {
                number: 22,
                title: "Assign GitHub issue to a session".to_string(),
                url: "https://github.com/kwannoel/the-controller/issues/22".to_string(),
                labels: vec![],
            }),
            initial_prompt: None,
            done_commits: vec![],
        };
        let json = serde_json::to_string(&session).expect("serialize");
        let deserialized: SessionConfig = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(deserialized.github_issue.as_ref().unwrap().number, 22);
        assert_eq!(
            deserialized.github_issue.as_ref().unwrap().title,
            "Assign GitHub issue to a session"
        );
    }

    #[test]
    fn test_session_config_github_issue_defaults_to_none() {
        let json = r#"{"id":"550e8400-e29b-41d4-a716-446655440000","label":"session-1","worktree_path":null,"worktree_branch":null,"archived":false}"#;
        let session: SessionConfig = serde_json::from_str(json).expect("deserialize");
        assert!(session.github_issue.is_none());
    }

    #[test]
    fn test_session_config_initial_prompt_defaults_to_none() {
        let json = r#"{"id":"550e8400-e29b-41d4-a716-446655440000","label":"session-1","worktree_path":null,"worktree_branch":null,"archived":false}"#;
        let session: SessionConfig = serde_json::from_str(json).expect("deserialize");
        assert!(session.initial_prompt.is_none());
    }

    #[test]
    fn test_session_config_initial_prompt_roundtrip() {
        let session = SessionConfig {
            id: Uuid::new_v4(),
            label: "session-1".to_string(),
            worktree_path: None,
            worktree_branch: None,
            archived: false,
            kind: "claude".to_string(),
            github_issue: None,
            initial_prompt: Some("fix the bug".to_string()),
            done_commits: vec![],
        };
        let json = serde_json::to_string(&session).expect("serialize");
        let deserialized: SessionConfig = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(deserialized.initial_prompt.as_deref(), Some("fix the bug"));
    }

    #[test]
    fn test_merge_response_pr_created_serialization() {
        let response = MergeResponse::PrCreated {
            url: "https://github.com/owner/repo/pull/1".to_string(),
        };
        let json = serde_json::to_string(&response).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["type"], "pr_created");
        assert_eq!(parsed["url"], "https://github.com/owner/repo/pull/1");
    }

    #[test]
    fn test_merge_response_rebase_conflicts_serialization() {
        let response = MergeResponse::RebaseConflicts;
        let json = serde_json::to_string(&response).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["type"], "rebase_conflicts");
    }

    #[test]
    fn test_session_status_serialization() {
        let running = SessionStatus::Running;
        let idle = SessionStatus::Idle;
        let running_json = serde_json::to_string(&running).unwrap();
        let idle_json = serde_json::to_string(&idle).unwrap();
        assert_eq!(running_json, "\"Running\"");
        assert_eq!(idle_json, "\"Idle\"");
    }

    #[test]
    fn test_github_issue_with_labels() {
        let issue = GithubIssue {
            number: 42,
            title: "Bug fix".to_string(),
            url: "https://github.com/owner/repo/issues/42".to_string(),
            labels: vec![
                GithubLabel { name: "bug".to_string() },
                GithubLabel { name: "priority".to_string() },
            ],
        };
        let json = serde_json::to_string(&issue).unwrap();
        let deserialized: GithubIssue = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.labels.len(), 2);
        assert_eq!(deserialized.labels[0].name, "bug");
        assert_eq!(deserialized.labels[1].name, "priority");
    }

    #[test]
    fn test_session_info_serialization_roundtrip() {
        let info = SessionInfo {
            id: Uuid::new_v4(),
            label: "session-1".to_string(),
            project_id: Uuid::new_v4(),
            worktree_path: Some("/tmp/wt".to_string()),
            worktree_branch: Some("session-1".to_string()),
            status: SessionStatus::Running,
        };
        let json = serde_json::to_string(&info).unwrap();
        let deserialized: SessionInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.id, info.id);
        assert_eq!(deserialized.status, SessionStatus::Running);
    }
}
