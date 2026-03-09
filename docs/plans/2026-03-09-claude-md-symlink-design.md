# CLAUDE.md Symlink for New Projects

## Problem

Claude Code looks for `CLAUDE.md` in the repo root to load project instructions. The Controller creates `agents.md` but not `CLAUDE.md`, so Claude Code doesn't pick up the instructions.

## Solution

Create `CLAUDE.md` as a symlink to `agents.md` wherever `agents.md` is created in the repo root.

### `scaffold_project`

After writing `agents.md` to repo root:
1. Create `CLAUDE.md` symlink → `agents.md`
2. Add `CLAUDE.md` to git index
3. Both files included in the initial commit

### `create_project` / `load_project`

When the repo already has `agents.md` in its root but no `CLAUDE.md`:
1. Create `CLAUDE.md` symlink → `agents.md` in repo root

When the repo lacks `agents.md` (config dir fallback):
- No change — non-invasive, don't touch the repo

### Helper

Extract a shared function `ensure_claude_md_symlink(repo_path)`:
- If `CLAUDE.md` doesn't exist and `agents.md` does exist in `repo_path` → create symlink
- If `CLAUDE.md` already exists → skip

### Tests

- Update `test_scaffold_project_creates_template_files` to verify `CLAUDE.md` symlink exists and points to `agents.md`
- Update `test_scaffold_initial_commit_contains_template_files` to verify `CLAUDE.md` is in the commit tree
- Add test for create/load: when repo has `agents.md` but no `CLAUDE.md`, symlink is created
- Add test for create/load: when repo has both, nothing changes
