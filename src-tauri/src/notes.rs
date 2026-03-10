use chrono::{DateTime, Utc};
use serde::Serialize;
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Serialize, Clone)]
pub struct NoteEntry {
    pub filename: String,
    pub modified_at: DateTime<Utc>,
}

/// Validates that a filename does not escape the notes directory.
fn validate_filename(filename: &str) -> std::io::Result<()> {
    if filename.contains('/')
        || filename.contains('\\')
        || filename.contains("..")
        || filename.is_empty()
    {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!("invalid note filename: {}", filename),
        ));
    }
    Ok(())
}

/// Returns the notes directory for a project under the default base path.
/// `~/.the-controller/notes/{project_name}/`
pub fn notes_dir(project_name: &str) -> PathBuf {
    let home = dirs::home_dir().expect("could not determine home directory");
    home.join(".the-controller")
        .join("notes")
        .join(project_name)
}

/// Returns the notes directory for a project under a custom base path (for testing).
pub fn notes_dir_with_base(base: &std::path::Path, project_name: &str) -> PathBuf {
    base.join("notes").join(project_name)
}

/// List all `.md` files in the project's notes directory, sorted by modified time (newest first).
pub fn list_notes(base: &std::path::Path, project_name: &str) -> std::io::Result<Vec<NoteEntry>> {
    let dir = notes_dir_with_base(base, project_name);
    if !dir.exists() {
        return Ok(Vec::new());
    }

    let mut entries = Vec::new();
    for entry in fs::read_dir(&dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().map_or(false, |e| e == "md") {
            let metadata = fs::metadata(&path)?;
            let modified = metadata.modified()?;
            let modified_at: DateTime<Utc> = modified.into();
            let Some(name) = path.file_name() else {
                continue;
            };
            let filename = name.to_string_lossy().to_string();
            entries.push(NoteEntry {
                filename,
                modified_at,
            });
        }
    }

    entries.sort_by(|a, b| b.modified_at.cmp(&a.modified_at));
    Ok(entries)
}

/// Read the content of a note file.
pub fn read_note(
    base: &std::path::Path,
    project_name: &str,
    filename: &str,
) -> std::io::Result<String> {
    validate_filename(filename)?;
    let path = notes_dir_with_base(base, project_name).join(filename);
    fs::read_to_string(path)
}

/// Write (create or overwrite) a note file. Creates the directory if needed.
pub fn write_note(
    base: &std::path::Path,
    project_name: &str,
    filename: &str,
    content: &str,
) -> std::io::Result<()> {
    validate_filename(filename)?;
    let dir = notes_dir_with_base(base, project_name);
    fs::create_dir_all(&dir)?;
    fs::write(dir.join(filename), content)
}

/// Create a new note with the given title. Auto-appends `.md` if not present.
/// The file content is initialized to `# {title}\n`.
/// Returns an error if a note with that filename already exists.
pub fn create_note(
    base: &std::path::Path,
    project_name: &str,
    title: &str,
) -> std::io::Result<String> {
    let filename = if title.ends_with(".md") {
        title.to_string()
    } else {
        format!("{}.md", title)
    };
    validate_filename(&filename)?;

    let dir = notes_dir_with_base(base, project_name);
    fs::create_dir_all(&dir)?;

    let path = dir.join(&filename);
    if path.exists() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::AlreadyExists,
            format!("note '{}' already exists", filename),
        ));
    }

    let display_title = if title.ends_with(".md") {
        &title[..title.len() - 3]
    } else {
        title
    };
    fs::write(&path, format!("# {}\n", display_title))?;
    Ok(filename)
}

/// Rename a note file. Auto-appends `.md` to `new_name` if not present.
/// Returns an error if the target filename already exists.
pub fn rename_note(
    base: &std::path::Path,
    project_name: &str,
    old_name: &str,
    new_name: &str,
) -> std::io::Result<String> {
    validate_filename(old_name)?;
    let new_filename = if new_name.ends_with(".md") {
        new_name.to_string()
    } else {
        format!("{}.md", new_name)
    };
    validate_filename(&new_filename)?;

    let dir = notes_dir_with_base(base, project_name);
    let old_path = dir.join(old_name);
    let new_path = dir.join(&new_filename);

    if !old_path.exists() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("note '{}' not found", old_name),
        ));
    }

    if new_path.exists() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::AlreadyExists,
            format!("note '{}' already exists", new_filename),
        ));
    }

    fs::rename(old_path, new_path)?;
    Ok(new_filename)
}

/// Delete a note file. Returns Ok(()) even if the file doesn't exist (idempotent).
pub fn delete_note(
    base: &std::path::Path,
    project_name: &str,
    filename: &str,
) -> std::io::Result<()> {
    validate_filename(filename)?;
    let path = notes_dir_with_base(base, project_name).join(filename);
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_list_notes_empty() {
        let tmp = TempDir::new().unwrap();
        let notes = list_notes(tmp.path(), "my-project").unwrap();
        assert!(notes.is_empty());
    }

    #[test]
    fn test_create_and_list_notes() {
        let tmp = TempDir::new().unwrap();
        let base = tmp.path();

        create_note(base, "proj", "first").unwrap();
        // Small delay to ensure different modified times
        std::thread::sleep(std::time::Duration::from_millis(50));
        create_note(base, "proj", "second").unwrap();

        let notes = list_notes(base, "proj").unwrap();
        assert_eq!(notes.len(), 2);
        // Newest first
        assert_eq!(notes[0].filename, "second.md");
        assert_eq!(notes[1].filename, "first.md");
    }

    #[test]
    fn test_create_note_adds_md_extension() {
        let tmp = TempDir::new().unwrap();
        let filename = create_note(tmp.path(), "proj", "my-note").unwrap();
        assert_eq!(filename, "my-note.md");

        let notes = list_notes(tmp.path(), "proj").unwrap();
        assert_eq!(notes.len(), 1);
        assert_eq!(notes[0].filename, "my-note.md");
    }

    #[test]
    fn test_create_note_preserves_md_extension() {
        let tmp = TempDir::new().unwrap();
        let filename = create_note(tmp.path(), "proj", "my-note.md").unwrap();
        assert_eq!(filename, "my-note.md");

        let notes = list_notes(tmp.path(), "proj").unwrap();
        assert_eq!(notes.len(), 1);
        assert_eq!(notes[0].filename, "my-note.md");
    }

    #[test]
    fn test_create_duplicate_note_fails() {
        let tmp = TempDir::new().unwrap();
        create_note(tmp.path(), "proj", "dup").unwrap();
        let result = create_note(tmp.path(), "proj", "dup");
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().kind(),
            std::io::ErrorKind::AlreadyExists
        );
    }

    #[test]
    fn test_read_note() {
        let tmp = TempDir::new().unwrap();
        create_note(tmp.path(), "proj", "hello").unwrap();
        let content = read_note(tmp.path(), "proj", "hello.md").unwrap();
        assert_eq!(content, "# hello\n");
    }

    #[test]
    fn test_write_and_read_note() {
        let tmp = TempDir::new().unwrap();
        write_note(tmp.path(), "proj", "test.md", "custom content").unwrap();
        let content = read_note(tmp.path(), "proj", "test.md").unwrap();
        assert_eq!(content, "custom content");
    }

    #[test]
    fn test_rename_note() {
        let tmp = TempDir::new().unwrap();
        create_note(tmp.path(), "proj", "old-name").unwrap();
        let new_filename = rename_note(tmp.path(), "proj", "old-name.md", "new-name").unwrap();
        assert_eq!(new_filename, "new-name.md");

        // Old name should not exist
        assert!(read_note(tmp.path(), "proj", "old-name.md").is_err());
        // New name should have the content
        let content = read_note(tmp.path(), "proj", "new-name.md").unwrap();
        assert_eq!(content, "# old-name\n");
    }

    #[test]
    fn test_rename_to_existing_fails() {
        let tmp = TempDir::new().unwrap();
        create_note(tmp.path(), "proj", "a").unwrap();
        create_note(tmp.path(), "proj", "b").unwrap();
        let result = rename_note(tmp.path(), "proj", "a.md", "b");
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().kind(),
            std::io::ErrorKind::AlreadyExists
        );
    }

    #[test]
    fn test_delete_note() {
        let tmp = TempDir::new().unwrap();
        create_note(tmp.path(), "proj", "to-delete").unwrap();
        assert!(read_note(tmp.path(), "proj", "to-delete.md").is_ok());

        delete_note(tmp.path(), "proj", "to-delete.md").unwrap();
        assert!(read_note(tmp.path(), "proj", "to-delete.md").is_err());
    }

    #[test]
    fn test_delete_nonexistent_note_is_ok() {
        let tmp = TempDir::new().unwrap();
        let result = delete_note(tmp.path(), "proj", "nonexistent.md");
        assert!(result.is_ok());
    }

    #[test]
    fn test_notes_are_project_scoped() {
        let tmp = TempDir::new().unwrap();
        create_note(tmp.path(), "project-a", "shared-name").unwrap();
        create_note(tmp.path(), "project-b", "shared-name").unwrap();

        let notes_a = list_notes(tmp.path(), "project-a").unwrap();
        let notes_b = list_notes(tmp.path(), "project-b").unwrap();
        assert_eq!(notes_a.len(), 1);
        assert_eq!(notes_b.len(), 1);

        // Writing to one project should not affect the other
        write_note(tmp.path(), "project-a", "shared-name.md", "content A").unwrap();
        let content_b = read_note(tmp.path(), "project-b", "shared-name.md").unwrap();
        assert_eq!(content_b, "# shared-name\n");
    }

    #[test]
    fn test_path_traversal_rejected() {
        let tmp = TempDir::new().unwrap();
        let malicious = "../../../etc/passwd";

        let read_result = read_note(tmp.path(), "proj", malicious);
        assert!(read_result.is_err());
        assert_eq!(
            read_result.unwrap_err().kind(),
            std::io::ErrorKind::InvalidInput
        );

        let write_result = write_note(tmp.path(), "proj", malicious, "pwned");
        assert!(write_result.is_err());
        assert_eq!(
            write_result.unwrap_err().kind(),
            std::io::ErrorKind::InvalidInput
        );

        let delete_result = delete_note(tmp.path(), "proj", malicious);
        assert!(delete_result.is_err());
        assert_eq!(
            delete_result.unwrap_err().kind(),
            std::io::ErrorKind::InvalidInput
        );
    }

    #[test]
    fn test_rename_nonexistent_source_fails() {
        let tmp = TempDir::new().unwrap();
        // Create the project directory so the error is about the source, not the dir
        create_note(tmp.path(), "proj", "existing").unwrap();
        let result = rename_note(tmp.path(), "proj", "no-such-note.md", "new-name");
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().kind(), std::io::ErrorKind::NotFound);
    }
}
