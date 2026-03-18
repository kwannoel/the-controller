use serde::Serialize;
use std::fs;
use std::path::Path;

#[derive(Debug, Serialize, Clone)]
pub struct AgentEntry {
    /// Directory name (e.g. "ceo-agent", "default-agent")
    pub name: String,
    /// First line of agents.md stripped of # prefix (e.g. "CEO Agent")
    pub title: String,
}

/// List all agents in a project's agents/ directory.
/// Each subdirectory containing an agents.md file is an agent.
pub fn list_agents(repo_path: &Path) -> std::io::Result<Vec<AgentEntry>> {
    let agents_dir = repo_path.join("agents");
    if !agents_dir.exists() {
        return Ok(Vec::new());
    }

    let mut entries = Vec::new();
    for entry in fs::read_dir(&agents_dir)? {
        let entry = entry?;
        if !entry.file_type()?.is_dir() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with('.') {
            continue;
        }
        let agents_md = entry.path().join("agents.md");
        if agents_md.exists() {
            let content = fs::read_to_string(&agents_md).unwrap_or_default();
            let title = content
                .lines()
                .next()
                .unwrap_or("")
                .trim_start_matches('#')
                .trim()
                .to_string();
            entries.push(AgentEntry { name, title });
        }
    }

    entries.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(entries)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_list_agents_empty() {
        let tmp = TempDir::new().unwrap();
        let agents = list_agents(tmp.path()).unwrap();
        assert!(agents.is_empty());
    }

    #[test]
    fn test_list_agents_finds_agents() {
        let tmp = TempDir::new().unwrap();
        let repo = tmp.path();

        fs::create_dir_all(repo.join("agents/ceo-agent")).unwrap();
        fs::write(repo.join("agents/ceo-agent/agents.md"), "# CEO Agent\n").unwrap();

        fs::create_dir_all(repo.join("agents/default-agent")).unwrap();
        fs::write(repo.join("agents/default-agent/agents.md"), "# Default\n").unwrap();

        let agents = list_agents(repo).unwrap();
        assert_eq!(agents.len(), 2);
        assert_eq!(agents[0].name, "ceo-agent");
        assert_eq!(agents[0].title, "CEO Agent");
        assert_eq!(agents[1].name, "default-agent");
        assert_eq!(agents[1].title, "Default");
    }

    #[test]
    fn test_list_agents_ignores_dirs_without_agents_md() {
        let tmp = TempDir::new().unwrap();
        let repo = tmp.path();

        fs::create_dir_all(repo.join("agents/empty-dir")).unwrap();
        fs::create_dir_all(repo.join("agents/real-agent")).unwrap();
        fs::write(repo.join("agents/real-agent/agents.md"), "# Real\n").unwrap();

        let agents = list_agents(repo).unwrap();
        assert_eq!(agents.len(), 1);
        assert_eq!(agents[0].name, "real-agent");
    }

    #[test]
    fn test_list_agents_ignores_hidden_dirs() {
        let tmp = TempDir::new().unwrap();
        let repo = tmp.path();

        fs::create_dir_all(repo.join("agents/.hidden")).unwrap();
        fs::write(repo.join("agents/.hidden/agents.md"), "# Hidden\n").unwrap();

        fs::create_dir_all(repo.join("agents/visible")).unwrap();
        fs::write(repo.join("agents/visible/agents.md"), "# Visible\n").unwrap();

        let agents = list_agents(repo).unwrap();
        assert_eq!(agents.len(), 1);
        assert_eq!(agents[0].name, "visible");
    }
}
