# Deterministic Maintainer Dedup Guard Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use the-controller-executing-plans to implement this plan task-by-task.

**Goal:** Add a deterministic dedup guard so semantically duplicate maintainer findings update existing `filed-by-maintainer` issues instead of creating new duplicates.

**Architecture:** Codex will return structured findings only; Rust in `maintainer.rs` will fetch existing issues, compute normalized fingerprints, apply deterministic similarity matching, and execute `gh issue create/edit/comment` actions accordingly.

**Tech Stack:** Rust (Tauri v2), serde JSON parsing, `gh` CLI, unit tests in `maintainer.rs`.

---

### Task 1: Add failing dedup tests (RED)

**Files:**
- Modify: `src-tauri/src/maintainer.rs`

1. Add tests for fingerprint normalization and semantic duplicate matching with mocked existing issues.
2. Run: `cd src-tauri && cargo test maintainer::tests::test_find_duplicate -- --nocapture`
3. Verify failure due to missing dedup functions/types.

### Task 2: Implement fingerprint + matching logic (GREEN)

**Files:**
- Modify: `src-tauri/src/maintainer.rs`

1. Add normalized token extraction and deterministic fingerprint generation.
2. Add duplicate matching with explicit threshold and deterministic tie-break.
3. Re-run targeted maintainer tests until green.

### Task 3: Integrate deterministic routing into maintainer pipeline

**Files:**
- Modify: `src-tauri/src/maintainer.rs`

1. Replace prompt contract to findings-only JSON.
2. Add GH helper functions to list/create/edit/comment issues from Rust.
3. Apply dedup matching before deciding update vs create.
4. Keep `MaintainerRunLog` compatibility and produce action summary.

### Task 4: Validate and self-review

**Files:**
- Modify: `src-tauri/src/maintainer.rs`
- Create: `docs/plans/2026-03-09-maintainer-dedup-guard-design.md`
- Create: `docs/plans/2026-03-09-maintainer-dedup-guard.md`

1. Run targeted tests: `cd src-tauri && cargo test maintainer::tests -- --nocapture`
2. Run full backend tests: `cd src-tauri && cargo test`
3. Self-review diff for determinism, error handling, and backward compatibility.
4. Commit with message including `closes #292`.

### Task 5: PR/merge/sync

1. Push branch and open PR with body containing `closes #292`.
2. Wait for checks; merge squash.
3. Delete remote branch.
4. Sync local `master` from origin.
