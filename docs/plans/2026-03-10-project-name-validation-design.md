# Project Name Validation Design

## Summary

`validate_project_name` already defines the allowed project-name rules, but `create_project` and `load_project` bypass it and persist names directly. That leaves command behavior inconsistent with `scaffold_project` and allows invalid names to reach later worktree path construction. The fix should make all project-creation entry points enforce the same validation contract.

## Goals

- Reject invalid project names consistently across `create_project`, `load_project`, and `scaffold_project`.
- Stop invalid names before they are persisted to storage.
- Add command-level regression tests that fail if either command stops calling the shared validator.

## Non-Goals

- Changing the validation rules themselves.
- Renaming or migrating existing stored projects.
- Refactoring unrelated project-loading or session code.

## Approach Options

### Option 1: Duplicate the validation logic inside each command

Pros:

- Very small code change.

Cons:

- Reintroduces drift risk between commands.
- Makes future validation changes easy to miss.

### Option 2: Call `validate_project_name` from the missing command entry points

Pros:

- Reuses the existing validation contract.
- Keeps all commands aligned with the same helper.
- Smallest change that fixes the bug.

Cons:

- Still depends on command-level tests to prevent future regressions.

Recommendation: Option 2.

## Design

Add `validate_project_name(&name)?;` near the start of both `create_project` and `load_project`, before duplicate-name checks and before any project metadata is saved.

Regression coverage should exercise the commands directly:

- `create_project` with an invalid name should return `Err("Invalid project name: ...")`.
- `load_project` with an invalid name should return the same error even when the repo path points at a valid git repository.

That keeps the tests focused on the actual bug: helper behavior already has coverage, but the commands need proof they invoke it.

## Testing

- Add one failing unit test for `create_project`.
- Add one failing unit test for `load_project`.
- Run those tests first to confirm red.
- Implement the minimal command changes.
- Re-run the targeted tests and then the Rust command test suite for confirmation.
