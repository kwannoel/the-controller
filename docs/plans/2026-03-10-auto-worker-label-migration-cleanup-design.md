# Auto-worker Label Migration Cleanup Design

**Problem**

The auto-worker still carries runtime migration logic for legacy GitHub labels even though the repo is now standardized on canonical labels. That keeps extra startup work, extra code paths, and tests for behavior we no longer want to support.

**Decision**

Adopt strict canonical labels for auto-worker scheduling:

- `priority:high|low`
- `complexity:low|high`

Legacy labels such as `priority: high`, `complexity: low`, and `complexity:simple` are no longer supported by the scheduler.

**Design**

- Remove startup background migration from `src-tauri/src/auto_worker.rs`
- Remove migration helpers and legacy-label constants/types from the same file
- Keep eligibility checks strict and canonical-only
- Replace migration-oriented tests with canonical behavior tests plus one explicit regression test that legacy labels are not eligible

**Validation**

- `cargo test auto_worker::tests --manifest-path src-tauri/Cargo.toml`
- `rg 'migrate_labels_background|migrate_issues_sync|LabelMigration|priority: high|complexity: low|complexity:simple' src-tauri/src/auto_worker.rs`
