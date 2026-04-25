use std::fs;
use std::os::unix::fs::symlink;
use std::path::{Path, PathBuf};
use std::process::Command;

const SKILL_PREFIX: &str = "the-controller-";

/// Resolve the main repo's skills directory.
///
/// Uses `CARGO_MANIFEST_DIR` (compile-time) as a starting point, then resolves
/// to the main repo via `git rev-parse --git-common-dir` (handles worktrees).
fn resolve_skills_source() -> Option<PathBuf> {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let repo_root = manifest_dir.parent()?;

    let output = Command::new("git")
        .args(["rev-parse", "--path-format=absolute", "--git-common-dir"])
        .current_dir(repo_root)
        .output()
        .ok()?;

    let main_repo = if output.status.success() {
        let git_dir = String::from_utf8(output.stdout).ok()?.trim().to_string();
        // git-common-dir returns the .git dir; parent is the repo root
        Path::new(&git_dir).parent()?.to_path_buf()
    } else {
        repo_root.to_path_buf()
    };

    let skills = main_repo.join("skills");
    if skills.is_dir() {
        Some(skills)
    } else {
        None
    }
}

/// List skill directories (those starting with `the-controller-`) in the source.
fn list_skill_dirs(skills_dir: &Path) -> std::io::Result<Vec<(String, PathBuf)>> {
    let mut skills = Vec::new();
    for entry in fs::read_dir(skills_dir)? {
        let entry = entry?;
        if !entry.path().is_dir() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().to_string();
        if !name.starts_with(SKILL_PREFIX) {
            continue;
        }
        skills.push((name, entry.path()));
    }
    Ok(skills)
}

/// Remove stale `the-controller-*` symlinks in a directory whose targets no longer exist.
fn cleanup_stale_symlinks(dir: &Path) -> std::io::Result<()> {
    if !dir.exists() {
        return Ok(());
    }
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().to_string();
        if !name.starts_with(SKILL_PREFIX) {
            continue;
        }
        let path = entry.path();
        let meta = match path.symlink_metadata() {
            Ok(m) => m,
            Err(_) => continue,
        };
        if meta.file_type().is_symlink() && !path.exists() {
            fs::remove_file(&path)?;
        }
    }
    Ok(())
}

/// Ensure a symlink exists at `link` pointing to `target`.
///
/// - If `link` is already a symlink to `target`, this is a no-op.
/// - If `link` is a symlink to something else, it's replaced.
/// - If `link` is a regular file/dir, it's skipped with a warning (don't clobber user files).
fn ensure_symlink(target: &Path, link: &Path) -> std::io::Result<()> {
    if let Ok(meta) = link.symlink_metadata() {
        if meta.file_type().is_symlink() {
            if let Ok(existing_target) = fs::read_link(link) {
                if existing_target == target {
                    return Ok(()); // Already correct
                }
            }
            fs::remove_file(link)?;
        } else {
            eprintln!(
                "Warning: {} exists as regular file/dir, skipping",
                link.display()
            );
            return Ok(());
        }
    }
    symlink(target, link)
}

/// Sync skills to `~/.claude/skills/` as directory symlinks.
///
/// Each skill becomes `~/.claude/skills/the-controller-<name>/` pointing
/// to `<repo>/skills/the-controller-<name>/`.
fn sync_claude_skills(skills_dir: &Path) -> std::io::Result<()> {
    let home = dirs::home_dir()
        .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::NotFound, "no home dir"))?;
    let claude_skills_dir = home.join(".claude").join("skills");
    fs::create_dir_all(&claude_skills_dir)?;

    cleanup_stale_symlinks(&claude_skills_dir)?;

    for (name, skill_path) in list_skill_dirs(skills_dir)? {
        let link_path = claude_skills_dir.join(&name);
        ensure_symlink(&skill_path, &link_path)?;
    }

    Ok(())
}

/// Sync skills to `~/.codex/skills/custom/` as directory symlinks.
///
/// Each skill becomes `~/.codex/skills/custom/the-controller-<name>/` pointing
/// to `<repo>/skills/the-controller-<name>/`.
fn sync_codex_skills(skills_dir: &Path) -> std::io::Result<()> {
    let home = dirs::home_dir()
        .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::NotFound, "no home dir"))?;
    let custom_dir = home.join(".codex").join("skills").join("custom");
    fs::create_dir_all(&custom_dir)?;

    cleanup_stale_symlinks(&custom_dir)?;

    for (name, skill_path) in list_skill_dirs(skills_dir)? {
        let link_path = custom_dir.join(&name);
        ensure_symlink(&skill_path, &link_path)?;
    }

    Ok(())
}

/// Sync all skills to Claude Code and Codex home directories.
///
/// Called on app startup. Idempotent — safe to call multiple times.
pub fn sync_skills() {
    let skills_dir = match resolve_skills_source() {
        Some(dir) => dir,
        None => {
            eprintln!("Warning: could not find skills directory, skipping skill injection");
            return;
        }
    };

    if let Err(e) = sync_claude_skills(&skills_dir) {
        eprintln!("Warning: failed to sync Claude skills: {e}");
    }

    if let Err(e) = sync_codex_skills(&skills_dir) {
        eprintln!("Warning: failed to sync Codex skills: {e}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn create_skill(dir: &Path, name: &str) {
        let skill_dir = dir.join(name);
        fs::create_dir_all(&skill_dir).unwrap();
        fs::write(
            skill_dir.join("SKILL.md"),
            format!("---\nname: {}\n---\nTest skill", name),
        )
        .unwrap();
    }

    #[test]
    fn test_list_skill_dirs_filters_by_prefix() {
        let tmp = TempDir::new().unwrap();
        create_skill(tmp.path(), "the-controller-foo");
        create_skill(tmp.path(), "the-controller-bar");
        create_skill(tmp.path(), "unrelated-skill");

        let skills = list_skill_dirs(tmp.path()).unwrap();
        assert_eq!(skills.len(), 2);
        let names: Vec<&str> = skills.iter().map(|(n, _)| n.as_str()).collect();
        assert!(names.contains(&"the-controller-foo"));
        assert!(names.contains(&"the-controller-bar"));
    }

    #[test]
    fn test_ensure_symlink_creates_new() {
        let tmp = TempDir::new().unwrap();
        let target = tmp.path().join("target.md");
        fs::write(&target, "content").unwrap();
        let link = tmp.path().join("link.md");

        ensure_symlink(&target, &link).unwrap();
        assert!(link.symlink_metadata().unwrap().file_type().is_symlink());
        assert_eq!(fs::read_link(&link).unwrap(), target);
    }

    #[test]
    fn test_ensure_symlink_idempotent() {
        let tmp = TempDir::new().unwrap();
        let target = tmp.path().join("target.md");
        fs::write(&target, "content").unwrap();
        let link = tmp.path().join("link.md");

        ensure_symlink(&target, &link).unwrap();
        ensure_symlink(&target, &link).unwrap(); // Should not error
        assert_eq!(fs::read_link(&link).unwrap(), target);
    }

    #[test]
    fn test_ensure_symlink_replaces_wrong_target() {
        let tmp = TempDir::new().unwrap();
        let old_target = tmp.path().join("old.md");
        let new_target = tmp.path().join("new.md");
        fs::write(&old_target, "old").unwrap();
        fs::write(&new_target, "new").unwrap();
        let link = tmp.path().join("link.md");

        ensure_symlink(&old_target, &link).unwrap();
        ensure_symlink(&new_target, &link).unwrap();
        assert_eq!(fs::read_link(&link).unwrap(), new_target);
    }

    #[test]
    fn test_ensure_symlink_skips_regular_file() {
        let tmp = TempDir::new().unwrap();
        let target = tmp.path().join("target.md");
        fs::write(&target, "content").unwrap();
        let link = tmp.path().join("link.md");
        fs::write(&link, "user file").unwrap(); // Regular file, not symlink

        ensure_symlink(&target, &link).unwrap();
        // Should NOT have been replaced
        assert!(!link.symlink_metadata().unwrap().file_type().is_symlink());
        assert_eq!(fs::read_to_string(&link).unwrap(), "user file");
    }

    #[test]
    fn test_cleanup_stale_symlinks() {
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path().join("commands");
        fs::create_dir_all(&dir).unwrap();

        // Create a dangling symlink
        let link = dir.join("the-controller-gone.md");
        symlink("/nonexistent/path", &link).unwrap();

        // Create a valid symlink
        let target = tmp.path().join("real.md");
        fs::write(&target, "content").unwrap();
        let valid_link = dir.join("the-controller-valid.md");
        symlink(&target, &valid_link).unwrap();

        // Create a non-prefixed symlink (should be ignored)
        let other = dir.join("other.md");
        symlink("/nonexistent", &other).unwrap();

        cleanup_stale_symlinks(&dir).unwrap();

        assert!(!link.exists() && link.symlink_metadata().is_err()); // Dangling removed
        assert!(valid_link.symlink_metadata().is_ok()); // Valid kept
        assert!(other.symlink_metadata().is_ok()); // Non-prefixed kept
    }

    #[test]
    fn test_sync_claude_skills_end_to_end() {
        let tmp = TempDir::new().unwrap();
        let skills_dir = tmp.path().join("skills");
        fs::create_dir_all(&skills_dir).unwrap();
        create_skill(&skills_dir, "the-controller-test-skill");

        // We can't easily test the actual sync without mocking home_dir,
        // but we can test the building blocks work together.
        let skills = list_skill_dirs(&skills_dir).unwrap();
        assert_eq!(skills.len(), 1);
        assert_eq!(skills[0].0, "the-controller-test-skill");
        assert!(skills[0].1.join("SKILL.md").exists());
    }
}
