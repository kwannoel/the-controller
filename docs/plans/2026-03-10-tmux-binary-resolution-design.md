# Tmux Binary Resolution Design

## Summary

`TmuxManager` currently assumes tmux always lives at `/opt/homebrew/bin/tmux`, and `PtyManager::attach_tmux_session` duplicates that assumption with its own string literal. That breaks Intel Macs, where Homebrew installs tmux at `/usr/local/bin/tmux`, and it creates two separate launch paths that can drift apart.

## Goals

- Resolve tmux from a single shared code path.
- Support both Apple Silicon and Intel Homebrew defaults.
- Fall back to `PATH` lookup so explicitly configured environments still work.
- Add regression coverage that fails if tmux resolution or tmux attach reverts to a hardcoded path.

## Non-Goals

- Changing tmux session naming or lifecycle behavior.
- Refactoring unrelated PTY management logic.
- Adding new configuration for custom tmux paths.

## Approach Options

### Option 1: Replace the constant with `/usr/local/bin/tmux`

Pros:

- Tiny code change.

Cons:

- Breaks Apple Silicon again.
- Leaves duplicate attach logic untouched.

### Option 2: Check both Homebrew locations at runtime, then fall back to `PATH`

Pros:

- Covers Intel and Apple Silicon defaults.
- Preserves support for users who expose tmux through `PATH`.
- Gives `TmuxManager` and PTY attach one shared source of truth.

Cons:

- Slightly more code than a single constant.

Recommendation: Option 2.

## Design

Add a shared tmux resolver in `src-tauri/src/tmux.rs` that prefers:

1. `/opt/homebrew/bin/tmux`
2. `/usr/local/bin/tmux`
3. the first executable `tmux` found in `PATH`

Update every `std::process::Command::new(...)` call in `TmuxManager` to use the resolved binary, and expose the same resolved path to `PtyManager::attach_tmux_session`.

## Testing

- First, add a failing unit test proving `TmuxManager::is_available()` succeeds when tmux is only available via `PATH`.
- Then add a failing PTY-manager regression test proving `spawn_session` can create and attach to a tmux session using that shared resolution path.
- Re-run the targeted tests for red/green verification, then run the Rust test suite for broader confirmation.
