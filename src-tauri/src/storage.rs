use serde::{Deserialize, Serialize};
use std::fs;
use std::io;
use std::ops::{Deref, DerefMut};
use std::path::PathBuf;
use uuid::Uuid;

use crate::models::{MaintainerRunLog, Project};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CorruptProjectEntry {
    pub project_dir: PathBuf,
    pub project_file: PathBuf,
    pub error: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ProjectInventory {
    pub projects: Vec<Project>,
    pub corrupt_entries: Vec<CorruptProjectEntry>,
}

impl ProjectInventory {
    pub fn filter_projects<F>(self, mut predicate: F) -> Self
    where
        F: FnMut(&Project) -> bool,
    {
        Self {
            projects: self
                .projects
                .into_iter()
                .filter(|project| predicate(project))
                .collect(),
            corrupt_entries: self.corrupt_entries,
        }
    }

    pub fn warn_if_corrupt(&self, context: &str) {
        for entry in &self.corrupt_entries {
            eprintln!(
                "Warning: {}: failed to parse {}: {}",
                context,
                entry.project_file.display(),
                entry.error
            );
        }
    }
}

impl Deref for ProjectInventory {
    type Target = Vec<Project>;

    fn deref(&self) -> &Self::Target {
        &self.projects
    }
}

impl DerefMut for ProjectInventory {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.projects
    }
}

impl IntoIterator for ProjectInventory {
    type Item = Project;
    type IntoIter = std::vec::IntoIter<Project>;

    fn into_iter(self) -> Self::IntoIter {
        self.projects.into_iter()
    }
}

impl<'a> IntoIterator for &'a ProjectInventory {
    type Item = &'a Project;
    type IntoIter = std::slice::Iter<'a, Project>;

    fn into_iter(self) -> Self::IntoIter {
        self.projects.iter()
    }
}

impl<'a> IntoIterator for &'a mut ProjectInventory {
    type Item = &'a mut Project;
    type IntoIter = std::slice::IterMut<'a, Project>;

    fn into_iter(self) -> Self::IntoIter {
        self.projects.iter_mut()
    }
}

pub struct Storage {
    base_dir: PathBuf,
}

impl Storage {
    /// Create a new Storage with a custom base directory (useful for testing).
    pub fn new(base_dir: PathBuf) -> Self {
        Self { base_dir }
    }

    pub fn default_base_dir(home_dir: Option<PathBuf>) -> io::Result<PathBuf> {
        home_dir
            .map(|home| home.join(".the-controller"))
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::NotFound,
                    "could not determine home directory",
                )
            })
    }

    /// Create a Storage using the default `~/.the-controller/` directory.
    pub fn with_default_path() -> io::Result<Self> {
        Ok(Self {
            base_dir: Self::default_base_dir(dirs::home_dir())?,
        })
    }

    /// Return the base directory path.
    pub fn base_dir(&self) -> PathBuf {
        self.base_dir.clone()
    }

    /// Ensure that the required directory structure exists.
    pub fn ensure_dirs(&self) -> std::io::Result<()> {
        fs::create_dir_all(self.base_dir.join("projects"))
    }

    /// Return the path to a specific project's config directory.
    pub fn project_dir(&self, project_id: Uuid) -> PathBuf {
        self.base_dir.join("projects").join(project_id.to_string())
    }

    /// Save a project's configuration to disk as `project.json`.
    pub fn save_project(&self, project: &Project) -> std::io::Result<()> {
        let dir = self.project_dir(project.id);
        fs::create_dir_all(&dir)?;
        let json = serde_json::to_string_pretty(project)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        fs::write(dir.join("project.json"), json)
    }

    /// Load a project's configuration from disk.
    pub fn load_project(&self, project_id: Uuid) -> std::io::Result<Project> {
        let path = self.project_dir(project_id).join("project.json");
        let json = fs::read_to_string(path)?;
        serde_json::from_str(&json)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))
    }

    /// List all projects by reading every `project.json` in the projects directory.
    pub fn list_projects(&self) -> std::io::Result<ProjectInventory> {
        let projects_dir = self.base_dir.join("projects");
        if !projects_dir.exists() {
            return Ok(ProjectInventory::default());
        }

        let mut inventory = ProjectInventory::default();
        for entry in fs::read_dir(&projects_dir)? {
            let entry = entry?;
            let project_dir = entry.path();
            let project_file = project_dir.join("project.json");
            if project_file.exists() {
                let json = fs::read_to_string(&project_file)?;
                match serde_json::from_str::<Project>(&json) {
                    Ok(project) => inventory.projects.push(project),
                    Err(error) => inventory.corrupt_entries.push(CorruptProjectEntry {
                        project_dir,
                        project_file,
                        error: error.to_string(),
                    }),
                }
            }
        }
        Ok(inventory)
    }

    /// Migrate worktree directories from UUID-based to name-based paths.
    ///
    /// Renames `worktrees/{project_uuid}/` to `worktrees/{project_name}/`
    /// and updates all `worktree_path` entries in the project's sessions.
    /// No-op if the UUID directory doesn't exist (already migrated or no worktrees).
    pub fn migrate_worktree_paths(&self, project: &Project) -> std::io::Result<()> {
        let uuid_dir = self.base_dir.join("worktrees").join(project.id.to_string());
        let name_dir = self.base_dir.join("worktrees").join(&project.name);
        if uuid_dir.exists() && name_dir.exists() {
            eprintln!(
                "Warning: cannot migrate worktrees for project '{}': target dir already exists",
                project.name
            );
            return Ok(());
        }

        if uuid_dir.exists() {
            fs::rename(&uuid_dir, &name_dir)?;
        } else if !name_dir.exists() {
            return Ok(());
        }

        let uuid_prefix = uuid_dir.to_str().unwrap_or_default();
        let name_prefix = name_dir.to_str().unwrap_or_default();

        let mut updated = project.clone();
        let mut changed = false;
        for session in &mut updated.sessions {
            if let Some(ref wt_path) = session.worktree_path {
                if wt_path.starts_with(uuid_prefix) {
                    session.worktree_path = Some(wt_path.replacen(uuid_prefix, name_prefix, 1));
                    changed = true;
                }
            }
        }

        if changed {
            self.save_project(&updated)?;
        }

        Ok(())
    }

    /// Delete a project's config directory.
    pub fn delete_project_dir(&self, project_id: Uuid) -> std::io::Result<()> {
        let dir = self.project_dir(project_id);
        if dir.exists() {
            fs::remove_dir_all(dir)
        } else {
            Ok(())
        }
    }

    /// Get the content of agents.md for a project.
    ///
    /// Checks the repo root first (`project.repo_path/agents.md`), then falls
    /// back to the project config dir (`<base_dir>/projects/<id>/agents.md`).
    /// Returns an empty string if neither exists, or an error if a file exists
    /// but cannot be read.
    pub fn get_agents_md(&self, project: &Project) -> std::io::Result<String> {
        // Check repo root first
        let repo_agents = PathBuf::from(&project.repo_path).join("agents.md");
        if repo_agents.exists() {
            return fs::read_to_string(&repo_agents);
        }

        // Fall back to project config dir
        let config_agents = self.project_dir(project.id).join("agents.md");
        if config_agents.exists() {
            return fs::read_to_string(&config_agents);
        }

        Ok(String::new())
    }

    /// Save agents.md content to the project's config directory.
    pub fn save_agents_md(&self, project_id: Uuid, content: &str) -> std::io::Result<()> {
        let dir = self.project_dir(project_id);
        fs::create_dir_all(&dir)?;
        fs::write(dir.join("agents.md"), content)
    }

    /// Return the path to a project's maintainer run logs directory.
    pub fn maintainer_run_logs_dir(&self, project_id: Uuid) -> PathBuf {
        self.project_dir(project_id).join("maintainer-reports")
    }

    /// Save a maintainer run log to disk.
    pub fn save_maintainer_run_log(&self, log: &MaintainerRunLog) -> std::io::Result<()> {
        let dir = self.maintainer_run_logs_dir(log.project_id);
        fs::create_dir_all(&dir)?;
        let filename = format!("{}.json", log.id);
        let json = serde_json::to_string_pretty(log)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        fs::write(dir.join(filename), json)
    }

    /// Load the most recent maintainer run log for a project.
    pub fn latest_maintainer_run_log(
        &self,
        project_id: Uuid,
    ) -> std::io::Result<Option<MaintainerRunLog>> {
        let dir = self.maintainer_run_logs_dir(project_id);
        if !dir.exists() {
            return Ok(None);
        }
        let mut logs = self.load_run_logs_from_dir(&dir)?;
        logs.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
        Ok(logs.into_iter().next())
    }

    /// Load maintainer run log history for a project, most recent first.
    pub fn maintainer_run_log_history(
        &self,
        project_id: Uuid,
        limit: usize,
    ) -> std::io::Result<Vec<MaintainerRunLog>> {
        let dir = self.maintainer_run_logs_dir(project_id);
        if !dir.exists() {
            return Ok(vec![]);
        }
        let mut logs = self.load_run_logs_from_dir(&dir)?;
        logs.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
        logs.truncate(limit);
        Ok(logs)
    }

    /// Delete all maintainer run logs for a project.
    /// Also deletes any old-format MaintainerReport files in the same directory.
    pub fn clear_maintainer_run_logs(&self, project_id: Uuid) -> std::io::Result<()> {
        let dir = self.maintainer_run_logs_dir(project_id);
        if dir.exists() {
            fs::remove_dir_all(&dir)?;
        }
        Ok(())
    }

    fn load_run_logs_from_dir(
        &self,
        dir: &std::path::Path,
    ) -> std::io::Result<Vec<MaintainerRunLog>> {
        let mut logs = Vec::new();
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().is_some_and(|e| e == "json") {
                let json = fs::read_to_string(&path)?;
                // Skip old-format files that fail to deserialize
                if let Ok(log) = serde_json::from_str::<MaintainerRunLog>(&json) {
                    logs.push(log);
                }
            }
        }
        Ok(logs)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{IssueAction, IssueSummary, MaintainerRunLog, SessionConfig};
    use tempfile::TempDir;

    fn make_storage(tmp: &TempDir) -> Storage {
        let storage = Storage::new(tmp.path().to_path_buf());
        storage.ensure_dirs().expect("ensure_dirs");
        storage
    }

    fn make_project(name: &str, repo_path: &str) -> Project {
        Project {
            id: Uuid::new_v4(),
            name: name.to_string(),
            repo_path: repo_path.to_string(),
            created_at: "2026-02-28T00:00:00Z".to_string(),
            archived: false,
            maintainer: crate::models::MaintainerConfig::default(),
            auto_worker: crate::models::AutoWorkerConfig::default(),
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
            staged_session: None,
        }
    }

    #[test]
    fn test_save_and_load_project() {
        let tmp = TempDir::new().unwrap();
        let storage = make_storage(&tmp);
        let project = make_project("test-project", "/tmp/repo");

        storage.save_project(&project).expect("save");
        let loaded = storage.load_project(project.id).expect("load");

        assert_eq!(loaded.id, project.id);
        assert_eq!(loaded.name, "test-project");
        assert_eq!(loaded.repo_path, "/tmp/repo");
        assert_eq!(loaded.sessions.len(), 1);
    }

    #[test]
    fn test_list_projects() {
        let tmp = TempDir::new().unwrap();
        let storage = make_storage(&tmp);

        let p1 = make_project("project-1", "/tmp/repo1");
        let p2 = make_project("project-2", "/tmp/repo2");
        storage.save_project(&p1).expect("save p1");
        storage.save_project(&p2).expect("save p2");

        let inventory = storage.list_projects().expect("list");
        assert_eq!(inventory.projects.len(), 2);
        assert!(inventory.corrupt_entries.is_empty());
    }

    #[test]
    fn test_list_projects_reports_corrupt_project_json() {
        let tmp = TempDir::new().unwrap();
        let storage = make_storage(&tmp);

        let valid = make_project("valid-project", "/tmp/repo-valid");
        storage.save_project(&valid).expect("save valid");

        let corrupt_dir = storage.project_dir(Uuid::new_v4());
        fs::create_dir_all(&corrupt_dir).expect("create corrupt dir");
        let corrupt_file = corrupt_dir.join("project.json");
        fs::write(&corrupt_file, "{ invalid json").expect("write corrupt project.json");

        let inventory = storage.list_projects().expect("list");
        assert_eq!(inventory.projects.len(), 1);
        assert_eq!(inventory.projects[0].name, "valid-project");
        assert_eq!(inventory.corrupt_entries.len(), 1);
        assert_eq!(inventory.corrupt_entries[0].project_file, corrupt_file);
    }

    #[test]
    fn test_list_empty() {
        let tmp = TempDir::new().unwrap();
        let storage = make_storage(&tmp);

        let inventory = storage.list_projects().expect("list");
        assert!(inventory.projects.is_empty());
        assert!(inventory.corrupt_entries.is_empty());
    }

    #[test]
    fn test_agents_md_fallback() {
        let tmp = TempDir::new().unwrap();
        let storage = make_storage(&tmp);
        let project = make_project("test-project", "/tmp/nonexistent-repo");

        // No agents.md anywhere -> empty string
        assert_eq!(storage.get_agents_md(&project).unwrap(), "");

        // Save local agents.md -> returns it
        storage
            .save_agents_md(project.id, "local agents content")
            .expect("save agents.md");
        assert_eq!(
            storage.get_agents_md(&project).unwrap(),
            "local agents content"
        );
    }

    #[test]
    fn test_agents_md_repo_takes_priority() {
        let tmp = TempDir::new().unwrap();
        let repo_dir = TempDir::new().unwrap();
        let storage = make_storage(&tmp);

        let project = make_project("test-project", repo_dir.path().to_str().unwrap());

        // Write agents.md in config dir
        storage
            .save_agents_md(project.id, "config dir content")
            .expect("save config agents.md");

        // Write agents.md in repo root
        fs::write(repo_dir.path().join("agents.md"), "repo content").expect("write repo agents.md");

        // Repo version should win
        assert_eq!(storage.get_agents_md(&project).unwrap(), "repo content");
    }

    #[test]
    fn test_delete_project() {
        let tmp = TempDir::new().unwrap();
        let storage = make_storage(&tmp);
        let project = make_project("test-project", "/tmp/repo");

        storage.save_project(&project).expect("save");
        assert!(storage.load_project(project.id).is_ok());

        storage.delete_project_dir(project.id).expect("delete");
        assert!(storage.load_project(project.id).is_err());
    }

    #[test]
    fn test_ensure_dirs_creates_projects_directory() {
        let tmp = TempDir::new().unwrap();
        let storage = Storage::new(tmp.path().to_path_buf());
        storage.ensure_dirs().expect("ensure_dirs");
        assert!(tmp.path().join("projects").is_dir());
    }

    #[test]
    fn test_project_dir_format() {
        let tmp = TempDir::new().unwrap();
        let storage = Storage::new(tmp.path().to_path_buf());
        let id = Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap();
        let dir = storage.project_dir(id);
        assert!(dir.ends_with("projects/550e8400-e29b-41d4-a716-446655440000"));
    }

    #[test]
    fn test_delete_nonexistent_project_is_ok() {
        let tmp = TempDir::new().unwrap();
        let storage = make_storage(&tmp);
        let id = Uuid::new_v4();
        // Deleting a project that was never saved should succeed (idempotent)
        assert!(storage.delete_project_dir(id).is_ok());
    }

    #[test]
    fn test_base_dir_returns_correct_path() {
        let tmp = TempDir::new().unwrap();
        let storage = Storage::new(tmp.path().to_path_buf());
        assert_eq!(storage.base_dir(), tmp.path().to_path_buf());
    }

    #[test]
    fn test_default_base_dir_returns_error_when_home_is_unavailable() {
        let error = Storage::default_base_dir(None)
            .expect_err("expected missing home directory to return an error");

        assert_eq!(error.kind(), std::io::ErrorKind::NotFound);
    }

    #[test]
    fn test_save_and_get_agents_md_roundtrip() {
        let tmp = TempDir::new().unwrap();
        let storage = make_storage(&tmp);
        let project = make_project("test-project", "/tmp/nonexistent-repo");
        let id = project.id;

        storage
            .save_agents_md(id, "# My Agents\nContent here")
            .unwrap();

        // get_agents_md falls back to config dir when repo doesn't exist
        let content = storage.get_agents_md(&project).unwrap();
        assert_eq!(content, "# My Agents\nContent here");
    }

    #[test]
    fn test_load_nonexistent_project_returns_error() {
        let tmp = TempDir::new().unwrap();
        let storage = make_storage(&tmp);
        let id = Uuid::new_v4();
        assert!(storage.load_project(id).is_err());
    }

    #[test]
    fn test_save_project_overwrites_existing() {
        let tmp = TempDir::new().unwrap();
        let storage = make_storage(&tmp);
        let mut project = make_project("test-project", "/tmp/repo");

        storage.save_project(&project).expect("first save");
        project.name = "updated-name".to_string();
        storage.save_project(&project).expect("second save");

        let loaded = storage.load_project(project.id).expect("load");
        assert_eq!(loaded.name, "updated-name");
    }

    fn make_run_log(project_id: Uuid, timestamp: &str) -> MaintainerRunLog {
        MaintainerRunLog {
            id: Uuid::new_v4(),
            project_id,
            timestamp: timestamp.to_string(),
            issues_filed: vec![IssueSummary {
                issue_number: 1,
                title: "Test issue".to_string(),
                url: "https://github.com/owner/repo/issues/1".to_string(),
                labels: vec!["filed-by-maintainer".to_string()],
                action: IssueAction::Filed,
            }],
            issues_updated: vec![],
            issues_unchanged: 0,
            issues_skipped: 0,
            summary: "Filed 1 issue".to_string(),
        }
    }

    #[test]
    fn test_save_and_load_run_log() {
        let tmp = TempDir::new().unwrap();
        let storage = make_storage(&tmp);
        let project_id = Uuid::new_v4();
        let log = make_run_log(project_id, "2026-03-09T00:00:00Z");
        let log_id = log.id;

        storage.save_maintainer_run_log(&log).expect("save");
        let latest = storage.latest_maintainer_run_log(project_id).expect("load");
        assert!(latest.is_some());
        let latest = latest.unwrap();
        assert_eq!(latest.id, log_id);
        assert_eq!(latest.summary, "Filed 1 issue");
    }

    #[test]
    fn test_latest_run_log_returns_none_when_empty() {
        let tmp = TempDir::new().unwrap();
        let storage = make_storage(&tmp);
        let project_id = Uuid::new_v4();
        let latest = storage.latest_maintainer_run_log(project_id).expect("load");
        assert!(latest.is_none());
    }

    #[test]
    fn test_run_log_history() {
        let tmp = TempDir::new().unwrap();
        let storage = make_storage(&tmp);
        let project_id = Uuid::new_v4();

        let r1 = make_run_log(project_id, "2026-03-09T01:00:00Z");
        let r2 = make_run_log(project_id, "2026-03-09T02:00:00Z");
        let r3 = make_run_log(project_id, "2026-03-09T03:00:00Z");

        storage.save_maintainer_run_log(&r1).expect("save r1");
        storage.save_maintainer_run_log(&r2).expect("save r2");
        storage.save_maintainer_run_log(&r3).expect("save r3");

        let history = storage
            .maintainer_run_log_history(project_id, 10)
            .expect("history");
        assert_eq!(history.len(), 3);
        assert_eq!(history[0].timestamp, "2026-03-09T03:00:00Z");
        assert_eq!(history[2].timestamp, "2026-03-09T01:00:00Z");
    }

    #[test]
    fn test_clear_maintainer_run_logs() {
        let tmp = TempDir::new().unwrap();
        let storage = make_storage(&tmp);
        let project_id = Uuid::new_v4();

        let r1 = make_run_log(project_id, "2026-03-09T01:00:00Z");
        storage.save_maintainer_run_log(&r1).expect("save");
        assert!(storage
            .latest_maintainer_run_log(project_id)
            .unwrap()
            .is_some());

        storage
            .clear_maintainer_run_logs(project_id)
            .expect("clear");
        assert!(storage
            .latest_maintainer_run_log(project_id)
            .unwrap()
            .is_none());
    }

    #[test]
    fn test_clear_maintainer_run_logs_idempotent() {
        let tmp = TempDir::new().unwrap();
        let storage = make_storage(&tmp);
        let project_id = Uuid::new_v4();
        assert!(storage.clear_maintainer_run_logs(project_id).is_ok());
    }

    #[test]
    fn test_old_format_reports_are_silently_skipped() {
        let tmp = TempDir::new().unwrap();
        let storage = make_storage(&tmp);
        let project_id = Uuid::new_v4();

        // Write an old-format MaintainerReport file
        let dir = storage.maintainer_run_logs_dir(project_id);
        std::fs::create_dir_all(&dir).unwrap();
        let old_report = r#"{
            "id": "550e8400-e29b-41d4-a716-446655440000",
            "project_id": "550e8400-e29b-41d4-a716-446655440001",
            "timestamp": "2026-03-07T00:00:00Z",
            "status": "passing",
            "findings": [],
            "summary": "old format"
        }"#;
        std::fs::write(dir.join("old-report.json"), old_report).unwrap();

        // Write a new-format run log
        let log = make_run_log(project_id, "2026-03-09T00:00:00Z");
        storage.save_maintainer_run_log(&log).expect("save");

        // History should only contain the new-format log
        let history = storage.maintainer_run_log_history(project_id, 10).unwrap();
        assert_eq!(history.len(), 1);
        assert_eq!(history[0].summary, "Filed 1 issue");
    }
}
