use chrono::{DateTime, Utc};
use serde::Serialize;
use std::fs;
use std::path::PathBuf;
use uuid::Uuid;

#[derive(Debug, Serialize, Clone)]
pub struct NoteEntry {
    pub filename: String,
    pub modified_at: DateTime<Utc>,
}

/// Validates that a filename does not escape the notes directory.
fn validate_filename(filename: &str) -> std::io::Result<()> {
    if filename.contains('/') || filename.contains('\\') || filename.contains("..") || filename.is_empty() {
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
pub fn list_notes(
    base: &std::path::Path,
    project_name: &str,
) -> std::io::Result<Vec<NoteEntry>> {
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

/// Check whether a note file exists after validating its filename.
pub fn note_exists(
    base: &std::path::Path,
    project_name: &str,
    filename: &str,
) -> std::io::Result<bool> {
    validate_filename(filename)?;
    let path = notes_dir_with_base(base, project_name).join(filename);
    Ok(path.exists())
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

/// Strip trailing `-<8 hex chars>` UUID suffixes from a stem.
/// e.g. "blog-adac4038-70d71ba1" → "blog"
fn strip_uuid_suffixes(stem: &str) -> &str {
    let mut s = stem;
    loop {
        if s.len() < 9 {
            break;
        }
        let (head, tail) = s.split_at(s.len() - 9);
        if tail.starts_with('-') && tail[1..].bytes().all(|b| b.is_ascii_hexdigit()) && !head.is_empty() {
            s = head;
        } else {
            break;
        }
    }
    s
}

/// Duplicate a note file. Creates a copy named `{base_stem}-{uuid}.md`,
/// stripping any existing UUID suffixes first to prevent stacking.
/// Returns the filename of the new copy.
pub fn duplicate_note(
    base: &std::path::Path,
    project_name: &str,
    filename: &str,
) -> std::io::Result<String> {
    validate_filename(filename)?;
    let dir = notes_dir_with_base(base, project_name);
    let src_path = dir.join(filename);

    if !src_path.exists() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("note '{}' not found", filename),
        ));
    }

    let content = fs::read_to_string(&src_path)?;
    let stem = filename.strip_suffix(".md").unwrap_or(filename);
    let base_stem = strip_uuid_suffixes(stem);
    let short_id = &Uuid::new_v4().to_string()[..8];
    let copy_filename = format!("{}-{}.md", base_stem, short_id);

    fs::write(dir.join(&copy_filename), content)?;
    Ok(copy_filename)
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
        assert_eq!(result.unwrap_err().kind(), std::io::ErrorKind::AlreadyExists);
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
        assert_eq!(result.unwrap_err().kind(), std::io::ErrorKind::AlreadyExists);
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
    fn test_strip_uuid_suffixes() {
        assert_eq!(strip_uuid_suffixes("blog"), "blog");
        assert_eq!(strip_uuid_suffixes("blog-adac4038"), "blog");
        assert_eq!(strip_uuid_suffixes("blog-adac4038-70d71ba1"), "blog");
        // Don't strip non-hex suffixes
        assert_eq!(strip_uuid_suffixes("my-notes"), "my-notes");
        assert_eq!(strip_uuid_suffixes("my-project-xyz"), "my-project-xyz");
        // Don't strip to empty
        assert_eq!(strip_uuid_suffixes("abcd1234"), "abcd1234");
    }

    #[test]
    fn test_duplicate_note() {
        let tmp = TempDir::new().unwrap();
        let base = tmp.path();
        create_note(base, "proj", "original").unwrap();
        write_note(base, "proj", "original.md", "hello world").unwrap();

        let copy = duplicate_note(base, "proj", "original.md").unwrap();
        assert!(copy.starts_with("original-"), "expected 'original-<uuid>.md', got '{}'", copy);
        assert!(copy.ends_with(".md"));
        assert_ne!(copy, "original.md");

        let content = read_note(base, "proj", &copy).unwrap();
        assert_eq!(content, "hello world");
    }

    #[test]
    fn test_duplicate_strips_existing_uuid_suffix() {
        let tmp = TempDir::new().unwrap();
        let base = tmp.path();
        create_note(base, "proj", "blog").unwrap();

        // First duplicate: blog → blog-<uuid1>
        let copy1 = duplicate_note(base, "proj", "blog.md").unwrap();
        assert!(copy1.starts_with("blog-"));

        // Second duplicate of the copy: should still be blog-<uuid2>, not blog-<uuid1>-<uuid2>
        let copy2 = duplicate_note(base, "proj", &copy1).unwrap();
        assert!(copy2.starts_with("blog-"), "expected 'blog-<uuid>.md', got '{}'", copy2);
        // Should only have one UUID segment (blog + dash + 8 hex + .md = stem of length 13)
        let stem2 = copy2.strip_suffix(".md").unwrap();
        assert_eq!(stem2.len(), 13, "expected 'blog-<8hex>', got '{}'", stem2);
    }

    #[test]
    fn test_duplicate_note_unique_names() {
        let tmp = TempDir::new().unwrap();
        let base = tmp.path();
        create_note(base, "proj", "doc").unwrap();

        let copy1 = duplicate_note(base, "proj", "doc.md").unwrap();
        let copy2 = duplicate_note(base, "proj", "doc.md").unwrap();
        assert_ne!(copy1, copy2);

        // Both should exist with the same content
        let c1 = read_note(base, "proj", &copy1).unwrap();
        let c2 = read_note(base, "proj", &copy2).unwrap();
        assert_eq!(c1, c2);
    }

    #[test]
    fn test_duplicate_nonexistent_note_fails() {
        let tmp = TempDir::new().unwrap();
        let result = duplicate_note(tmp.path(), "proj", "nope.md");
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().kind(), std::io::ErrorKind::NotFound);
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
        assert_eq!(read_result.unwrap_err().kind(), std::io::ErrorKind::InvalidInput);

        let write_result = write_note(tmp.path(), "proj", malicious, "pwned");
        assert!(write_result.is_err());
        assert_eq!(write_result.unwrap_err().kind(), std::io::ErrorKind::InvalidInput);

        let delete_result = delete_note(tmp.path(), "proj", malicious);
        assert!(delete_result.is_err());
        assert_eq!(delete_result.unwrap_err().kind(), std::io::ErrorKind::InvalidInput);
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
