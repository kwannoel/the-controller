# AppState Storage Init Failure Design

**Issue:** `AppState::new()` panics on startup if the default storage directory cannot be resolved or created.

## Definition

Startup should fail gracefully when storage initialization cannot complete. Instead of panicking in Rust before the Tauri app is usable, the app should surface a user-facing error and exit cleanly.

## Constraints

- Keep the change narrowly scoped to startup initialization.
- Preserve the existing `AppState` structure for downstream command handlers.
- Remove panic paths from both storage directory resolution and directory creation.
- Do not start background schedulers when app state initialization fails.
- Keep validation automated and non-brittle.

## Validation

- Add a regression test that forces storage initialization to fail because the configured base path is unusable.
- Verify the new test fails before the implementation and passes after it.
- Run the relevant Rust test suite after implementation.

## Approaches Considered

### 1. Minimal unwrap replacement in `AppState::new()`

Convert the `unwrap()` to `expect()` or map the error to a string but keep constructor semantics otherwise.

**Rejected:** this still leaves `Storage::with_default_path()` panicking and does not provide a user-facing failure mode.

### 2. Return `Result` from storage and app-state constructors, handle failure in bootstrap

Make `Storage::with_default_path()` and `AppState::new()` return `Result`. In `run()`, initialize state before building the app loop; if initialization fails, show a native error dialog and return without starting the application.

**Chosen:** fixes the root cause, removes both panic sites, and keeps the failure handling centralized at startup.

### 3. Delay state initialization until after app startup

Build the Tauri app first, then initialize state in `setup()` and exit if it fails.

**Rejected:** it complicates startup ordering and leaves the app briefly alive without required state.

## Design

1. Add a storage-path resolution helper that returns `std::io::Result<PathBuf>` instead of panicking when the home directory cannot be determined.
2. Add an `AppState::from_storage(Storage) -> std::io::Result<Self>` constructor for testable initialization.
3. Change `AppState::new()` to return `std::io::Result<Self>` and delegate to `from_storage`.
4. In `run()`, attempt to build `AppState` before registering it with Tauri.
5. If initialization fails, show a native error dialog describing the storage initialization failure and return early instead of panicking.

## Error Handling

- Home directory resolution failure becomes an `io::Error`.
- Directory creation failure is propagated unchanged.
- Startup dialog message should explain that app storage could not be initialized and include the underlying error text.

## Testing

- Unit test for `AppState::from_storage` returning an error when the storage base path is a file.
- Unit test for storage default-path resolution returning an error when no home directory is available.
