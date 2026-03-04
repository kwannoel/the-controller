use std::fs;
use std::path::PathBuf;
use uuid::Uuid;

use crate::models::Project;

pub struct Storage {
    base_dir: PathBuf,
}

impl Storage {
    /// Create a new Storage with a custom base directory (useful for testing).
    pub fn new(base_dir: PathBuf) -> Self {
        Self { base_dir }
    }

    /// Create a Storage using the default `~/.the-controller/` directory.
    pub fn with_default_path() -> Self {
        let home = dirs::home_dir().expect("could not determine home directory");
        Self {
            base_dir: home.join(".the-controller"),
        }
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
    pub fn list_projects(&self) -> std::io::Result<Vec<Project>> {
        let projects_dir = self.base_dir.join("projects");
        if !projects_dir.exists() {
            return Ok(Vec::new());
        }

        let mut projects = Vec::new();
        for entry in fs::read_dir(&projects_dir)? {
            let entry = entry?;
            let project_file = entry.path().join("project.json");
            if project_file.exists() {
                let json = fs::read_to_string(&project_file)?;
                if let Ok(project) = serde_json::from_str::<Project>(&json) {
                    projects.push(project);
                }
            }
        }
        Ok(projects)
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::SessionConfig;
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
            sessions: vec![SessionConfig {
                id: Uuid::new_v4(),
                label: "main".to_string(),
                worktree_path: None,
                worktree_branch: None,
                archived: false,
                kind: "claude".to_string(),
            }],
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

        let projects = storage.list_projects().expect("list");
        assert_eq!(projects.len(), 2);
    }

    #[test]
    fn test_list_empty() {
        let tmp = TempDir::new().unwrap();
        let storage = make_storage(&tmp);

        let projects = storage.list_projects().expect("list");
        assert!(projects.is_empty());
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
        assert_eq!(storage.get_agents_md(&project).unwrap(), "local agents content");
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
}
