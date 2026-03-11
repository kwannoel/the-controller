#!/usr/bin/env bash
set -euo pipefail

SKILL_PREFIX="the-controller-"

# Resolve main repo root (handles worktrees via --git-common-dir)
git_common_dir="$(git rev-parse --path-format=absolute --git-common-dir 2>/dev/null)" || {
  echo "Error: not inside a git repository" >&2
  exit 1
}
# --git-common-dir returns the .git dir; parent is the repo root
repo_root="$(dirname "$git_common_dir")"
skills_source="$repo_root/skills"

if [[ ! -d "$skills_source" ]]; then
  echo "Error: skills directory not found at $skills_source" >&2
  exit 1
fi

# Target directories
claude_skills_dir="$HOME/.claude/skills"
codex_skills_dir="$HOME/.codex/skills/custom"

sync_to_target() {
  local target_dir="$1"
  local label="$2"

  mkdir -p "$target_dir"

  # Clean stale symlinks
  for link in "$target_dir"/${SKILL_PREFIX}*; do
    [[ -e "$link" || -L "$link" ]] || continue
    if [[ -L "$link" && ! -e "$link" ]]; then
      echo "  removed stale: $(basename "$link")"
      rm "$link"
    fi
  done

  # Create/update symlinks
  for skill_path in "$skills_source"/${SKILL_PREFIX}*/; do
    [[ -d "$skill_path" ]] || continue
    local name
    name="$(basename "$skill_path")"
    local link_path="$target_dir/$name"

    if [[ -L "$link_path" ]]; then
      local existing_target
      existing_target="$(readlink "$link_path")"
      if [[ "$existing_target" == "$skill_path" || "$existing_target" == "${skill_path%/}" ]]; then
        continue  # Already correct
      fi
      rm "$link_path"
    elif [[ -e "$link_path" ]]; then
      echo "  skipped (not a symlink): $name" >&2
      continue
    fi

    ln -s "${skill_path%/}" "$link_path"
    echo "  added: $name"
  done
}

echo "Syncing skills from $skills_source"
echo ""
echo "~/.claude/skills/:"
sync_to_target "$claude_skills_dir" "Claude"
echo ""
echo "~/.codex/skills/custom/:"
sync_to_target "$codex_skills_dir" "Codex"
echo ""
echo "Done."
