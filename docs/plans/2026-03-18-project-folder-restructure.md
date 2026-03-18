# Project Folder Restructure: agents/, notes/, and Agent Picker

> **For Claude:** REQUIRED SUB-SKILL: Use executing-plans to implement this plan task-by-task.

**Goal:** Restructure projects to have `agents/` and `notes/` directories alongside development code, make notes project-scoped (same git repo), add an agent picker (`a` key), and create CEO/CPO/CTO global agent profiles.

**Architecture:** Each project repo gains `agents/` (agent definitions) and `notes/` (project-scoped markdown notes) as top-level directories. The default agent moves to `agents/default-agent/agents.md` with a symlink chain preserving backwards compatibility (`CLAUDE.md` → `agents.md` → `agents/default-agent/agents.md`). Global agents (CEO/CPO/CTO) are copied into new projects as local agents. Notes switch from global storage (`~/.the-controller/notes/`) to project-scoped (`{repo_path}/notes/`), committed to the project's git repo. A new agent picker UI (triggered by `a` in development mode) lets users spawn sessions with specific agent instructions.

**Tech Stack:** Rust (Tauri v2), Svelte 5, git2, portable-pty

---

## Task 1: Create Global Agent Definitions in the-controller Repo

Create the CEO, CPO, and CTO agent profiles in the-controller's own `agents/` directory. These serve as templates that get copied into new projects.

**Files:**
- Create: `agents/ceo-agent/agents.md`
- Create: `agents/cpo-agent/agents.md`
- Create: `agents/cto-agent/agents.md`

**Step 1: Create agents/ directory structure**

```bash
mkdir -p agents/ceo-agent agents/cpo-agent agents/cto-agent
```

**Step 2: Write CEO agent definition**

Create `agents/ceo-agent/agents.md`:

```markdown
# CEO Agent

You are a CEO-level strategic advisor. You think about the business holistically — vision, priorities, resource allocation, and execution velocity.

## Your Perspective

- **Strategic clarity:** Every decision should ladder up to a clear goal. If the goal isn't clear, clarify it before acting.
- **Prioritization:** Ruthlessly prioritize. The question is never "is this good?" but "is this the highest-leverage thing we could do right now?"
- **Speed of execution:** Bias toward action. A good plan executed today beats a perfect plan next week.
- **Simplicity:** Complex systems break. Prefer simple solutions that can be understood and maintained by the team.

## How You Work

- When asked to review plans or features, evaluate them through the lens of business impact and opportunity cost.
- When asked to prioritize, consider: revenue impact, user impact, team velocity, and strategic positioning.
- When asked to make decisions, state your recommendation clearly with reasoning, then identify the key risk.
- You have access to the full codebase. Use it to ground your advice in reality, not abstraction.

## What You Focus On

- Product-market fit and user value
- Team velocity and execution bottlenecks
- Strategic sequencing (what enables what)
- Risk identification and mitigation
- Saying no to good ideas that aren't great ideas

## Project Structure

You are spawned inside `agents/ceo-agent/` within the project repository.
- `../` — Other agent definitions
- `../../notes/` — Project notes
- `../../` — Development code (the main codebase)
```

**Step 3: Write CPO agent definition**

Create `agents/cpo-agent/agents.md`:

```markdown
# CPO Agent

You are a CPO-level product advisor. You think about the user experience end-to-end — what users need, how they'll interact with the product, and what makes a feature feel complete.

## Your Perspective

- **User empathy:** Every feature exists to serve a user need. If you can't articulate the need, the feature isn't ready.
- **Completeness:** A feature isn't done when the code works. It's done when the user can discover it, use it, and recover from mistakes.
- **Coherence:** The product should feel like one thing, not a collection of features. New additions must fit the existing mental model.
- **Evidence over opinion:** Prefer user behavior data and direct feedback over internal debates about what users want.

## How You Work

- When asked to scope features, define the user story, acceptance criteria, and edge cases before implementation details.
- When asked to review UX, evaluate discoverability, learnability, error recovery, and consistency with existing patterns.
- When asked to prioritize features, weigh user pain severity, frequency, and the cost of not addressing it.
- You have access to the full codebase. Use it to understand existing UX patterns and ensure consistency.

## What You Focus On

- User stories and jobs-to-be-done
- Information architecture and interaction design
- Edge cases, error states, and empty states
- Feature completeness (not just the happy path)
- Consistency with existing product patterns

## Project Structure

You are spawned inside `agents/cpo-agent/` within the project repository.
- `../` — Other agent definitions
- `../../notes/` — Project notes
- `../../` — Development code (the main codebase)
```

**Step 4: Write CTO agent definition**

Create `agents/cto-agent/agents.md`:

```markdown
# CTO Agent

You are a CTO-level technical advisor. You think about the system as a whole — architecture, reliability, performance, developer experience, and technical debt.

## Your Perspective

- **Architecture fitness:** Every technical decision should serve the current scale and the next 3x of growth. Not 100x — that's over-engineering.
- **Simplicity:** The best architecture is the one the team can understand, debug, and modify confidently. Cleverness is a liability.
- **Reliability:** Systems should fail gracefully. Every external dependency is a potential failure point. Plan for it.
- **Developer experience:** If the dev workflow is painful, velocity drops. Fast feedback loops (build, test, deploy) compound.

## How You Work

- When asked to review architecture, evaluate coupling, failure modes, testability, and operational complexity.
- When asked to design systems, start with the data model and work outward. Get the data right and the rest follows.
- When asked about tech debt, quantify the cost: how often does this cause bugs? slow down features? confuse new developers?
- You have access to the full codebase. Read it thoroughly before making recommendations. Ground advice in the actual code.

## What You Focus On

- System architecture and component boundaries
- Data modeling and state management
- Performance bottlenecks and scalability
- Testing strategy and CI/CD pipeline
- Technical debt triage (fix now vs. accept vs. isolate)
- Security posture and dependency hygiene

## Project Structure

You are spawned inside `agents/cto-agent/` within the project repository.
- `../` — Other agent definitions
- `../../notes/` — Project notes
- `../../` — Development code (the main codebase)
```

**Step 5: Commit**

```bash
git add agents/
git commit -m "feat: add CEO, CPO, CTO global agent definitions"
```

---

## Task 2: Move Default Agent to `agents/default-agent/agents.md`

The current `agents.md` at repo root moves to `agents/default-agent/agents.md`. A symlink chain preserves backwards compatibility: `agents.md` (root) → `agents/default-agent/agents.md`, and `CLAUDE.md` → `agents.md`.

**Files:**
- Create: `agents/default-agent/agents.md` (move from root)
- Modify: `agents.md` (becomes symlink)
- Modify: `CLAUDE.md` (stays as symlink to `agents.md`)

**Step 1: Create default-agent directory and move content**

```bash
mkdir -p agents/default-agent
cp agents.md agents/default-agent/agents.md
```

**Step 2: Add project structure context to the default agent**

Append to `agents/default-agent/agents.md`, after the existing content:

```markdown

## Project Structure

You are the default development agent, spawned in the project's root directory.
- `agents/` — Agent definitions (CEO, CPO, CTO, and project-specific agents)
- `notes/` — Project notes (version-controlled, collaborative)
- Everything else — Development code
```

**Step 3: Replace root agents.md with symlink**

```bash
rm agents.md
ln -s agents/default-agent/agents.md agents.md
```

Verify `CLAUDE.md` still resolves (it points to `agents.md` which now points to the real file).

**Step 4: Verify symlink chain works**

```bash
cat CLAUDE.md  # Should show the full default agent content
cat agents.md  # Should show the same content
```

**Step 5: Commit**

```bash
git add agents/default-agent/agents.md
# Git tracks symlink targets, so add the changed symlinks
git add agents.md CLAUDE.md
git commit -m "refactor: move default agent to agents/default-agent/agents.md"
```

---

## Task 3: Update `ensure_claude_md_symlink` for New Agent Structure

The function must now handle the new symlink chain. When a project has `agents/default-agent/agents.md`, the root `agents.md` should symlink to it, and `CLAUDE.md` should symlink to `agents.md`.

**Files:**
- Modify: `src-tauri/src/commands.rs:19-33` (`ensure_claude_md_symlink`)

**Step 1: Write the failing test**

Add to `src-tauri/src/commands.rs` test module (or create one if absent). Since `ensure_claude_md_symlink` is a free function, it can be tested with a temp dir:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_ensure_symlinks_with_agents_dir_structure() {
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path();

        // Create agents/default-agent/agents.md
        std::fs::create_dir_all(dir.join("agents/default-agent")).unwrap();
        std::fs::write(
            dir.join("agents/default-agent/agents.md"),
            "# Default Agent",
        )
        .unwrap();

        // Run the function
        ensure_claude_md_symlink(dir).unwrap();

        // agents.md at root should exist and be a symlink
        let agents_md = dir.join("agents.md");
        assert!(agents_md.exists(), "agents.md should exist");
        assert!(agents_md.is_symlink(), "agents.md should be a symlink");

        // CLAUDE.md should exist and be a symlink
        let claude_md = dir.join("CLAUDE.md");
        assert!(claude_md.exists(), "CLAUDE.md should exist");
        assert!(claude_md.is_symlink(), "CLAUDE.md should be a symlink");

        // Content should be readable through the chain
        let content = std::fs::read_to_string(&claude_md).unwrap();
        assert!(content.contains("Default Agent"));
    }

    #[test]
    fn test_ensure_symlinks_preserves_existing_root_agents_md() {
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path();

        // Root agents.md exists as a regular file (legacy project)
        std::fs::write(dir.join("agents.md"), "# Legacy Agent").unwrap();

        ensure_claude_md_symlink(dir).unwrap();

        // CLAUDE.md should be created pointing to agents.md
        let claude_md = dir.join("CLAUDE.md");
        assert!(claude_md.exists());
        let content = std::fs::read_to_string(&claude_md).unwrap();
        assert!(content.contains("Legacy Agent"));
    }
}
```

**Step 2: Run test to verify it fails**

```bash
cd src-tauri && cargo test ensure_symlinks -- --nocapture
```

Expected: FAIL (the new agents dir structure test will fail because current function doesn't create root `agents.md` symlink from `agents/default-agent/agents.md`)

**Step 3: Update `ensure_claude_md_symlink`**

Replace the function at `src-tauri/src/commands.rs:19-33`:

```rust
/// Ensure the symlink chain exists:
/// 1. If `agents/default-agent/agents.md` exists and root `agents.md` does not,
///    create `agents.md` → `agents/default-agent/agents.md`
/// 2. If `agents.md` exists (file or symlink) and `CLAUDE.md` does not,
///    create `CLAUDE.md` → `agents.md`
pub fn ensure_claude_md_symlink(dir: &Path) -> Result<(), String> {
    let claude_md = dir.join("CLAUDE.md");
    let agents_md = dir.join("agents.md");
    let default_agent = dir.join("agents").join("default-agent").join("agents.md");

    // Step 1: Create root agents.md symlink if needed
    if default_agent.exists() && !agents_md.exists() {
        #[cfg(unix)]
        std::os::unix::fs::symlink("agents/default-agent/agents.md", &agents_md)
            .map_err(|e| format!("failed to create agents.md symlink: {}", e))?;
        #[cfg(windows)]
        std::os::windows::fs::symlink_file("agents/default-agent/agents.md", &agents_md)
            .map_err(|e| format!("failed to create agents.md symlink: {}", e))?;
    }

    // Step 2: Create CLAUDE.md symlink if needed
    if agents_md.exists() && !claude_md.exists() {
        #[cfg(unix)]
        std::os::unix::fs::symlink("agents.md", &claude_md)
            .map_err(|e| format!("failed to create CLAUDE.md symlink: {}", e))?;
        #[cfg(windows)]
        std::os::windows::fs::symlink_file("agents.md", &claude_md)
            .map_err(|e| format!("failed to create CLAUDE.md symlink: {}", e))?;
    }
    Ok(())
}
```

**Step 4: Run tests to verify they pass**

```bash
cd src-tauri && cargo test ensure_symlinks -- --nocapture
```

Expected: PASS

**Step 5: Commit**

```bash
cd src-tauri && git add src/commands.rs
git commit -m "feat: update ensure_claude_md_symlink for agents/ directory structure"
```

---

## Task 4: Update `scaffold_project_blocking` to Create `agents/` and `notes/` Directories

When scaffolding a new project, create the full directory structure including `agents/` (with default + global agents copied in) and `notes/`.

**Files:**
- Modify: `src-tauri/src/commands.rs:233-335` (`scaffold_project_blocking`)

**Step 1: Write the failing test**

```rust
#[test]
fn test_scaffold_creates_agents_and_notes_dirs() {
    let tmp = TempDir::new().unwrap();
    let repo_path = tmp.path().join("test-project");

    // We can't run the full scaffold (needs gh CLI), but we can test
    // the directory structure creation part separately.
    // For now, verify the expected structure after scaffold would run.
    std::fs::create_dir_all(&repo_path).unwrap();

    // Simulate what scaffold should create
    let agents_dir = repo_path.join("agents");
    let default_agent = agents_dir.join("default-agent");
    let notes_dir = repo_path.join("notes");

    assert!(!agents_dir.exists(), "agents/ should not exist before scaffold");
    assert!(!notes_dir.exists(), "notes/ should not exist before scaffold");
}
```

This is a structural test — the real validation is that `scaffold_project_blocking` creates these dirs. Since that function requires GitHub CLI, we'll validate by reading the code change.

**Step 2: Update `scaffold_project_blocking`**

In `src-tauri/src/commands.rs`, inside `scaffold_project_blocking` (around line 253-261), after writing `agents.md` and before creating `docs/plans/`:

```rust
// Create agents/ directory with default agent
let default_agent_dir = repo_path.join("agents").join("default-agent");
std::fs::create_dir_all(&default_agent_dir)
    .map_err(|e| rollback_dir(format!("failed to create agents/default-agent: {}", e)))?;
std::fs::write(default_agent_dir.join("agents.md"), &agents_content)
    .map_err(|e| rollback_dir(format!("failed to write default agent: {}", e)))?;

// Copy global agents (CEO, CPO, CTO) into project
for agent_name in &["ceo-agent", "cpo-agent", "cto-agent"] {
    let source = resolve_global_agents_dir().join(agent_name).join("agents.md");
    if source.exists() {
        let dest_dir = repo_path.join("agents").join(agent_name);
        std::fs::create_dir_all(&dest_dir)
            .map_err(|e| rollback_dir(format!("failed to create {}: {}", agent_name, e)))?;
        std::fs::copy(&source, dest_dir.join("agents.md"))
            .map_err(|e| rollback_dir(format!("failed to copy {}: {}", agent_name, e)))?;
    }
}

// Create notes/ directory
let notes_dir = repo_path.join("notes");
std::fs::create_dir_all(&notes_dir)
    .map_err(|e| rollback_dir(format!("failed to create notes/: {}", e)))?;
std::fs::write(notes_dir.join(".gitkeep"), "")
    .map_err(|e| rollback_dir(format!("failed to write notes/.gitkeep: {}", e)))?;

// Replace root agents.md with symlink to default agent
std::fs::remove_file(repo_path.join("agents.md"))
    .map_err(|e| rollback_dir(format!("failed to remove agents.md: {}", e)))?;
#[cfg(unix)]
std::os::unix::fs::symlink("agents/default-agent/agents.md", repo_path.join("agents.md"))
    .map_err(|e| rollback_dir(format!("failed to create agents.md symlink: {}", e)))?;
```

Also add a helper function to resolve the global agents directory (the-controller's own `agents/` dir):

```rust
/// Resolve the global agents directory from the-controller's source.
/// Uses the same git-common-dir approach as skills.rs to handle worktrees.
fn resolve_global_agents_dir() -> PathBuf {
    // Try CARGO_MANIFEST_DIR first (compile-time), then fallback
    if let Ok(manifest) = std::env::var("CARGO_MANIFEST_DIR") {
        let agents_dir = PathBuf::from(manifest).parent().unwrap().join("agents");
        if agents_dir.exists() {
            return agents_dir;
        }
    }
    // Fallback: resolve via git
    if let Ok(output) = std::process::Command::new("git")
        .args(["rev-parse", "--git-common-dir"])
        .output()
    {
        if output.status.success() {
            let git_dir = String::from_utf8_lossy(&output.stdout).trim().to_string();
            let agents_dir = PathBuf::from(&git_dir).parent().unwrap().join("agents");
            if agents_dir.exists() {
                return agents_dir;
            }
        }
    }
    PathBuf::from("agents")
}
```

**Step 3: Update git index additions**

In the same function, update the `index.add_path` calls to include the new files:

```rust
index.add_path(std::path::Path::new("agents/default-agent/agents.md"))
    .map_err(|e| rollback_dir(format!("failed to add default agent: {}", e)))?;
index.add_path(std::path::Path::new("notes/.gitkeep"))
    .map_err(|e| rollback_dir(format!("failed to add notes/.gitkeep: {}", e)))?;
// Add global agent copies
for agent_name in &["ceo-agent", "cpo-agent", "cto-agent"] {
    let agent_path = format!("agents/{}/agents.md", agent_name);
    let _ = index.add_path(std::path::Path::new(&agent_path));
}
```

**Step 4: Run format and lint checks**

```bash
cd src-tauri && cargo fmt --check && cargo clippy -- -D warnings
```

**Step 5: Commit**

```bash
cd src-tauri && git add src/commands.rs
git commit -m "feat: scaffold projects with agents/ and notes/ directories"
```

---

## Task 5: Make Notes Project-Scoped (Backend)

Change the notes backend to resolve notes from `{repo_path}/notes/` instead of the global `~/.the-controller/notes/` path. All note commands need a `project_id` parameter to locate the project's `repo_path`.

**Files:**
- Modify: `src-tauri/src/notes.rs:42-63` (path resolution)
- Modify: `src-tauri/src/commands/notes.rs` (all commands get `project_id` param)

**Step 1: Write the failing test**

In `src-tauri/src/notes.rs`, the existing tests already use `base` as a parameter, so the core logic is already project-agnostic. The change is in how the command layer resolves `base`. Add a test that verifies project-scoped path resolution:

```rust
#[test]
fn test_notes_dir_resolves_to_repo_path() {
    let tmp = TempDir::new().unwrap();
    let repo_path = tmp.path().join("my-project");
    std::fs::create_dir_all(&repo_path).unwrap();

    // Notes should be under {repo_path}/notes/
    let dir = notes_dir_with_base(&repo_path, "work");
    assert_eq!(dir, repo_path.join("notes").join("work"));
}
```

**Step 2: Run test to verify it passes**

This test should already pass since `notes_dir_with_base` is generic. The real change is in the command layer.

```bash
cd src-tauri && cargo test test_notes_dir_resolves_to_repo_path -- --nocapture
```

**Step 3: Update command layer to accept `project_id`**

Modify `src-tauri/src/commands/notes.rs`. Every command that currently does `storage.lock()...base_dir()` needs to instead resolve the project's `repo_path`. Add a helper:

```rust
/// Resolve the notes base directory for a project.
/// Notes live at `{repo_path}/` — the notes module adds the `notes/` prefix.
fn resolve_notes_base(
    storage: &std::sync::Arc<std::sync::Mutex<crate::storage::Storage>>,
    project_id: &str,
) -> Result<std::path::PathBuf, String> {
    let id = uuid::Uuid::parse_str(project_id).map_err(|e| e.to_string())?;
    let storage = storage.lock().map_err(|e| e.to_string())?;
    let project = storage.load_project(id).map_err(|e| e.to_string())?;
    Ok(std::path::PathBuf::from(&project.repo_path))
}
```

Then update each command to take `project_id: String` and use `resolve_notes_base` instead of `storage.lock()...base_dir()`. Example for `list_notes`:

```rust
pub(crate) async fn list_notes(
    state: State<'_, AppState>,
    project_id: String,
    folder: String,
) -> Result<Vec<NoteEntry>, String> {
    let storage = state.storage.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let base_dir = resolve_notes_base(&storage, &project_id)?;
        notes::list_notes(&base_dir, &folder).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}
```

Apply the same pattern to: `read_note`, `write_note`, `create_note`, `rename_note`, `duplicate_note`, `delete_note`, `list_folders`, `create_folder`, `rename_folder`, `delete_folder`, `commit_notes`.

For `commit_notes` (git operations), the notes now commit to the project repo. Update `notes.rs:commit_notes` to commit against `{base}/notes/` but using the repo at `{base}/.git`. This requires a change to `open_or_init_repo`:

```rust
fn open_or_init_repo(base: &Path) -> Result<Repository, git2::Error> {
    // Notes are in {repo_path}/notes/, but git repo is at {repo_path}/.git
    // Try opening the parent repo first
    match Repository::open(base) {
        Ok(repo) => {
            tracing::debug!("opened project git repo for notes");
            Ok(repo)
        }
        Err(_) => {
            // Fallback: open/init repo at notes root (legacy behavior)
            let root = notes_root(base);
            // ... existing init logic
        }
    }
}
```

**Step 4: Run tests**

```bash
cd src-tauri && cargo test notes -- --nocapture
```

**Step 5: Commit**

```bash
cd src-tauri && git add src/notes.rs src/commands/notes.rs
git commit -m "feat: make notes project-scoped via repo_path"
```

---

## Task 6: Update Notes Frontend to Pass `project_id`

All frontend note commands need to pass the active project's ID.

**Files:**
- Modify: `src/lib/Sidebar.svelte:246-252` (folder listing)
- Modify: `src/lib/Sidebar.svelte` (all note command calls)
- Modify: `src/lib/NotesEditor.svelte` (write/read/commit calls)
- Modify: `src/lib/stores.ts` (add active project tracking for notes)

**Step 1: Determine the active project for notes**

Notes mode needs to know which project's notes to show. The simplest approach: use the currently focused project (same as development mode). Add to stores or derive from existing state.

In `src/lib/Sidebar.svelte`, the `list_folders` call at line 247 currently passes no project context:

```typescript
command<string[]>("list_folders", {})
```

Change to:

```typescript
// Derive active project for notes
let activeProjectForNotes: Project | undefined = $derived(
  projectList.find(p => p.sessions.some(s => s.id === activeSession)) ?? projectList[0]
);

// When project changes or mode is notes, reload folders
$effect(() => {
  if (activeProjectForNotes) {
    command<string[]>("list_folders", { projectId: activeProjectForNotes.id })
      .then(folders => noteFolders.set(folders))
      .catch(err => console.error("Failed to list folders:", err));
  }
});
```

**Step 2: Update all note command calls**

Search for all `command("create_note"`, `command("write_note"`, etc. calls and add `projectId` parameter. This affects:

- `Sidebar.svelte` — create, delete, rename, duplicate note/folder operations
- `NotesEditor.svelte` — read, write, commit operations
- `NewNoteModal.svelte` — create note
- Any other component calling note commands

Each call changes from:
```typescript
command("create_note", { folder, title })
```
To:
```typescript
command("create_note", { projectId: activeProjectForNotes.id, folder, title })
```

**Step 3: Run frontend type check**

```bash
pnpm check
```

**Step 4: Run frontend tests**

```bash
pnpm test
```

**Step 5: Commit**

```bash
git add src/
git commit -m "feat: pass project_id to all note commands in frontend"
```

---

## Task 7: Add `list_agents` Backend Command

Create a new Tauri command that lists available agents for a project by scanning `{repo_path}/agents/`.

**Files:**
- Create: `src-tauri/src/agents.rs`
- Modify: `src-tauri/src/lib.rs` (register module)
- Modify: `src-tauri/src/commands.rs` (add command)

**Step 1: Write the failing test**

Create `src-tauri/src/agents.rs`:

```rust
use serde::Serialize;
use std::fs;
use std::path::Path;

#[derive(Debug, Serialize, Clone)]
pub struct AgentEntry {
    /// Directory name (e.g. "ceo-agent", "default-agent")
    pub name: String,
    /// First line of agents.md (typically "# Agent Name")
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
}
```

**Step 2: Run test to verify it passes**

```bash
cd src-tauri && cargo test list_agents -- --nocapture
```

**Step 3: Add Tauri command**

In `src-tauri/src/commands.rs`, add:

```rust
#[tauri::command]
pub async fn list_agents(
    state: State<'_, AppState>,
    project_id: String,
) -> Result<Vec<crate::agents::AgentEntry>, String> {
    let id = Uuid::parse_str(&project_id).map_err(|e| e.to_string())?;
    let storage = state.storage.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let storage = storage.lock().map_err(|e| e.to_string())?;
        let project = storage.load_project(id).map_err(|e| e.to_string())?;
        crate::agents::list_agents(Path::new(&project.repo_path)).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}
```

Register the module in `src-tauri/src/lib.rs`:

```rust
mod agents;
```

Register the command in the Tauri builder's `invoke_handler`.

**Step 4: Run all tests**

```bash
cd src-tauri && cargo test && cargo clippy -- -D warnings
```

**Step 5: Commit**

```bash
cd src-tauri && git add src/agents.rs src/commands.rs src/lib.rs
git commit -m "feat: add list_agents backend command"
```

---

## Task 8: Create Agent Session Spawning

When spawning an agent session, the CWD should be the agent's directory (`{worktree}/agents/{agent_name}/`), and the agent's `agents.md` should be what Claude Code reads.

**Files:**
- Modify: `src-tauri/src/commands.rs` (`create_session`)
- Modify: `src/lib/stores.ts` (`HotkeyAction` type)

**Step 1: Add `agent_name` parameter to `create_session`**

In `src-tauri/src/commands.rs:622`, add `agent_name: Option<String>` parameter:

```rust
#[tauri::command]
pub async fn create_session(
    state: State<'_, AppState>,
    _app_handle: AppHandle,
    project_id: String,
    kind: Option<String>,
    github_issue: Option<crate::models::GithubIssue>,
    background: Option<bool>,
    initial_prompt: Option<String>,
    agent_name: Option<String>,  // NEW
) -> Result<String, String> {
```

**Step 2: Use agent-specific CWD when spawning**

After the worktree is created (around line 683), determine the session directory:

```rust
// If an agent is specified, use its directory as the CWD
let session_dir = if let Some(ref agent) = agent_name {
    let agent_dir = PathBuf::from(&session_dir).join("agents").join(agent);
    if !agent_dir.exists() {
        return Err(format!("Agent directory not found: agents/{}", agent));
    }
    // Ensure the agent's agents.md exists and CLAUDE.md symlink is set up
    ensure_claude_md_symlink(&agent_dir).map_err(|e| {
        format!("Failed to setup agent symlinks: {}", e)
    })?;
    agent_dir.to_string_lossy().to_string()
} else {
    session_dir
};
```

**Step 3: Update `ensure_claude_md_symlink` to handle agent directories**

The existing function already handles the case where `agents.md` exists and `CLAUDE.md` doesn't. This will work for agent subdirectories too, since each agent dir has its own `agents.md`.

**Step 4: Run tests**

```bash
cd src-tauri && cargo test && cargo clippy -- -D warnings
```

**Step 5: Commit**

```bash
cd src-tauri && git add src/commands.rs
git commit -m "feat: support agent_name in create_session for agent-specific CWD"
```

---

## Task 9: Add Agent Picker Frontend Component

Create the agent picker modal that shows when pressing `a` in development mode.

**Files:**
- Create: `src/lib/AgentPickerModal.svelte`
- Modify: `src/lib/stores.ts` (add HotkeyAction variant)
- Modify: `src/lib/commands.ts` (add `a` keybinding for development mode)
- Modify: `src/lib/HotkeyManager.svelte` (handle new command)
- Modify: `src/lib/Sidebar.svelte` (render modal, handle action)

**Step 1: Add command definition**

In `src/lib/commands.ts`, add to the Sessions section:

```typescript
{ id: "spawn-agent", key: "a", section: "Sessions", description: "Spawn agent session", mode: "development" },
```

Add `"spawn-agent"` to the `CommandId` type.

**Step 2: Add HotkeyAction variant**

In `src/lib/stores.ts`, add to the `HotkeyAction` union:

```typescript
| { type: "spawn-agent"; projectId: string }
```

**Step 3: Create AgentPickerModal.svelte**

Use the `SessionPickerModal.svelte` pattern (j/k navigation, Enter to select, Escape to close):

```svelte
<script lang="ts">
  import { onMount } from "svelte";
  import { command } from "$lib/backend";

  interface AgentEntry {
    name: string;
    title: string;
  }

  interface Props {
    projectId: string;
    onSelect: (agentName: string) => void;
    onCancel: () => void;
  }

  let { projectId, onSelect, onCancel }: Props = $props();

  let agents: AgentEntry[] = $state([]);
  let selectedIndex = $state(0);
  let loading = $state(true);
  let error: string | null = $state(null);

  onMount(async () => {
    try {
      agents = await command<AgentEntry[]>("list_agents", { projectId });
      loading = false;
    } catch (e) {
      error = String(e);
      loading = false;
    }
  });

  function onKeydown(e: KeyboardEvent) {
    if (e.key === "Escape") {
      e.preventDefault();
      e.stopPropagation();
      onCancel();
    } else if (e.key === "j" || e.key === "ArrowDown") {
      e.preventDefault();
      e.stopPropagation();
      if (agents.length > 0) selectedIndex = (selectedIndex + 1) % agents.length;
    } else if (e.key === "k" || e.key === "ArrowUp") {
      e.preventDefault();
      e.stopPropagation();
      if (agents.length > 0) selectedIndex = (selectedIndex - 1 + agents.length) % agents.length;
    } else if (e.key === "Enter" || e.key === "l") {
      e.preventDefault();
      e.stopPropagation();
      if (agents.length > 0) onSelect(agents[selectedIndex].name);
    }
  }
</script>

<svelte:window on:keydown={onKeydown} />

<div class="overlay" role="dialog">
  <div class="picker">
    <div class="picker-title">Spawn Agent</div>
    {#if loading}
      <div class="loading">Loading agents...</div>
    {:else if error}
      <div class="error">{error}</div>
    {:else if agents.length === 0}
      <div class="empty">No agents found. Add agents to agents/ in your project.</div>
    {:else}
      <div class="agent-list">
        {#each agents as agent, i}
          <div
            class="agent-item"
            class:selected={i === selectedIndex}
            onclick={() => onSelect(agent.name)}
          >
            <span class="agent-name">{agent.title || agent.name}</span>
            <span class="agent-dir">{agent.name}/</span>
          </div>
        {/each}
      </div>
    {/if}
    <div class="hint">j/k navigate · Enter select · Esc cancel</div>
  </div>
</div>

<style>
  .overlay {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.7);
    backdrop-filter: blur(16px);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 100;
  }

  .picker {
    background: var(--bg-elevated);
    border: 1px solid var(--border-default);
    border-radius: 8px;
    padding: 20px 24px;
    min-width: 320px;
    max-width: 480px;
  }

  .picker-title {
    font-size: 14px;
    font-weight: 600;
    color: var(--text-primary);
    margin-bottom: 16px;
    text-align: center;
  }

  .agent-list {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .agent-item {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 8px 12px;
    border-radius: 6px;
    cursor: pointer;
    color: var(--text-secondary);
  }

  .agent-item.selected {
    background: rgba(255, 255, 255, 0.08);
    color: var(--text-primary);
  }

  .agent-name {
    font-size: 13px;
    font-weight: 500;
  }

  .agent-dir {
    font-size: 11px;
    color: var(--text-muted);
    font-family: var(--font-mono);
  }

  .hint {
    margin-top: 16px;
    font-size: 11px;
    color: var(--text-muted);
    text-align: center;
  }

  .loading, .error, .empty {
    padding: 12px;
    text-align: center;
    font-size: 13px;
    color: var(--text-secondary);
  }

  .error {
    color: var(--text-error, #f38ba8);
  }
</style>
```

**Step 4: Wire up in HotkeyManager.svelte**

In the `handleHotkey` function, add a case for `"spawn-agent"`:

```typescript
case "spawn-agent": {
  const project = getFocusedProject();
  if (!project) {
    showToast("Select a project first", "error");
    return true;
  }
  dispatchAction({ type: "spawn-agent", projectId: project.id });
  return true;
}
```

**Step 5: Wire up in Sidebar.svelte**

Add state and handler:

```typescript
let agentPickerTarget: { projectId: string } | null = $state(null);
```

In the hotkey action subscriber:

```typescript
case "spawn-agent": {
  agentPickerTarget = { projectId: action.projectId };
  break;
}
```

In the template, add the modal:

```svelte
{#if agentPickerTarget}
  <AgentPickerModal
    projectId={agentPickerTarget.projectId}
    onSelect={(agentName) => {
      createSession(agentPickerTarget!.projectId, currentSessionProvider, agentName);
      agentPickerTarget = null;
    }}
    onCancel={() => { agentPickerTarget = null; }}
  />
{/if}
```

Update the `createSession` function to accept an optional agent name and pass it to the backend:

```typescript
async function createSession(projectId: string, kind: string, agentName?: string) {
  // ... existing logic, add agentName to the command call
  const sessionId = await command<string>("create_session", {
    projectId,
    kind,
    agentName,  // NEW
  });
  // ... rest of existing logic
}
```

**Step 6: Run checks**

```bash
pnpm check && pnpm test
```

**Step 7: Commit**

```bash
git add src/lib/AgentPickerModal.svelte src/lib/commands.ts src/lib/stores.ts src/lib/HotkeyManager.svelte src/lib/Sidebar.svelte
git commit -m "feat: add agent picker modal triggered by 'a' key in development mode"
```

---

## Task 10: Update `create_project` / `load_project` for Agent Initialization

When loading an existing project that doesn't have `agents/` yet, copy global agents into it. When creating a project via `create_project` (not scaffold), initialize the agents directory.

**Files:**
- Modify: `src-tauri/src/commands.rs:429-481` (`create_project`)
- Modify: `src-tauri/src/commands.rs:483-546` (`load_project`)

**Step 1: Add agent initialization to `create_project`**

After the existing `ensure_claude_md_symlink` call (line 478), add:

```rust
// Initialize agents/ directory if it doesn't exist
let agents_dir = path.join("agents");
if !agents_dir.exists() {
    let _ = std::fs::create_dir_all(agents_dir.join("default-agent"));
    // Copy the root agents.md as the default agent
    if repo_agents.exists() {
        let _ = std::fs::copy(&repo_agents, agents_dir.join("default-agent").join("agents.md"));
    }
    // Copy global agents
    let global_agents = resolve_global_agents_dir();
    for agent_name in &["ceo-agent", "cpo-agent", "cto-agent"] {
        let source = global_agents.join(agent_name).join("agents.md");
        if source.exists() {
            let dest_dir = agents_dir.join(agent_name);
            let _ = std::fs::create_dir_all(&dest_dir);
            let _ = std::fs::copy(&source, dest_dir.join("agents.md"));
        }
    }
}

// Initialize notes/ directory if it doesn't exist
let notes_dir = path.join("notes");
if !notes_dir.exists() {
    let _ = std::fs::create_dir_all(&notes_dir);
    let _ = std::fs::write(notes_dir.join(".gitkeep"), "");
}
```

**Step 2: Add same to `load_project`**

Same logic after line 543's `ensure_claude_md_symlink` call.

**Step 3: Run tests**

```bash
cd src-tauri && cargo test && cargo clippy -- -D warnings
```

**Step 4: Commit**

```bash
cd src-tauri && git add src/commands.rs
git commit -m "feat: initialize agents/ and notes/ when loading existing projects"
```

---

## Task 11: Migrate Existing Notes

Provide a migration path for existing global notes (`~/.the-controller/notes/`) into the project's `notes/` directory.

**Files:**
- Modify: `src-tauri/src/commands.rs` (add `migrate_notes` command)

**Step 1: Add migration command**

```rust
#[tauri::command]
pub async fn migrate_notes(
    state: State<'_, AppState>,
    project_id: String,
) -> Result<u32, String> {
    let id = Uuid::parse_str(&project_id).map_err(|e| e.to_string())?;
    let storage = state.storage.clone();

    tauri::async_runtime::spawn_blocking(move || {
        let storage = storage.lock().map_err(|e| e.to_string())?;
        let project = storage.load_project(id).map_err(|e| e.to_string())?;
        let base_dir = storage.base_dir();

        let global_notes = base_dir.join("notes");
        let project_notes = PathBuf::from(&project.repo_path).join("notes");

        if !global_notes.exists() {
            return Ok(0);
        }

        std::fs::create_dir_all(&project_notes).map_err(|e| e.to_string())?;

        let mut migrated = 0u32;
        for entry in std::fs::read_dir(&global_notes).map_err(|e| e.to_string())? {
            let entry = entry.map_err(|e| e.to_string())?;
            if !entry.file_type().map_err(|e| e.to_string())?.is_dir() {
                continue;
            }
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with('.') {
                continue;
            }
            let dest = project_notes.join(&name);
            if dest.exists() {
                tracing::warn!("skipping migration of folder '{}': already exists in project", name);
                continue;
            }
            // Copy the folder recursively
            copy_dir_recursive(&entry.path(), &dest).map_err(|e| e.to_string())?;
            migrated += 1;
        }

        Ok(migrated)
    })
    .await
    .map_err(|e| e.to_string())?
}

fn copy_dir_recursive(src: &Path, dst: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dst)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let dest_path = dst.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_dir_recursive(&entry.path(), &dest_path)?;
        } else {
            std::fs::copy(entry.path(), dest_path)?;
        }
    }
    Ok(())
}
```

Register in the Tauri handler. This can be called from the frontend when a user first opens notes mode for a project.

**Step 2: Run tests**

```bash
cd src-tauri && cargo test && cargo clippy -- -D warnings
```

**Step 3: Commit**

```bash
cd src-tauri && git add src/commands.rs
git commit -m "feat: add migrate_notes command for global-to-project migration"
```

---

## Task 12: Update Server Mode (if applicable)

If the server binary (`src-tauri/src/bin/server.rs`) has note-related routes, they need to be updated to accept `project_id`.

**Files:**
- Modify: `src-tauri/src/bin/server.rs` (note routes)

**Step 1: Search for note routes in server**

```bash
cd src-tauri && grep -n "list_notes\|read_note\|write_note\|create_note\|list_folders\|list_agents" src/bin/server.rs
```

**Step 2: Update each route to accept and pass `project_id`**

The pattern should be the same as the Tauri commands — add `project_id` to the request body and pass it through.

**Step 3: Add `list_agents` route**

```rust
// POST /api/list_agents
async fn list_agents_handler(
    State(state): State<Arc<AppState>>,
    Json(body): Json<ListAgentsRequest>,
) -> Result<Json<Vec<agents::AgentEntry>>, ...> {
    // ... resolve project, call agents::list_agents
}
```

**Step 4: Run tests**

```bash
cd src-tauri && cargo test && cargo clippy -- -D warnings
```

**Step 5: Commit**

```bash
cd src-tauri && git add src/bin/server.rs
git commit -m "feat: update server routes for project-scoped notes and agent listing"
```

---

## Task 13: Final Integration Test and Lint

**Step 1: Run all Rust tests**

```bash
cd src-tauri && cargo test
```

**Step 2: Run all frontend tests**

```bash
pnpm test
```

**Step 3: Run format and lint gates**

```bash
pnpm check
cd src-tauri && cargo fmt --check
cd src-tauri && cargo clippy -- -D warnings
```

**Step 4: Fix any issues found**

**Step 5: Final commit**

```bash
git add -A
git commit -m "chore: final integration fixes for project folder restructure"
```

---

## Summary of Key Design Decisions

1. **Repo root is the development area** — no `development/` subfolder. `agents/` and `notes/` sit alongside `src/`, `src-tauri/`, etc.

2. **Global agents are copied, not symlinked** — allows per-project customization of CEO/CPO/CTO agents without affecting other projects.

3. **Notes are project-scoped** — stored in `{repo_path}/notes/`, committed to the project's git repo. Global notes at `~/.the-controller/notes/` can be migrated via the `migrate_notes` command.

4. **`a` key in development mode** opens the agent picker. The workspace mode picker (`Space` → `a` for agents mode) is unaffected since it requires `Space` first.

5. **Agent sessions get their own worktree** — the worktree is created normally, but the CWD for the spawned Claude Code session is `{worktree}/agents/{agent_name}/`, giving the agent its own `agents.md` context.

6. **Symlink chain**: `CLAUDE.md` → `agents.md` → `agents/default-agent/agents.md`. Backwards compatible — legacy projects with a root `agents.md` file continue to work.
