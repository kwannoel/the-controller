use chrono::{DateTime, Utc};
use git2::{Repository, Signature};
use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};
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

/// Validates that a folder name does not contain path separators or traversal sequences.
fn validate_folder_name(name: &str) -> std::io::Result<()> {
    if name.contains('/') || name.contains('\\') || name.contains("..") || name.is_empty() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!("invalid folder name: {}", name),
        ));
    }
    Ok(())
}

/// Returns the notes directory for a folder under the default base path.
/// `~/.the-controller/notes/{folder}/`
pub fn notes_dir(folder: &str) -> PathBuf {
    let home = dirs::home_dir().expect("could not determine home directory");
    home.join(".the-controller")
        .join("notes")
        .join(folder)
}

/// Returns the notes directory for a folder under a custom base path (for testing).
pub fn notes_dir_with_base(base: &std::path::Path, folder: &str) -> PathBuf {
    base.join("notes").join(folder)
}

/// Returns the root notes directory under a custom base path.
pub fn notes_root_with_base(base: &std::path::Path) -> PathBuf {
    base.join("notes")
}

/// List all `.md` files in the folder's notes directory, sorted by modified time (newest first).
pub fn list_notes(
    base: &std::path::Path,
    folder: &str,
) -> std::io::Result<Vec<NoteEntry>> {
    let dir = notes_dir_with_base(base, folder);
    if !dir.exists() {
        return Ok(Vec::new());
    }

    let mut entries = Vec::new();
    for entry in fs::read_dir(&dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().is_some_and(|e| e == "md") {
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
    folder: &str,
    filename: &str,
) -> std::io::Result<String> {
    validate_filename(filename)?;
    let path = notes_dir_with_base(base, folder).join(filename);
    fs::read_to_string(path)
}

/// Check whether a note file exists after validating its filename.
pub fn note_exists(
    base: &std::path::Path,
    folder: &str,
    filename: &str,
) -> std::io::Result<bool> {
    validate_filename(filename)?;
    let path = notes_dir_with_base(base, folder).join(filename);
    Ok(path.exists())
}

/// Write (create or overwrite) a note file. Creates the directory if needed.
pub fn write_note(
    base: &std::path::Path,
    folder: &str,
    filename: &str,
    content: &str,
) -> std::io::Result<()> {
    validate_filename(filename)?;
    let dir = notes_dir_with_base(base, folder);
    fs::create_dir_all(&dir)?;
    fs::write(dir.join(filename), content)
}

/// Create a new note with the given title. Auto-appends `.md` if not present.
/// The file content is initialized to `# {title}\n`.
/// Returns an error if a note with that filename already exists.
pub fn create_note(
    base: &std::path::Path,
    folder: &str,
    title: &str,
) -> std::io::Result<String> {
    let filename = if title.ends_with(".md") {
        title.to_string()
    } else {
        format!("{}.md", title)
    };
    validate_filename(&filename)?;

    let dir = notes_dir_with_base(base, folder);
    fs::create_dir_all(&dir)?;

    let path = dir.join(&filename);
    if path.exists() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::AlreadyExists,
            format!("note '{}' already exists", filename),
        ));
    }

    let display_title = title.strip_suffix(".md").unwrap_or(title);
    fs::write(&path, format!("# {}\n", display_title))?;
    Ok(filename)
}

/// Rename a note file. Auto-appends `.md` to `new_name` if not present.
/// Returns an error if the target filename already exists.
pub fn rename_note(
    base: &std::path::Path,
    folder: &str,
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

    let dir = notes_dir_with_base(base, folder);
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
    folder: &str,
    filename: &str,
) -> std::io::Result<String> {
    validate_filename(filename)?;
    let dir = notes_dir_with_base(base, folder);
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
    folder: &str,
    filename: &str,
) -> std::io::Result<()> {
    validate_filename(filename)?;
    let path = notes_dir_with_base(base, folder).join(filename);
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e),
    }
}

/// List all folder names (subdirectories) under the notes root, sorted alphabetically.
pub fn list_folders(base: &std::path::Path) -> std::io::Result<Vec<String>> {
    let root = notes_root_with_base(base);
    if !root.exists() {
        return Ok(Vec::new());
    }
    let mut folders = Vec::new();
    for entry in fs::read_dir(&root)? {
        let entry = entry?;
        if entry.file_type()?.is_dir() {
            if let Some(name) = entry.file_name().to_str() {
                folders.push(name.to_string());
            }
        }
    }
    folders.sort();
    Ok(folders)
}

/// Create an empty folder. Returns error if it already exists.
pub fn create_folder(base: &std::path::Path, name: &str) -> std::io::Result<()> {
    validate_folder_name(name)?;
    let dir = notes_dir_with_base(base, name);
    if dir.exists() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::AlreadyExists,
            format!("folder '{}' already exists", name),
        ));
    }
    fs::create_dir_all(&dir)
}

/// Rename a folder. Returns error if target already exists.
pub fn rename_folder(base: &std::path::Path, old_name: &str, new_name: &str) -> std::io::Result<()> {
    validate_folder_name(old_name)?;
    validate_folder_name(new_name)?;
    let old_dir = notes_dir_with_base(base, old_name);
    let new_dir = notes_dir_with_base(base, new_name);
    if !old_dir.exists() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("folder '{}' not found", old_name),
        ));
    }
    if new_dir.exists() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::AlreadyExists,
            format!("folder '{}' already exists", new_name),
        ));
    }
    fs::rename(old_dir, new_dir)
}

/// Delete a folder. If `force` is false, fails when the folder is non-empty.
/// Returns Ok(()) if the folder doesn't exist (idempotent).
pub fn delete_folder(base: &std::path::Path, name: &str, force: bool) -> std::io::Result<()> {
    validate_folder_name(name)?;
    let dir = notes_dir_with_base(base, name);
    if !dir.exists() {
        return Ok(());
    }
    if force {
        fs::remove_dir_all(&dir)
    } else {
        fs::remove_dir(&dir)
    }
}

// ── Image assets ────────────────────────────────────────────────────

const ALLOWED_IMAGE_EXTENSIONS: &[&str] = &["png", "jpg", "jpeg", "gif", "webp"];

/// Validate that a relative asset path (e.g. "assets/foo.png") is safe.
fn validate_asset_path(relative_path: &str) -> std::io::Result<()> {
    if relative_path.starts_with('/') || relative_path.contains("..") || relative_path.contains('\\') || relative_path.is_empty() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!("invalid asset path: {}", relative_path),
        ));
    }
    Ok(())
}

/// Save image bytes to the folder's assets directory.
/// Returns the relative path (e.g. "assets/a1b2c3d4.png").
pub fn save_note_image(
    base: &std::path::Path,
    folder: &str,
    image_bytes: &[u8],
    extension: &str,
) -> std::io::Result<String> {
    let ext_lower = extension.to_lowercase();
    if !ALLOWED_IMAGE_EXTENSIONS.contains(&ext_lower.as_str()) {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!("unsupported image extension: {}", extension),
        ));
    }

    let assets_dir = notes_dir_with_base(base, folder).join("assets");
    fs::create_dir_all(&assets_dir)?;

    let short_id = &Uuid::new_v4().to_string()[..8];
    let filename = format!("{}.{}", short_id, ext_lower);
    fs::write(assets_dir.join(&filename), image_bytes)?;

    Ok(format!("assets/{}", filename))
}

/// Resolve a relative asset path to an absolute filesystem path.
/// Validates the path does not escape the notes directory.
pub fn resolve_note_asset_path(
    base: &std::path::Path,
    folder: &str,
    relative_path: &str,
) -> std::io::Result<PathBuf> {
    validate_asset_path(relative_path)?;
    let full_path = notes_dir_with_base(base, folder).join(relative_path);
    Ok(full_path)
}

// ── Git version control ─────────────────────────────────────────────

/// Returns the notes root directory: `{base}/notes/`
fn notes_root(base: &Path) -> PathBuf {
    base.join("notes")
}

/// Open or initialize the notes git repo at `{base}/notes/`.
fn open_or_init_repo(base: &Path) -> Result<Repository, git2::Error> {
    let root = notes_root(base);
    fs::create_dir_all(&root).map_err(|e| {
        git2::Error::from_str(&format!("failed to create notes dir: {}", e))
    })?;
    match Repository::open(&root) {
        Ok(repo) => Ok(repo),
        Err(_) => {
            let repo = Repository::init(&root)?;
            let sig = Signature::now("the-controller", "noreply@the-controller")?;
            let tree_id = repo.index()?.write_tree()?;
            {
                let tree = repo.find_tree(tree_id)?;
                repo.commit(Some("HEAD"), &sig, &sig, "init notes", &tree, &[])?;
            }
            Ok(repo)
        }
    }
}

/// Stage all changes and commit with the given message.
/// Returns Ok(true) if a commit was created, Ok(false) if there was nothing to commit.
pub fn commit_notes(base: &Path, message: &str) -> Result<bool, git2::Error> {
    let repo = open_or_init_repo(base)?;
    let mut index = repo.index()?;

    index.add_all(["*"].iter(), git2::IndexAddOption::DEFAULT, None)?;
    index.update_all(["*"].iter(), None)?;
    index.write()?;

    let head_commit = repo.head()?.peel_to_commit()?;
    let head_tree = head_commit.tree()?;
    let new_tree_id = index.write_tree()?;
    let new_tree = repo.find_tree(new_tree_id)?;

    let diff = repo.diff_tree_to_tree(Some(&head_tree), Some(&new_tree), None)?;
    if diff.deltas().count() == 0 {
        return Ok(false);
    }

    let sig = Signature::now("the-controller", "noreply@the-controller")?;
    repo.commit(
        Some("HEAD"),
        &sig,
        &sig,
        message,
        &new_tree,
        &[&head_commit],
    )?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_list_notes_empty() {
        let tmp = TempDir::new().unwrap();
        let notes = list_notes(tmp.path(), "my-folder").unwrap();
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
        assert_eq!(strip_uuid_suffixes("my-folder-xyz"), "my-folder-xyz");
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
    fn test_notes_are_folder_scoped() {
        let tmp = TempDir::new().unwrap();
        create_note(tmp.path(), "folder-a", "shared-name").unwrap();
        create_note(tmp.path(), "folder-b", "shared-name").unwrap();

        let notes_a = list_notes(tmp.path(), "folder-a").unwrap();
        let notes_b = list_notes(tmp.path(), "folder-b").unwrap();
        assert_eq!(notes_a.len(), 1);
        assert_eq!(notes_b.len(), 1);

        // Writing to one folder should not affect the other
        write_note(tmp.path(), "folder-a", "shared-name.md", "content A").unwrap();
        let content_b = read_note(tmp.path(), "folder-b", "shared-name.md").unwrap();
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
        // Create the folder directory so the error is about the source, not the dir
        create_note(tmp.path(), "proj", "existing").unwrap();
        let result = rename_note(tmp.path(), "proj", "no-such-note.md", "new-name");
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().kind(), std::io::ErrorKind::NotFound);
    }

    #[test]
    fn test_list_folders() {
        let tmp = TempDir::new().unwrap();
        let base = tmp.path();
        let folders = list_folders(base).unwrap();
        assert!(folders.is_empty());
        create_note(base, "work", "task1").unwrap();
        create_note(base, "personal", "diary").unwrap();
        let mut folders = list_folders(base).unwrap();
        folders.sort();
        assert_eq!(folders, vec!["personal", "work"]);
    }

    #[test]
    fn test_create_folder() {
        let tmp = TempDir::new().unwrap();
        create_folder(tmp.path(), "my-folder").unwrap();
        let folders = list_folders(tmp.path()).unwrap();
        assert_eq!(folders, vec!["my-folder"]);
    }

    #[test]
    fn test_create_folder_already_exists() {
        let tmp = TempDir::new().unwrap();
        create_folder(tmp.path(), "dup").unwrap();
        let result = create_folder(tmp.path(), "dup");
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().kind(), std::io::ErrorKind::AlreadyExists);
    }

    #[test]
    fn test_rename_folder() {
        let tmp = TempDir::new().unwrap();
        create_note(tmp.path(), "old-name", "note1").unwrap();
        rename_folder(tmp.path(), "old-name", "new-name").unwrap();
        let folders = list_folders(tmp.path()).unwrap();
        assert_eq!(folders, vec!["new-name"]);
        let content = read_note(tmp.path(), "new-name", "note1.md").unwrap();
        assert_eq!(content, "# note1\n");
    }

    #[test]
    fn test_rename_folder_target_exists() {
        let tmp = TempDir::new().unwrap();
        create_folder(tmp.path(), "a").unwrap();
        create_folder(tmp.path(), "b").unwrap();
        let result = rename_folder(tmp.path(), "a", "b");
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().kind(), std::io::ErrorKind::AlreadyExists);
    }

    #[test]
    fn test_delete_folder_empty() {
        let tmp = TempDir::new().unwrap();
        create_folder(tmp.path(), "empty").unwrap();
        delete_folder(tmp.path(), "empty", false).unwrap();
        let folders = list_folders(tmp.path()).unwrap();
        assert!(folders.is_empty());
    }

    #[test]
    fn test_delete_folder_nonempty_without_force() {
        let tmp = TempDir::new().unwrap();
        create_note(tmp.path(), "has-notes", "note1").unwrap();
        let result = delete_folder(tmp.path(), "has-notes", false);
        assert!(result.is_err());
    }

    #[test]
    fn test_delete_folder_nonempty_with_force() {
        let tmp = TempDir::new().unwrap();
        create_note(tmp.path(), "has-notes", "note1").unwrap();
        delete_folder(tmp.path(), "has-notes", true).unwrap();
        let folders = list_folders(tmp.path()).unwrap();
        assert!(folders.is_empty());
    }

    #[test]
    fn test_delete_folder_nonexistent_is_ok() {
        let tmp = TempDir::new().unwrap();
        let result = delete_folder(tmp.path(), "nope", false);
        assert!(result.is_ok());
    }

    // ── Git tests ───────────────────────────────────────────────────

    #[test]
    fn test_commit_notes_creates_repo_and_commits() {
        let tmp = TempDir::new().unwrap();
        let base = tmp.path();
        create_note(base, "proj", "hello").unwrap();

        let committed = commit_notes(base, "add hello").unwrap();
        assert!(committed);

        let repo = Repository::open(notes_root(base)).unwrap();
        let head = repo.head().unwrap().peel_to_commit().unwrap();
        assert_eq!(head.message().unwrap(), "add hello");
    }

    #[test]
    fn test_commit_notes_noop_when_no_changes() {
        let tmp = TempDir::new().unwrap();
        let base = tmp.path();
        create_note(base, "proj", "hello").unwrap();
        commit_notes(base, "first").unwrap();

        let committed = commit_notes(base, "second").unwrap();
        assert!(!committed);
    }

    #[test]
    fn test_commit_notes_tracks_deletes() {
        let tmp = TempDir::new().unwrap();
        let base = tmp.path();
        create_note(base, "proj", "temp").unwrap();
        commit_notes(base, "add temp").unwrap();

        delete_note(base, "proj", "temp.md").unwrap();
        let committed = commit_notes(base, "delete temp").unwrap();
        assert!(committed);
    }

    #[test]
    fn test_commit_notes_tracks_content_changes() {
        let tmp = TempDir::new().unwrap();
        let base = tmp.path();
        create_note(base, "proj", "doc").unwrap();
        commit_notes(base, "create").unwrap();

        write_note(base, "proj", "doc.md", "updated content").unwrap();
        let committed = commit_notes(base, "update doc").unwrap();
        assert!(committed);
    }

    // ── Image asset tests ───────────────────────────────────────────

    #[test]
    fn test_save_note_image_creates_assets_dir_and_file() {
        let tmp = TempDir::new().unwrap();
        let bytes = vec![0x89, 0x50, 0x4E, 0x47]; // fake PNG header
        let relative_path = save_note_image(tmp.path(), "proj", &bytes, "png").unwrap();
        assert!(relative_path.starts_with("assets/"));
        assert!(relative_path.ends_with(".png"));

        // File should exist on disk
        let full_path = notes_dir_with_base(tmp.path(), "proj").join(&relative_path);
        assert!(full_path.exists());
        assert_eq!(fs::read(&full_path).unwrap(), bytes);
    }

    #[test]
    fn test_save_note_image_rejects_invalid_extension() {
        let tmp = TempDir::new().unwrap();
        let result = save_note_image(tmp.path(), "proj", &[1, 2, 3], "exe");
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().kind(), std::io::ErrorKind::InvalidInput);
    }

    #[test]
    fn test_save_note_image_unique_filenames() {
        let tmp = TempDir::new().unwrap();
        let bytes = vec![1, 2, 3];
        let path1 = save_note_image(tmp.path(), "proj", &bytes, "png").unwrap();
        let path2 = save_note_image(tmp.path(), "proj", &bytes, "png").unwrap();
        assert_ne!(path1, path2);
    }

    #[test]
    fn test_resolve_note_asset_path_valid() {
        let tmp = TempDir::new().unwrap();
        let bytes = vec![1, 2, 3];
        let relative = save_note_image(tmp.path(), "proj", &bytes, "png").unwrap();
        let abs = resolve_note_asset_path(tmp.path(), "proj", &relative).unwrap();
        assert!(abs.exists());
        assert!(abs.is_absolute());
    }

    #[test]
    fn test_resolve_note_asset_path_rejects_traversal() {
        let tmp = TempDir::new().unwrap();
        let result = resolve_note_asset_path(tmp.path(), "proj", "../../../etc/passwd");
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().kind(), std::io::ErrorKind::InvalidInput);
    }

    #[test]
    fn test_resolve_note_asset_path_rejects_absolute() {
        let tmp = TempDir::new().unwrap();
        let result = resolve_note_asset_path(tmp.path(), "proj", "/etc/passwd");
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().kind(), std::io::ErrorKind::InvalidInput);
    }
}
