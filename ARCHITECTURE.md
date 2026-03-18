# Architecture

## Agent Compatibility: Claude Code & Codex

The Controller supports both Claude Code and Codex as coding agents. Compatibility is maintained through two mechanisms: a shared instruction file and skill synchronization.

### Shared Instruction File (`agents.md`)

Claude Code reads project instructions from `CLAUDE.md`. Rather than maintaining separate files, we use `agents.md` as the single source of truth and symlink `CLAUDE.md` to it. This happens automatically:

- On project scaffold (`scaffold_project_blocking`) — both files are committed to git
- On project load/create — `ensure_claude_md_symlink()` creates the symlink if `agents.md` exists and `CLAUDE.md` doesn't

The symlink is non-invasive: if a real `CLAUDE.md` already exists, it's left alone.

See: `src-tauri/src/commands.rs` (`ensure_claude_md_symlink`)

### Skill Synchronization on Bootstrap

Skills live in `skills/<name>/` inside the repo (each containing a `SKILL.md`). On app startup, `sync_skills()` symlinks each skill directory into both agent homes using the raw directory name:

- `~/.claude/skills/<name>/` (Claude Code)
- `~/.codex/skills/custom/<name>/` (Codex)

The sync is idempotent, worktree-aware (resolves to the main repo via `git rev-parse --git-common-dir`), and cleans up stale symlinks whose targets no longer exist. Regular files are never overwritten — only symlinks are managed.

See: `src-tauri/src/skills.rs`

## Why We Vendorize Skills

Skills are authored and versioned inside this repo rather than relying on Claude's built-in skill discovery. The reasons:

1. **Merge-time control** — We want specific things to happen when skills are merged. Keeping them in-repo means PRs, reviews, and CI apply to skill changes the same as code changes.
2. **Development workflow control** — We control the development workflow, injecting important constraints where we see importance (e.g., mandatory task structure, verification before completion).
3. **Standardized behavior** — Both Claude Code and Codex get the exact same skill definitions, ensuring consistent behavior regardless of which agent runs a session.

If any of these motivations change — for example, if upstream skill management becomes sufficiently flexible — we may revise this decision.

## Claude vs Codex: When to Use Which

Claude Code is the first-class citizen for this project. Codex handles background maintenance.

**Claude Code** is preferred for the majority of work because:

- **Better UX** — richer interaction model, inline feedback, plan mode
- **General thinking** — stronger at reasoning through ambiguous problems and making judgment calls
- **Meta thinking** — better at reflecting on its own approach and course-correcting
- **Design sense** — most of the work on this project is design work (architecture, UX, interaction patterns), where Claude excels

**Codex** is useful for:

- Pushing out straightforward code changes at volume
- Background maintenance tasks (dependency updates, mechanical refactors)
- Parallel execution of well-defined, independent implementation tasks

Since the project's current focus is heavily design-oriented — shaping interactions, defining workflows, refining the development experience — Claude Code is the better fit for the primary development loop.

## Metadata and Worktree Layout

Session state lives on disk in two places: project metadata (JSON) and git worktrees. Understanding the layout is essential for manual recovery.

### Directory Structure

```
~/.the-controller/
├── config.json                              # App-level config (projects_root)
├── projects/
│   └── {project-uuid}/
│       └── project.json                     # Project + all session metadata
└── worktrees/
    └── {project-name}/
        └── {session-label}/                 # Git worktree checkout
            ├── .env -> ../../.env           # Symlinked from main repo
            └── ...                          # Full working copy
```

### Project Metadata (`project.json`)

Each project gets a `project.json` keyed by UUID under `~/.the-controller/projects/`. It contains the project name, repo path, and an array of `SessionConfig` entries — one per session. Each session records:

- `id` — Session UUID
- `label` — Human-readable label (e.g., `session-3-a1b2c3`)
- `worktree_path` — Absolute path to the worktree directory
- `worktree_branch` — Git branch name (matches the label)
- `archived`, `kind`, `github_issue`, `initial_prompt`, `done_commits`

This is the authoritative record. If a worktree directory exists but has no matching session in `project.json`, the controller won't know about it.

### Session Labels

Labels are auto-generated: `session-{N}-{6-char-uuid}`. The number increments based on the highest existing session number in the project. The UUID suffix ensures uniqueness across parallel creation.

See: `commands.rs` (`next_session_label`)

### Worktree Creation

When a session is created:

1. A new git branch is created with the session label as its name
2. `git2::Repository::worktree()` creates a worktree at `~/.the-controller/worktrees/{project-name}/{session-label}/`
3. The main repo's `.env` is symlinked into the worktree so secrets are shared
4. If the repo has no commits (unborn branch), the repo path is used directly — no worktree is created

See: `worktree.rs` (`create_worktree`)

### Worktree Cleanup

- **Close session** with `delete_worktree=true`: removes the directory, prunes the git worktree reference, and deletes the branch
- **Delete project**: iterates all sessions, closes PTYs, and removes each worktree
- **Failed spawn**: if PTY spawn fails after worktree creation, the worktree and branch are rolled back automatically

See: `worktree.rs` (`remove_worktree`), `commands.rs` (`cleanup_failed_session_spawn`)

### Recovery

If the app crashes or metadata gets corrupted, here's what you need to know:

- **Worktrees are plain git worktrees.** You can list them with `git worktree list` from the main repo. They're normal checkouts — you can `cd` into them and work directly.
- **Branches match session labels.** Each worktree branch is named after the session label, so `git branch` in the main repo shows all session branches.
- **Metadata is JSON.** You can read/edit `~/.the-controller/projects/{uuid}/project.json` directly to fix session records or update stale `worktree_path` values.
- **Orphaned worktrees** (directory exists, no metadata entry) can be recovered by adding a session entry back to `project.json`, or cleaned up with `git worktree remove`.
- **Orphaned metadata** (entry exists, directory missing) will fail on connect. Remove the session entry from `project.json` or recreate the worktree manually with `git worktree add`.
- **Legacy migration**: older installs used `worktrees/{project-uuid}/` instead of `worktrees/{project-name}/`. The `restore_sessions` command migrates these on startup automatically.
