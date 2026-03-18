---
name: synchronizing-skills
description: Use when a new skill has been added to the repo and needs to be symlinked into ~/.claude/skills/ and ~/.codex/skills/custom/ so Claude Code and Codex can discover it
---

# Synchronizing Skills

## Overview

Syncs skill directories from the repo into agent home directories as symlinks. Run this after adding, renaming, or removing skills.

## Usage

Run the sync script from anywhere inside the repo (including worktrees):

```bash
bash "$(git rev-parse --show-toplevel)/skills/synchronizing-skills/sync.sh"
```

The script reports what was added, removed, or already up to date.
