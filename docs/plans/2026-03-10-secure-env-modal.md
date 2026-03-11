# Secure Env Modal Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use the-controller-executing-plans to implement this plan task-by-task.

**Goal:** Add a CLI tool that asks the running The Controller app to open a secure modal for creating or updating a known project's `.env` variable, then writes the value directly without exposing the secret to the agent.

**Architecture:** Add a small Rust CLI binary that talks to the running app over a dedicated local Unix socket. The app owns request validation, modal orchestration, and `.env` mutation; the CLI only sends non-secret metadata and receives redacted completion status. The frontend renders a new secure modal and submits the secret back through Tauri invoke so the value never returns to the CLI.

**Tech Stack:** Rust (`tauri`, `tokio`, Unix domain sockets), Svelte 5, Vitest, Cargo tests.

---

### Task 1: Add failing `.env` mutation tests and the backend helper

**Files:**
- Create: `src-tauri/src/secure_env.rs`
- Modify: `src-tauri/src/lib.rs`
- Modify: `src-tauri/Cargo.toml`
- Test: `src-tauri/src/secure_env.rs`

**Step 1: Write the failing tests**

Add Rust unit tests in `src-tauri/src/secure_env.rs` for:
- replacing an existing `OPENAI_API_KEY=old` entry with a new value
- appending `OPENAI_API_KEY=new` when the key is missing
- creating `.env` when it does not exist
- preserving unrelated lines and comments
- rejecting invalid env keys such as empty strings or keys containing `=`

Each test should use a temp directory and assert on final file contents, for example:

```rust
#[test]
fn updates_existing_env_key_without_touching_other_lines() {
    let tmp = tempfile::TempDir::new().unwrap();
    let env_path = tmp.path().join(".env");
    std::fs::write(&env_path, "# comment\nOPENAI_API_KEY=old\nFOO=bar\n").unwrap();

    update_env_file(&env_path, "OPENAI_API_KEY", "new-secret").unwrap();

    let updated = std::fs::read_to_string(&env_path).unwrap();
    assert_eq!(updated, "# comment\nOPENAI_API_KEY=new-secret\nFOO=bar\n");
}
```

**Step 2: Run test to verify it fails**

Run: `cd src-tauri && cargo test secure_env::tests::updates_existing_env_key_without_touching_other_lines`

Expected: FAIL because `secure_env`/`update_env_file` does not exist yet.

**Step 3: Write minimal implementation**

In `src-tauri/src/secure_env.rs`:
- add `pub(crate) fn validate_env_key(key: &str) -> Result<(), String>`
- add `pub(crate) fn update_env_file(env_path: &Path, key: &str, value: &str) -> Result<EnvWriteResult, String>`
- implement line-preserving replace-or-append behavior
- create `.env` when missing
- keep the helper pure and independent from Tauri/AppState so it is cheap to test

In `src-tauri/src/lib.rs`:
- add `pub mod secure_env;`

**Step 4: Run test to verify it passes**

Run: `cd src-tauri && cargo test secure_env::tests`

Expected: PASS for the new helper tests.

**Step 5: Commit**

```bash
git add src-tauri/src/secure_env.rs src-tauri/src/lib.rs docs/plans/2026-03-10-secure-env-modal.md
git commit -m "test: add secure env file mutation coverage"
```

### Task 2: Add failing IPC parsing tests for secure env requests

**Files:**
- Modify: `src-tauri/src/secure_env.rs`
- Test: `src-tauri/src/secure_env.rs`

**Step 1: Write the failing tests**

Add tests for local IPC message parsing and response serialization, for example:
- parse `set|<project>|OPENAI_API_KEY|<request-id>` into a typed request
- reject malformed messages
- serialize `ok|updated|<request-id>`
- serialize `error|busy|<request-id>`

Use focused tests around a pure parser/formatter API rather than socket integration.

**Step 2: Run test to verify it fails**

Run: `cd src-tauri && cargo test secure_env::tests::parses_secure_env_request_message`

Expected: FAIL because the parser/response helpers do not exist yet.

**Step 3: Write minimal implementation**

In `src-tauri/src/secure_env.rs`:
- add request/response structs for the socket protocol
- add pure parse/format helpers that the socket listener will reuse
- keep the protocol redacted; no field should ever contain secret values

**Step 4: Run test to verify it passes**

Run: `cd src-tauri && cargo test secure_env::tests`

Expected: PASS for both the mutation tests and protocol tests.

**Step 5: Commit**

```bash
git add src-tauri/src/secure_env.rs
git commit -m "test: cover secure env socket protocol"
```

### Task 3: Add failing backend state tests for request lifecycle

**Files:**
- Modify: `src-tauri/src/state.rs`
- Modify: `src-tauri/src/secure_env.rs`
- Test: `src-tauri/src/secure_env.rs`

**Step 1: Write the failing tests**

Add backend tests that exercise request orchestration without the UI:
- known project resolves to `<repo_path>/.env`
- unknown project is rejected
- only one active request can exist at a time
- cancel clears active request state
- complete writes the file and clears active request state

Use `Storage::new(tempdir)` and a test `AppState::from_storage(...)` where practical.

**Step 2: Run test to verify it fails**

Run: `cd src-tauri && cargo test secure_env::tests::rejects_unknown_project_for_secure_env_request`

Expected: FAIL because there is no active-request coordinator yet.

**Step 3: Write minimal implementation**

In `src-tauri/src/state.rs`:
- add secure-env request state guarded by `Mutex`, storing the active request id plus target metadata needed for completion

In `src-tauri/src/secure_env.rs`:
- add app-facing helpers such as:
  - `begin_secure_env_request(...)`
  - `cancel_secure_env_request(...)`
  - `submit_secure_env_value(...)`
- ensure they validate project membership through storage and use the tested `.env` helper

**Step 4: Run test to verify it passes**

Run: `cd src-tauri && cargo test secure_env::tests`

Expected: PASS with lifecycle coverage.

**Step 5: Commit**

```bash
git add src-tauri/src/state.rs src-tauri/src/secure_env.rs
git commit -m "feat: add secure env request coordinator"
```

### Task 4: Add failing socket listener tests and hook it into app startup

**Files:**
- Modify: `src-tauri/src/status_socket.rs`
- Modify: `src-tauri/src/lib.rs`
- Modify: `src-tauri/src/secure_env.rs`
- Test: `src-tauri/src/status_socket.rs`

**Step 1: Write the failing tests**

Add socket-layer tests for:
- detecting secure-env request messages separately from existing session-status messages
- routing a valid request to the secure-env coordinator
- rejecting malformed or busy requests with redacted error responses

Keep these tests focused on the dispatcher function; do not require a real GUI window.

**Step 2: Run test to verify it fails**

Run: `cd src-tauri && cargo test status_socket::tests::parses_secure_env_message`

Expected: FAIL because the dispatcher only understands status hook messages today.

**Step 3: Write minimal implementation**

In `src-tauri/src/status_socket.rs`:
- extend the local socket listener to recognize a second message family for secure env requests
- emit a frontend event such as `secure-env-requested` with redacted metadata
- send the CLI a redacted result after submit/cancel/failure
- keep the existing session-status behavior unchanged

In `src-tauri/src/lib.rs`:
- continue starting the listener during setup; no extra startup thread should be needed if the current socket service can handle both protocols

**Step 4: Run test to verify it passes**

Run: `cd src-tauri && cargo test status_socket::tests`

Expected: PASS for both existing status-socket tests and new secure-env routing coverage.

**Step 5: Commit**

```bash
git add src-tauri/src/status_socket.rs src-tauri/src/lib.rs src-tauri/src/secure_env.rs
git commit -m "feat: route secure env requests over local socket"
```

### Task 5: Add failing Tauri command tests for submit/cancel behavior

**Files:**
- Modify: `src-tauri/src/commands.rs`
- Modify: `src-tauri/src/lib.rs`
- Modify: `src-tauri/src/secure_env.rs`
- Test: `src-tauri/src/secure_env.rs`

**Step 1: Write the failing tests**

Add tests covering the backend commands that the modal will call:
- submit writes the value and returns redacted metadata
- cancel clears the pending request
- submit rejects mismatched or unknown request ids

Use pure functions or thin wrappers that are testable without invoking a real Tauri window.

**Step 2: Run test to verify it fails**

Run: `cd src-tauri && cargo test secure_env::tests::submit_secure_env_request_writes_env_file`

Expected: FAIL because the command helpers do not exist yet.

**Step 3: Write minimal implementation**

In `src-tauri/src/commands.rs`:
- add async commands such as:
  - `submit_secure_env_value`
  - `cancel_secure_env_request`
- register them in `src-tauri/src/lib.rs`
- use `tokio::task::spawn_blocking` around file I/O if needed

**Step 4: Run test to verify it passes**

Run: `cd src-tauri && cargo test secure_env::tests`

Expected: PASS with command-path coverage.

**Step 5: Commit**

```bash
git add src-tauri/src/commands.rs src-tauri/src/lib.rs src-tauri/src/secure_env.rs
git commit -m "feat: add secure env backend commands"
```

### Task 6: Add failing frontend tests for the secure env modal

**Files:**
- Create: `src/lib/SecureEnvModal.svelte`
- Create: `src/lib/SecureEnvModal.test.ts`
- Modify: `src/App.svelte`
- Modify: `src/lib/backend.ts`

**Step 1: Write the failing tests**

In `src/lib/SecureEnvModal.test.ts`, add tests for:
- rendering project/key metadata from a request payload
- masking the value input by default
- calling `command("submit_secure_env_value", ...)` on save
- calling `command("cancel_secure_env_request", ...)` on cancel
- never showing the raw secret in toast text

Use the same testing style as the existing Svelte component tests with mocked backend calls.

**Step 2: Run test to verify it fails**

Run: `npx vitest run src/lib/SecureEnvModal.test.ts`

Expected: FAIL because the component and backend command calls do not exist yet.

**Step 3: Write minimal implementation**

In `src/lib/SecureEnvModal.svelte`:
- build the modal using the existing overlay/modal styling conventions
- focus the password input on mount
- keep the input masked by default
- provide Save and Cancel actions only

In `src/App.svelte`:
- listen for the `secure-env-requested` backend event
- track pending modal state
- render `<SecureEnvModal ... />`
- on submit/cancel, call the new backend commands and clear modal state

In `src/lib/backend.ts`:
- no protocol change may be needed, but add or adjust typed helpers if that keeps the modal code clean

**Step 4: Run test to verify it passes**

Run: `npx vitest run src/lib/SecureEnvModal.test.ts src/lib/backend.test.ts`

Expected: PASS.

**Step 5: Commit**

```bash
git add src/lib/SecureEnvModal.svelte src/lib/SecureEnvModal.test.ts src/App.svelte src/lib/backend.ts
git commit -m "feat: add secure env modal UI"
```

### Task 7: Add failing CLI tests and implement the CLI binary

**Files:**
- Modify: `src-tauri/Cargo.toml`
- Create: `src-tauri/src/bin/controller-cli.rs`
- Modify: `src-tauri/src/secure_env.rs`
- Test: `src-tauri/src/bin/controller-cli.rs`

**Step 1: Write the failing tests**

Add CLI-focused tests covering:
- `controller-cli env set --project demo --key OPENAI_API_KEY` sends the correct redacted request
- app-not-running returns a non-zero exit
- cancel returns a distinct non-zero exit
- success prints only redacted metadata, never the secret

Prefer testing a pure `run(args, io)` helper so argument parsing and socket behavior are testable without spawning a subprocess.

**Step 2: Run test to verify it fails**

Run: `cd src-tauri && cargo test controller_cli`

Expected: FAIL because the binary/helper does not exist yet.

**Step 3: Write minimal implementation**

In `src-tauri/Cargo.toml`:
- add a new `[[bin]]` entry for the CLI tool

In `src-tauri/src/bin/controller-cli.rs`:
- parse `env set --project <selector> --key <ENV_KEY>`
- connect to the app's local socket
- send the redacted request and block for the response
- print only success/failure metadata
- return distinct exit codes for usage error, app-not-running, cancel, and write failure

If useful, extract shared socket helpers into `src-tauri/src/secure_env.rs` so the app and CLI do not duplicate protocol formatting.

**Step 4: Run test to verify it passes**

Run: `cd src-tauri && cargo test controller_cli`

Expected: PASS.

**Step 5: Commit**

```bash
git add src-tauri/Cargo.toml src-tauri/src/bin/controller-cli.rs src-tauri/src/secure_env.rs
git commit -m "feat: add secure env CLI"
```

### Task 8: Verify the integrated flow end-to-end and document usage

**Files:**
- Modify: `README.md`
- Optional docs: `docs/demo.md`

**Step 1: Write the failing verification target**

Before changing docs, define the manual verification checklist:
- start the app
- run the CLI against a known project
- confirm the modal appears
- save a secret and verify only the `.env` file changes
- re-run and cancel, verify no file change
- verify the CLI output stays redacted

**Step 2: Run verification before final cleanup**

Run:
- `cd src-tauri && cargo test secure_env::tests status_socket::tests controller_cli`
- `npx vitest run src/lib/SecureEnvModal.test.ts src/lib/backend.test.ts`
- manual app + CLI flow

Expected: all automated tests pass, and the manual flow confirms the modal/write contract.

**Step 3: Write minimal documentation**

Update `README.md` with:
- CLI command shape
- requirement that the app is already running
- known-project-only restriction
- redacted result contract

**Step 4: Re-run verification**

Run the same automated commands after the doc update if any code moved during cleanup.

**Step 5: Commit**

```bash
git add README.md docs/demo.md
git commit -m "docs: document secure env CLI flow"
```
