# GitHub CI Design

**Date:** 2026-03-14

## Goal

Bring GitHub Actions CI in line with the current repository standards and expand it into a clearer, more complete pipeline without adding high-cost packaging or multi-platform build jobs.

## Current Problems

- The workflow still triggers on `master` even though the repo now uses `main`.
- The workflow uses `npm`, but the repo has standardized on `pnpm`.
- CI does not enforce all required gates documented in the repo:
  - `pnpm check`
  - `cd src-tauri && cargo fmt --check`
  - `cd src-tauri && cargo clippy -- -D warnings`
- The existing jobs mix concerns, which makes failures less obvious and slows feedback.

## Constraints

- Keep Linux system package installation required by Tauri and Rust dependencies.
- Reuse cacheable setup for Node, pnpm, and Rust to keep CI practical.
- Do not add heavyweight packaging or release workflows as part of this change.
- Keep the workflow easy to read and hard to regress.

## Chosen Approach

Use a single `ci.yml` workflow with separate jobs for:

1. `frontend`
   - install dependencies with `pnpm`
   - run `pnpm check`
   - run frontend tests

2. `rust-lint`
   - install Linux system dependencies
   - run `cargo fmt --check`
   - run `cargo clippy -- -D warnings`

3. `rust-test`
   - install Linux system dependencies
   - run `cargo test`

This keeps the workflow easy to understand while allowing GitHub Actions to run independent checks in parallel.

## Testing Strategy

Extend the existing workflow regression test so it asserts the important contract of the CI file:

- triggers on `main`
- uses `pnpm`
- runs `pnpm check`
- runs `cargo fmt --check`
- runs `cargo clippy -- -D warnings`
- runs `cargo test`
- still installs `libasound2-dev`

This is intentionally a text-level regression test because the workflow contract matters more than implementation details.
