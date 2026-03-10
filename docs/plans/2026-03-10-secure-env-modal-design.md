# Secure Env Modal Design

## Summary

Build a CLI entrypoint that agents can invoke to request secure credential input without exposing the secret in chat, stdout, or shell history. The CLI will ask the already-running The Controller app to open a secure modal, and the app will write the entered value directly into the target project's `.env` file.

## Goals

- Let an agent trigger a human-only credential entry flow from the terminal.
- Restrict writes to `.env` files for projects already known to The Controller.
- Keep secret values out of CLI arguments, stdout, stderr, logs, events, and toast messages.
- Support both updating an existing env var and creating a new one.

## Non-Goals

- Arbitrary filesystem writes outside known Controller projects.
- OS keychain integration.
- Returning secret values back to the CLI caller.
- Managing files other than `<project repo>/.env`.

## Approach Options

### Option 1: CLI delegates to the running Controller app over local IPC

Pros:

- Keeps the secret inside the app-owned modal and backend write path.
- Lets the app validate that the target project is already known.
- Reuses the existing Tauri/Svelte app instead of building a second UI surface.

Cons:

- Requires a local IPC contract between the CLI and the running app.
- Depends on the app already running.

### Option 2: CLI launches a dedicated native window for secret entry

Pros:

- Strong separation from the main app window.

Cons:

- Duplicates modal/UI/backend logic.
- Complicates project validation and app discovery.
- Adds process-management overhead for little user value.

### Option 3: CLI prompts in the terminal with hidden input

Pros:

- Simplest implementation.

Cons:

- Does not meet the product goal of a secure, app-owned input surface.
- Keeps more of the flow inside the agent-invoked terminal boundary.

Recommendation: Option 1.

## Design

### Architecture

Add a CLI command such as `controller env set --project <project> --key <ENV_KEY>` that sends a request to the already-running The Controller app. The app resolves the project from Controller storage, opens a secure modal for the human to enter or edit the value, and writes the result directly into `<project.repo_path>/.env`.

The CLI only receives redacted status metadata such as success, cancel, or failure. It never accepts the secret on stdin or argv and never prints the secret to stdout or stderr.

### Components

1. CLI entrypoint
   - Parses the project selector and env var key.
   - Performs non-secret validation only.
   - Sends a local IPC request to the running app and waits for completion.

2. App-local request service
   - Receives the CLI request.
   - Validates that the project belongs to Controller storage.
   - Rejects unknown projects and concurrent requests when another secret modal is already active.
   - Emits an event that opens the modal in the frontend.

3. Secure modal
   - Displays the project name and env var key.
   - Indicates whether the key already exists.
   - Uses a masked input by default with optional temporary reveal.
   - Sends the value back to the backend through Tauri invoke on submit.
   - Supports explicit cancel.

4. Backend `.env` updater
   - Creates `.env` if missing.
   - Replaces an existing key assignment if present.
   - Appends a new assignment if absent.
   - Preserves unrelated lines and comments as much as practical.
   - Returns only redacted metadata.

### Data Flow

1. Agent runs the CLI command with project selector and env var key.
2. CLI sends a local IPC request to the running app.
3. App resolves the project and opens the secure modal.
4. Human enters the value and chooses Save or Cancel.
5. Backend updates `<project repo>/.env`.
6. CLI exits with a redacted result such as success, cancel, project-not-found, app-not-running, or write-failed.

### Security Constraints

- Secrets exist only in modal state and backend write handling.
- No secret value is included in IPC logs, CLI output, frontend events, or toast messages.
- The caller cannot choose an arbitrary path; the backend always targets `<known project repo>/.env`.
- Invalid or empty env var keys are rejected before opening the modal.
- Only one active secure env request is allowed at a time to avoid result mix-ups.

### Error Handling

- App not running: CLI exits non-zero with a clear message.
- Unknown project: CLI exits non-zero.
- Busy request service: CLI exits non-zero with a retryable error.
- Modal canceled: CLI exits non-zero with a distinct cancel result.
- File write failure: backend returns a generic error that excludes the secret value.

## Testing

- CLI contract tests for success, app-not-running, unknown-project, busy, and cancel cases.
- Backend unit tests for `.env` mutation behavior:
  - replace existing key
  - append missing key
  - create `.env` when absent
  - preserve unrelated lines/comments
  - handle newline edge cases
- Backend tests for project resolution and known-project enforcement.
- Frontend tests for modal open, submit, cancel, and redacted UI messaging.
- End-to-end manual validation with a running app and a real CLI invocation:
  - modal appears when CLI is invoked
  - save updates the project `.env`
  - cancel leaves the file unchanged
  - CLI output remains redacted
