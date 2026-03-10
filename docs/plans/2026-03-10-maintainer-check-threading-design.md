# Maintainer Check Threading Design

**Issue:** `trigger_maintainer_check` blocks a Tokio async runtime thread while the maintainer check shells out to the Claude CLI.

## Definition

Manual maintainer checks should behave like the rest of the app's slow Tauri commands: schedule blocking work on the dedicated blocking pool and keep the async runtime responsive. The fix is limited to the manual trigger path in `src-tauri/src/commands.rs`.

## Constraints

- Preserve the existing command API and emitter behavior.
- Follow the pattern already used elsewhere in the codebase for blocking Git/CLI work.
- Keep the maintainer pipeline implementation in `src-tauri/src/maintainer.rs` unchanged unless testing requires a narrow seam.
- Add a regression test before the production fix.
- Prefer a behavioral test over a source-shape assertion.

## Approach Options

### Option 1: Wrap the existing maintainer call in `tokio::task::spawn_blocking`

Add a small async helper in `commands.rs` that moves owned inputs into a blocking closure, then await and unwrap the nested `Result`.

Pros:
- Smallest production change
- Matches `connect_session` and other existing commands
- Easy to keep the public command behavior unchanged

Cons:
- Needs a small test seam to verify off-thread execution without invoking the full CLI pipeline

### Option 2: Make `run_maintainer_check` async

Push the async boundary into `maintainer.rs` and wrap the blocking CLI pieces there.

Pros:
- Makes the offloading explicit closer to the blocking work

Cons:
- Larger refactor touching scheduler and other call sites
- Unnecessary for the issue scope

## Chosen Design

Use Option 1. Introduce a helper in `src-tauri/src/commands.rs` that offloads the maintainer check to `spawn_blocking`, then have `trigger_maintainer_check` call that helper. For testing, add a second helper that accepts an injected closure so a unit test can verify the work runs on a different thread from the async runtime.

## Validation

1. Write a regression test that fails while the helper runs inline and passes once the work is dispatched via `spawn_blocking`.
2. Run the targeted Rust test for the new regression.
3. Run the full Rust test suite for `src-tauri`.
4. Self-review the diff for behavior regressions in success and error emission paths.
