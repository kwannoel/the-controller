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
    #[serde(default)]
    pub maintainer: MaintainerConfig,
    #[serde(default)]
    pub auto_worker: AutoWorkerConfig,
    #[serde(default)]
    pub prompts: Vec<SavedPrompt>,
    /// Sessions staged as separate Controller instances.
    #[serde(
        default,
        alias = "staged_session",
        deserialize_with = "deserialize_staged_sessions"
    )]
    pub staged_sessions: Vec<StagedSession>,
}

/// Tracks staging state: which session is running as a separate
/// Controller instance, and the PID/port of that process.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StagedSession {
    pub session_id: Uuid,
    pub pid: u32,
    pub port: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MaintainerConfig {
    pub enabled: bool,
    pub interval_minutes: u64,
    #[serde(default)]
    pub github_repo: Option<String>,
}

impl Default for MaintainerConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            interval_minutes: 60,
            github_repo: None,
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AutoWorkerConfig {
    pub enabled: bool,
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
    #[serde(default)]
    pub auto_worker_session: bool,
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
    #[serde(default)]
    pub body: Option<String>,
    pub labels: Vec<GithubLabel>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GithubLabel {
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GithubAssignee {
    pub login: String,
}

/// An open issue that has at least one assignee.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssignedIssue {
    pub number: u64,
    pub title: String,
    pub url: String,
    pub assignees: Vec<GithubAssignee>,
    #[serde(rename = "updatedAt")]
    pub updated_at: String,
    pub labels: Vec<GithubLabel>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SavedPrompt {
    pub id: Uuid,
    pub name: String,
    pub text: String,
    pub created_at: String,
    pub source_session_label: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum MergeResponse {
    #[serde(rename = "pr_created")]
    PrCreated { url: String },
    #[serde(rename = "rebase_conflicts")]
    RebaseConflicts,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum IssueAction {
    Filed,
    Updated,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IssueSummary {
    pub issue_number: u32,
    pub title: String,
    pub url: String,
    pub labels: Vec<String>,
    pub action: IssueAction,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MaintainerRunLog {
    pub id: Uuid,
    pub project_id: Uuid,
    pub timestamp: String,
    pub issues_filed: Vec<IssueSummary>,
    pub issues_updated: Vec<IssueSummary>,
    pub issues_unchanged: u32,
    #[serde(default)]
    pub issues_skipped: u32,
    pub summary: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MaintainerIssue {
    pub number: u32,
    pub title: String,
    pub state: String,
    pub url: String,
    pub labels: Vec<GithubLabel>,
    #[serde(rename = "createdAt")]
    pub created_at: String,
    #[serde(rename = "closedAt")]
    pub closed_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MaintainerIssueDetail {
    pub number: u32,
    pub title: String,
    pub state: String,
    pub body: String,
    pub url: String,
    pub labels: Vec<GithubLabel>,
    #[serde(rename = "createdAt")]
    pub created_at: String,
    #[serde(rename = "closedAt")]
    pub closed_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AutoWorkerQueueIssue {
    pub number: u64,
    pub title: String,
    pub url: String,
    #[serde(default)]
    pub body: Option<String>,
    pub labels: Vec<String>,
    pub is_active: bool,
}

fn deserialize_staged_sessions<'de, D>(deserializer: D) -> Result<Vec<StagedSession>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::Deserialize;
    use serde_json::Value;

    let value = Option::<Value>::deserialize(deserializer)?;
    match value {
        None => Ok(vec![]),
        Some(Value::Array(arr)) => {
            let sessions: Vec<StagedSession> = arr
                .into_iter()
                .map(|v| serde_json::from_value(v).map_err(serde::de::Error::custom))
                .collect::<Result<_, _>>()?;
            Ok(sessions)
        }
        Some(obj @ Value::Object(_)) => {
            let session: StagedSession =
                serde_json::from_value(obj).map_err(serde::de::Error::custom)?;
            Ok(vec![session])
        }
        Some(_) => Err(serde::de::Error::custom("unexpected staged_sessions value")),
    }
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
            maintainer: MaintainerConfig::default(),
            auto_worker: AutoWorkerConfig::default(),
            prompts: vec![],
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
                auto_worker_session: false,
            }],
            staged_sessions: vec![],
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
            maintainer: MaintainerConfig::default(),
            auto_worker: AutoWorkerConfig::default(),
            prompts: vec![],
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
                auto_worker_session: false,
            }],
            staged_sessions: vec![],
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
                body: None,
                labels: vec![],
            }),
            initial_prompt: None,
            done_commits: vec![],
            auto_worker_session: false,
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
            auto_worker_session: false,
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
            body: None,
            labels: vec![
                GithubLabel {
                    name: "bug".to_string(),
                },
                GithubLabel {
                    name: "priority".to_string(),
                },
            ],
        };
        let json = serde_json::to_string(&issue).unwrap();
        let deserialized: GithubIssue = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.labels.len(), 2);
        assert_eq!(deserialized.labels[0].name, "bug");
        assert_eq!(deserialized.labels[1].name, "priority");
    }

    #[test]
    fn test_maintainer_config_defaults_when_absent() {
        let json = r#"{
            "id": "550e8400-e29b-41d4-a716-446655440000",
            "name": "test-project",
            "repo_path": "/tmp/test-repo",
            "created_at": "2026-02-28T00:00:00Z",
            "archived": false,
            "sessions": []
        }"#;
        let project: Project = serde_json::from_str(json).expect("deserialize");
        assert!(!project.maintainer.enabled);
        assert_eq!(project.maintainer.interval_minutes, 60);
    }

    #[test]
    fn test_maintainer_config_roundtrip() {
        let project = Project {
            id: Uuid::new_v4(),
            name: "test-project".to_string(),
            repo_path: "/tmp/test-repo".to_string(),
            created_at: "2026-02-28T00:00:00Z".to_string(),
            archived: false,
            maintainer: MaintainerConfig {
                enabled: true,
                interval_minutes: 30,
                github_repo: None,
            },
            auto_worker: AutoWorkerConfig::default(),
            prompts: vec![],
            sessions: vec![],
            staged_sessions: vec![],
        };
        let json = serde_json::to_string(&project).expect("serialize");
        let deserialized: Project = serde_json::from_str(&json).expect("deserialize");
        assert!(deserialized.maintainer.enabled);
        assert_eq!(deserialized.maintainer.interval_minutes, 30);
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

    #[test]
    fn test_maintainer_config_github_repo_defaults_to_none() {
        let json = r#"{
            "id": "550e8400-e29b-41d4-a716-446655440000",
            "name": "test-project",
            "repo_path": "/tmp/test-repo",
            "created_at": "2026-02-28T00:00:00Z",
            "archived": false,
            "sessions": []
        }"#;
        let project: Project = serde_json::from_str(json).expect("deserialize");
        assert!(project.maintainer.github_repo.is_none());
    }

    #[test]
    fn test_maintainer_run_log_serialization() {
        let run_log = MaintainerRunLog {
            id: Uuid::new_v4(),
            project_id: Uuid::new_v4(),
            timestamp: "2026-03-09T00:00:00Z".to_string(),
            issues_filed: vec![IssueSummary {
                issue_number: 42,
                title: "Fix the bug".to_string(),
                url: "https://github.com/owner/repo/issues/42".to_string(),
                labels: vec!["bug".to_string(), "priority".to_string()],
                action: IssueAction::Filed,
            }],
            issues_updated: vec![],
            issues_unchanged: 3,
            issues_skipped: 0,
            summary: "Filed 1 issue, 3 unchanged".to_string(),
        };
        let json = serde_json::to_string(&run_log).expect("serialize");
        let deserialized: MaintainerRunLog = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(deserialized.issues_filed.len(), 1);
        assert_eq!(deserialized.issues_filed[0].issue_number, 42);
        assert_eq!(deserialized.issues_filed[0].title, "Fix the bug");
        assert_eq!(deserialized.issues_filed[0].labels.len(), 2);
        assert_eq!(deserialized.issues_filed[0].action, IssueAction::Filed);
        assert_eq!(deserialized.issues_updated.len(), 0);
        assert_eq!(deserialized.issues_unchanged, 3);
        assert_eq!(deserialized.summary, "Filed 1 issue, 3 unchanged");
    }

    #[test]
    fn test_issue_action_serialization() {
        let filed = IssueAction::Filed;
        let updated = IssueAction::Updated;
        let filed_json = serde_json::to_string(&filed).unwrap();
        let updated_json = serde_json::to_string(&updated).unwrap();
        assert_eq!(filed_json, "\"filed\"");
        assert_eq!(updated_json, "\"updated\"");
    }

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
            prompts: vec![],
            sessions: vec![],
            staged_sessions: vec![],
        };
        let json = serde_json::to_string(&project).expect("serialize");
        let deserialized: Project = serde_json::from_str(&json).expect("deserialize");
        assert!(deserialized.auto_worker.enabled);
    }

    #[test]
    fn test_assigned_issue_serialization_roundtrip() {
        let issue = AssignedIssue {
            number: 42,
            title: "Fix the bug".to_string(),
            url: "https://github.com/owner/repo/issues/42".to_string(),
            assignees: vec![GithubAssignee {
                login: "alice".to_string(),
            }],
            updated_at: "2026-03-01T12:00:00Z".to_string(),
            labels: vec![GithubLabel {
                name: "bug".to_string(),
            }],
        };
        let json = serde_json::to_string(&issue).expect("serialize");
        let deserialized: AssignedIssue = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(deserialized.number, 42);
        assert_eq!(deserialized.title, "Fix the bug");
        assert_eq!(deserialized.assignees.len(), 1);
        assert_eq!(deserialized.assignees[0].login, "alice");
        assert_eq!(deserialized.updated_at, "2026-03-01T12:00:00Z");
        assert_eq!(deserialized.labels.len(), 1);
    }

    #[test]
    fn test_assigned_issue_deserialization_from_gh_json() {
        let json = r#"{
            "number": 10,
            "title": "Stale issue",
            "url": "https://github.com/owner/repo/issues/10",
            "assignees": [
                {"login": "bob"},
                {"login": "carol"}
            ],
            "updatedAt": "2026-01-15T08:30:00Z",
            "labels": []
        }"#;
        let issue: AssignedIssue = serde_json::from_str(json).expect("deserialize");
        assert_eq!(issue.number, 10);
        assert_eq!(issue.assignees.len(), 2);
        assert_eq!(issue.assignees[0].login, "bob");
        assert_eq!(issue.assignees[1].login, "carol");
        assert_eq!(issue.updated_at, "2026-01-15T08:30:00Z");
    }

    #[test]
    fn test_assigned_issue_empty_assignees() {
        let issue = AssignedIssue {
            number: 1,
            title: "Unassigned".to_string(),
            url: "https://github.com/owner/repo/issues/1".to_string(),
            assignees: vec![],
            updated_at: "2026-03-09T00:00:00Z".to_string(),
            labels: vec![],
        };
        let json = serde_json::to_string(&issue).expect("serialize");
        let deserialized: AssignedIssue = serde_json::from_str(&json).expect("deserialize");
        assert!(deserialized.assignees.is_empty());
    }

    #[test]
    fn test_staged_session_defaults_to_none() {
        let json = r#"{
            "id": "550e8400-e29b-41d4-a716-446655440000",
            "name": "test-project",
            "repo_path": "/tmp/test-repo",
            "created_at": "2026-02-28T00:00:00Z",
            "archived": false,
            "sessions": []
        }"#;
        let project: Project = serde_json::from_str(json).expect("deserialize");
        assert!(project.staged_sessions.is_empty());
    }

    #[test]
    fn test_staged_session_roundtrip() {
        let project = Project {
            id: Uuid::new_v4(),
            name: "test".to_string(),
            repo_path: "/tmp".to_string(),
            created_at: "2026-03-09T00:00:00Z".to_string(),
            archived: false,
            maintainer: MaintainerConfig::default(),
            auto_worker: AutoWorkerConfig::default(),
            prompts: vec![],
            sessions: vec![],
            staged_sessions: vec![StagedSession {
                session_id: Uuid::new_v4(),
                pid: 99999,
                port: 2420,
            }],
        };
        let json = serde_json::to_string(&project).expect("serialize");
        let deserialized: Project = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(deserialized.staged_sessions.len(), 1);
        assert_eq!(deserialized.staged_sessions[0].pid, 99999);
        assert_eq!(deserialized.staged_sessions[0].port, 2420);
    }

    #[test]
    fn test_run_log_includes_issues_skipped_field() {
        let run_log = MaintainerRunLog {
            id: Uuid::new_v4(),
            project_id: Uuid::new_v4(),
            timestamp: "2026-03-09T00:00:00Z".to_string(),
            issues_filed: vec![],
            issues_updated: vec![],
            issues_unchanged: 2,
            issues_skipped: 5,
            summary: "Skipped 5 closed issues".to_string(),
        };
        let json = serde_json::to_string(&run_log).expect("serialize");
        let deserialized: MaintainerRunLog = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(deserialized.issues_skipped, 5);
        assert_eq!(deserialized.issues_unchanged, 2);

        // Also verify that issues_skipped defaults to 0 when absent
        let json_without_skipped = r#"{
            "id": "550e8400-e29b-41d4-a716-446655440000",
            "project_id": "550e8400-e29b-41d4-a716-446655440001",
            "timestamp": "2026-03-09T00:00:00Z",
            "issues_filed": [],
            "issues_updated": [],
            "issues_unchanged": 3,
            "summary": "test"
        }"#;
        let from_old: MaintainerRunLog =
            serde_json::from_str(json_without_skipped).expect("deserialize without skipped");
        assert_eq!(from_old.issues_skipped, 0);
    }

    #[test]
    fn test_maintainer_issue_serialization_roundtrip() {
        let issue = MaintainerIssue {
            number: 42,
            title: "Test issue".to_string(),
            state: "OPEN".to_string(),
            url: "https://github.com/owner/repo/issues/42".to_string(),
            labels: vec![GithubLabel {
                name: "filed-by-maintainer".to_string(),
            }],
            created_at: "2026-03-09T00:00:00Z".to_string(),
            closed_at: None,
        };
        let json = serde_json::to_string(&issue).expect("serialize");
        let deserialized: MaintainerIssue = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(deserialized.number, 42);
        assert_eq!(deserialized.state, "OPEN");
        assert!(deserialized.closed_at.is_none());
    }

    #[test]
    fn test_maintainer_issue_detail_serialization_roundtrip() {
        let detail = MaintainerIssueDetail {
            number: 10,
            title: "Detail issue".to_string(),
            state: "CLOSED".to_string(),
            body: "Issue body here".to_string(),
            url: "https://github.com/owner/repo/issues/10".to_string(),
            labels: vec![],
            created_at: "2026-03-01T00:00:00Z".to_string(),
            closed_at: Some("2026-03-05T00:00:00Z".to_string()),
        };
        let json = serde_json::to_string(&detail).expect("serialize");
        let deserialized: MaintainerIssueDetail = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(deserialized.number, 10);
        assert_eq!(deserialized.state, "CLOSED");
        assert_eq!(deserialized.body, "Issue body here");
        assert_eq!(
            deserialized.closed_at.as_deref(),
            Some("2026-03-05T00:00:00Z")
        );
    }

    #[test]
    fn test_staged_sessions_multiple_roundtrip() {
        let project = Project {
            id: Uuid::new_v4(),
            name: "test".to_string(),
            repo_path: "/tmp".to_string(),
            created_at: "2026-03-16T00:00:00Z".to_string(),
            archived: false,
            maintainer: MaintainerConfig::default(),
            auto_worker: AutoWorkerConfig::default(),
            prompts: vec![],
            sessions: vec![],
            staged_sessions: vec![
                StagedSession {
                    session_id: Uuid::new_v4(),
                    pid: 1001,
                    port: 2420,
                },
                StagedSession {
                    session_id: Uuid::new_v4(),
                    pid: 1002,
                    port: 2421,
                },
            ],
        };
        let json = serde_json::to_string(&project).expect("serialize");
        let deserialized: Project = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(deserialized.staged_sessions.len(), 2);
        assert_eq!(deserialized.staged_sessions[0].port, 2420);
        assert_eq!(deserialized.staged_sessions[1].port, 2421);
    }

    #[test]
    fn test_staged_session_migration_from_old_format() {
        let json = r#"{
            "id": "550e8400-e29b-41d4-a716-446655440000",
            "name": "test",
            "repo_path": "/tmp",
            "created_at": "2026-03-16T00:00:00Z",
            "archived": false,
            "sessions": [],
            "staged_session": {
                "session_id": "550e8400-e29b-41d4-a716-446655440001",
                "pid": 12345,
                "port": 2420
            }
        }"#;
        let project: Project = serde_json::from_str(json).expect("deserialize old format");
        assert_eq!(project.staged_sessions.len(), 1);
        assert_eq!(project.staged_sessions[0].pid, 12345);
        assert_eq!(project.staged_sessions[0].port, 2420);
    }

    #[test]
    fn test_staged_session_null_migrates_to_empty() {
        let json = r#"{
            "id": "550e8400-e29b-41d4-a716-446655440000",
            "name": "test",
            "repo_path": "/tmp",
            "created_at": "2026-03-16T00:00:00Z",
            "archived": false,
            "sessions": [],
            "staged_session": null
        }"#;
        let project: Project = serde_json::from_str(json).expect("deserialize null staged_session");
        assert!(project.staged_sessions.is_empty());
    }
}
