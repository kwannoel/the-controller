# Issue Cache Invalidation for Auto-Worker Label Edits Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use the-controller-executing-plans to implement this plan task-by-task.

**Goal:** Ensure auto-worker label edits cannot leave `IssueCache` stale after `gh issue edit` succeeds.

**Architecture:** Keep the existing dual-path design, but add an explicit cache invalidation hook for the auto-worker's synchronous label-edit helpers. Successful auto-worker label edits evict the repo cache entry so the next issue read refetches fresh GitHub data.

**Tech Stack:** Rust, Tauri v2, GitHub CLI, cargo test

---

### Task 1: Add the regression test for auto-worker cache invalidation

**Files:**
- Create: `docs/plans/2026-03-10-issue-cache-invalidation-design.md`
- Create: `docs/plans/2026-03-10-issue-cache-invalidation.md`
- Modify: `src-tauri/src/auto_worker.rs`
- Test: `src-tauri/src/auto_worker.rs`

**Step 1: Write the failing test**

Add a unit test in `src-tauri/src/auto_worker.rs` for a helper that wraps a synchronous label edit and cache invalidation. Cover two cases:
- success removes the seeded repo entry from `IssueCache`;
- failure keeps the seeded repo entry intact.

**Step 2: Run test to verify it fails**

Run: `cd src-tauri && cargo test auto_worker::tests::successful_label_edit_invalidates_issue_cache`

Expected: FAIL because the helper does not exist yet and the auto-worker label path has no cache invalidation behavior.

**Step 3: Write minimal implementation**

In `src-tauri/src/auto_worker.rs`:
- extract the raw `gh issue edit` execution into a small helper;
- add a wrapper that accepts `&AppState`, runs the edit, and invalidates the repo cache on success.

In `src-tauri/src/state.rs`:
- add `IssueCache::invalidate`.

**Step 4: Run test to verify it passes**

Run: `cd src-tauri && cargo test auto_worker::tests::successful_label_edit_invalidates_issue_cache`

Expected: PASS.

**Step 5: Commit**

```bash
git add src-tauri/src/auto_worker.rs src-tauri/src/state.rs docs/plans/2026-03-10-issue-cache-invalidation-design.md docs/plans/2026-03-10-issue-cache-invalidation.md
git commit -m "fix: invalidate issue cache after auto-worker label edits"
```

### Task 2: Wire the invalidation through all auto-worker label edit call sites

**Files:**
- Modify: `src-tauri/src/auto_worker.rs`

**Step 1: Write the failing test**

Extend the same unit test coverage or add a second focused test proving the helper only invalidates on successful edits, so callers can safely reuse it for issue pickup, cleanup, completion, and migration paths.

**Step 2: Run test to verify it fails**

Run: `cd src-tauri && cargo test auto_worker::tests::failed_label_edit_keeps_issue_cache`

Expected: FAIL before the helper preserves cache entries on edit failure.

**Step 3: Write minimal implementation**

Update the auto-worker call sites to use the new cache-aware helpers:
- stale-label cleanup;
- issue pickup;
- label migration;
- worker completion/finish handling.

Thread `&AppState` where needed, keeping the change local to `auto_worker.rs`.

**Step 4: Run test to verify it passes**

Run: `cd src-tauri && cargo test auto_worker::tests::failed_label_edit_keeps_issue_cache`

Expected: PASS.

**Step 5: Commit**

```bash
git add src-tauri/src/auto_worker.rs src-tauri/src/state.rs
git commit -m "fix: route auto-worker label edits through cache invalidation"
```

### Task 3: Verify the regression and shared backend behavior

**Files:**
- Modify: `src-tauri/src/auto_worker.rs` if cleanup is needed
- Modify: `src-tauri/src/state.rs` if cleanup is needed

**Step 1: Run focused backend verification**

Run: `cd src-tauri && cargo test auto_worker::tests::successful_label_edit_invalidates_issue_cache auto_worker::tests::failed_label_edit_keeps_issue_cache state::tests::test_issue_cache_invalidate_removes_repo_entry`

Expected: PASS.

**Step 2: Run broader backend verification**

Run: `cd src-tauri && cargo test`

Expected: PASS, or capture any unrelated pre-existing failures explicitly.

**Step 3: Self-review the diff**

Check:
- every successful auto-worker label mutation invalidates the cache;
- failed mutations leave cache state alone;
- direct UI GitHub commands still use their existing precise cache updates.

**Step 4: Commit**

```bash
git add src-tauri/src/auto_worker.rs src-tauri/src/state.rs
git commit -m "test: cover auto-worker issue cache invalidation"
```
