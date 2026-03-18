# Unreadable Enumeration Design

## Summary

Global storage scans should keep returning valid projects and maintainer run logs even when one JSON file cannot be read. Today `Storage::load_run_logs_from_dir` aborts on the first unreadable log file, and `Storage::list_projects` skips unreadable project files without recording them as corrupt inventory.

## Goals

- Keep global enumeration available when an individual file is unreadable.
- Treat unreadable `project.json` files like corrupt project metadata so callers can warn consistently.
- Skip unreadable maintainer log files instead of aborting history and status lookups.
- Keep the change local to storage and its tests.

## Non-Goals

- Changing the public API for maintainer history or status endpoints.
- Adding a new UI surface for maintainer log read failures.
- Repairing unreadable files automatically.

## Approach

1. Update `Storage::list_projects` so per-file read failures populate `corrupt_entries` rather than only printing a warning.
2. Update `Storage::load_run_logs_from_dir` so unreadable `.json` files are logged and skipped, matching the existing behavior for old-format log files that fail deserialization.
3. Add regression tests that create one valid file and one unreadable file in the same scan directory, asserting the valid entry remains available and the scan does not error.

## Validation

- `cargo test --manifest-path src-tauri/Cargo.toml storage::tests::test_list_projects_reports_unreadable_project_json_as_corrupt`
- `cargo test --manifest-path src-tauri/Cargo.toml storage::tests::test_run_log_history_skips_unreadable_log_files`
- Revert-check each new regression locally to confirm it fails without the implementation.
