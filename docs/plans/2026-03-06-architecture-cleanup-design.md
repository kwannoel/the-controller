# Architecture Cleanup Review (Issue #108)

## Scope

Review the current codebase architecture for simplification opportunities and implement one scoped, non-behavioral cleanup that improves maintainability.

## Audit Findings

1. `src-tauri/src/commands.rs` had grown into a monolithic command hub (~1.7k LOC) with mixed responsibilities:
- project/session lifecycle
- GitHub issue/label operations
- media/clipboard/screenshot commands
- merge + commit history behaviors

2. GitHub command flow included repeated patterns for extracting `owner/repo` and running `gh` command sequences.

3. Media-specific command logic (clipboard image + screenshot capture) lived inline with unrelated domain commands, increasing cognitive load.

## Implemented Cleanup (This Change)

1. Extracted GitHub command implementation into `src-tauri/src/commands/github.rs`.
2. Extracted media/clipboard command implementation into `src-tauri/src/commands/media.rs`.
3. Kept `#[tauri::command]` entrypoints in `src-tauri/src/commands.rs` as thin wrappers to preserve the existing public command surface and `generate_handler!` compatibility.
4. Moved GitHub parsing tests into the new `github.rs` module test block.

## Why This Is Safe

- No command names changed.
- No argument/return signatures changed.
- `tauri::generate_handler!` registration remains unchanged.
- Existing Rust + frontend tests remain green after refactor.

## Follow-up Cleanup Candidates

1. Split remaining `commands.rs` wrappers and domain code into explicit subdomains (`project`, `session`, `merge`, `history`) while preserving command wrappers.
2. Consolidate repeated storage/session lookup patterns in command handlers into small shared helpers.
3. Move `discover_branch_commits` and related git-history helpers to a dedicated module near `worktree.rs`.
4. Standardize error construction in command handlers (e.g., helper for CLI stderr extraction).
