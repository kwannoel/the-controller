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

Skills live in `skills/the-controller-*/` inside the repo. On app startup, `sync_skills()` symlinks each skill directory into both agent homes:

- `~/.claude/skills/the-controller-<name>/` (Claude Code)
- `~/.codex/skills/custom/the-controller-<name>/` (Codex)

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
