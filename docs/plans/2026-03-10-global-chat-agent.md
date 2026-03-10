# Global Chat Agent Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use the-controller-executing-plans to implement this plan task-by-task.

**Goal:** Add a persistent global controller chat that runs a real agent in the focused project context and can update notes through explicit app bridge tools.

**Architecture:** Keep one persistent controller chat session in app state, but execute individual turns through a backend runner that replays transcript and current focus into a headless Codex invocation. Let the agent use normal environment tools directly, and reserve controller-owned mutations for a small notes bridge that emits transcript and UI update events.

**Tech Stack:** Rust, Tauri v2, Svelte 5, Vitest, `codex exec`, existing notes storage and focus stores

---

### Task 1: Add controller chat data models and session state

**Files:**
- Create: `src-tauri/src/controller_chat.rs`
- Modify: `src-tauri/src/state.rs`
- Modify: `src-tauri/src/lib.rs`
- Test: `src-tauri/src/controller_chat.rs`

**Step 1: Write the failing test**

Add Rust unit tests for:
- initializing an empty controller chat session with no project focus
- updating `controller_focus` when a project, session, and note are supplied
- appending transcript items in order and preserving them across turns

Use concrete structs such as:

```rust
#[test]
fn test_controller_focus_updates_note_without_dropping_project() {
    let mut session = ControllerChatSession::default();
    session.update_focus(ControllerFocusUpdate {
        project_id: Some(Uuid::nil()),
        project_name: Some("proj".to_string()),
        session_id: Some(Uuid::nil()),
        note_filename: None,
        workspace_mode: Some("notes".to_string()),
    }).unwrap();

    session.update_focus(ControllerFocusUpdate {
        project_id: None,
        project_name: None,
        session_id: None,
        note_filename: Some("issue-123.md".to_string()),
        workspace_mode: None,
    }).unwrap();

    assert_eq!(session.focus.project_name.as_deref(), Some("proj"));
    assert_eq!(session.focus.note_filename.as_deref(), Some("issue-123.md"));
}
```

**Step 2: Run test to verify it fails**

Run: `cd src-tauri && cargo test controller_chat::tests::test_controller_focus_updates_note_without_dropping_project`
Expected: FAIL because `controller_chat` types do not exist yet

**Step 3: Write minimal implementation**

- Add `ControllerChatSession`, `ControllerFocus`, `ControllerFocusUpdate`, and transcript item structs in `src-tauri/src/controller_chat.rs`
- Store the controller chat session behind a mutex in `AppState`
- Register the new module in `src-tauri/src/lib.rs`

Keep the state model minimal:
- one global session
- focus snapshot
- ordered transcript items
- `turn_in_progress` flag

**Step 4: Run test to verify it passes**

Run: `cd src-tauri && cargo test controller_chat::tests::test_controller_focus_updates_note_without_dropping_project`
Expected: PASS

**Step 5: Commit**

```bash
git add src-tauri/src/controller_chat.rs src-tauri/src/state.rs src-tauri/src/lib.rs
git commit -m "feat: add controller chat session state"
```

### Task 2: Implement the notes bridge and transcript event payloads

**Files:**
- Modify: `src-tauri/src/controller_chat.rs`
- Modify: `src-tauri/src/notes.rs`
- Modify: `src-tauri/src/models.rs`
- Test: `src-tauri/src/controller_chat.rs`

**Step 1: Write the failing test**

Add Rust tests covering:
- `controller.create_note` creates a note inside the focused project notes directory
- `controller.write_note` updates the created note
- `controller.open_note` returns an event payload that includes project id and filename
- invalid filenames are rejected through the bridge

Example test shape:

```rust
#[test]
fn test_execute_create_and_write_note_bridge_action() {
    let tmp = TempDir::new().unwrap();
    let mut session = ControllerChatSession::with_focus(project_focus_for("proj"));

    let results = execute_bridge_actions(
        &tmp.path(),
        &mut session,
        vec![
            BridgeAction::CreateNote { filename: "issue-123.md".to_string() },
            BridgeAction::WriteNote {
                filename: "issue-123.md".to_string(),
                content: "# Issue 123\n".to_string(),
            },
        ],
    ).unwrap();

    assert_eq!(notes::read_note(tmp.path(), "proj", "issue-123.md").unwrap(), "# Issue 123\n");
    assert_eq!(results.len(), 2);
}
```

**Step 2: Run test to verify it fails**

Run: `cd src-tauri && cargo test controller_chat::tests::test_execute_create_and_write_note_bridge_action`
Expected: FAIL because bridge action execution does not exist yet

**Step 3: Write minimal implementation**

- Add serializable bridge action and bridge result enums
- Implement note bridge execution against existing notes helpers
- Update controller focus when a note is created or opened
- Define event payload structs for transcript rows and note-open events

Do not add speculative non-notes bridge tools yet.

**Step 4: Run test to verify it passes**

Run: `cd src-tauri && cargo test controller_chat::tests::test_execute_create_and_write_note_bridge_action`
Expected: PASS

**Step 5: Commit**

```bash
git add src-tauri/src/controller_chat.rs src-tauri/src/notes.rs src-tauri/src/models.rs
git commit -m "feat: add controller chat notes bridge"
```

### Task 3: Add backend commands and the per-turn controller runner

**Files:**
- Modify: `src-tauri/src/controller_chat.rs`
- Modify: `src-tauri/src/commands.rs`
- Modify: `src-tauri/src/lib.rs`
- Modify: `src-tauri/src/bin/server.rs`
- Test: `src-tauri/src/controller_chat.rs`

**Step 1: Write the failing test**

Add Rust tests for:
- starting the controller chat from a focus snapshot
- rejecting a turn when no project focus exists
- parsing a structured controller bridge call from an agent turn result
- appending agent text, bridge action rows, and bridge results to the transcript

Use a runner seam so the tests can substitute a fake agent result without spawning Codex.

**Step 2: Run test to verify it fails**

Run: `cd src-tauri && cargo test controller_chat::tests::test_send_turn_rejects_missing_project_focus`
Expected: FAIL because the controller chat command path does not exist yet

**Step 3: Write minimal implementation**

- Add Tauri commands for:
  - `get_controller_chat_session`
  - `update_controller_chat_focus`
  - `send_controller_chat_message`
- Add matching Axum routes in `src-tauri/src/bin/server.rs`
- Implement a turn runner that:
  - snapshots transcript and focus
  - invokes `codex exec` off the main thread
  - parses assistant text plus controller bridge calls from the response
  - executes bridge calls
  - emits transcript update events

Use a strict response envelope in the agent prompt, for example:

```json
{
  "assistant_message": "Fetched the issue and wrote it into a note.",
  "controller_actions": [
    { "tool": "create_note", "filename": "issue-123.md" },
    { "tool": "write_note", "filename": "issue-123.md", "content": "# Issue 123\n..." },
    { "tool": "open_note", "filename": "issue-123.md" }
  ]
}
```

Keep environment tool usage inside Codex. Only app-owned actions come back through this envelope.

**Step 4: Run test to verify it passes**

Run: `cd src-tauri && cargo test controller_chat::tests::test_send_turn_rejects_missing_project_focus`
Expected: PASS

**Step 5: Run broader backend verification**

Run: `cd src-tauri && cargo test controller_chat`
Expected: PASS

**Step 6: Commit**

```bash
git add src-tauri/src/controller_chat.rs src-tauri/src/commands.rs src-tauri/src/lib.rs src-tauri/src/bin/server.rs
git commit -m "feat: add controller chat turn runner and commands"
```

### Task 4: Build the global chat UI and sync focus while the session is live

**Files:**
- Create: `src/lib/GlobalChat.svelte`
- Create: `src/lib/GlobalChat.test.ts`
- Modify: `src/lib/stores.ts`
- Modify: `src/App.svelte`
- Modify: `src/lib/project-listing.ts`
- Test: `src/lib/GlobalChat.test.ts`

**Step 1: Write the failing test**

Add frontend tests covering:
- rendering the persistent transcript from store state
- submitting a user message through `send_controller_chat_message`
- showing controller bridge result rows distinctly from assistant text
- reacting to a note-open event by updating `activeNote`, `focusTarget`, and the visible note title context

Example assertions:

```ts
it("renders controller bridge activity rows after a chat turn", async () => {
  controllerChatSession.set({
    focus: { project_id: "project-1", project_name: "proj", note_filename: "issue-123.md" },
    items: [
      { id: "1", kind: "user", text: "fetch issue 123" },
      { id: "2", kind: "tool", text: "controller.create_note(issue-123.md)" },
      { id: "3", kind: "assistant", text: "Created the note and opened it." },
    ],
  });

  render(GlobalChat);

  expect(screen.getByText("controller.create_note(issue-123.md)")).toBeInTheDocument();
  expect(screen.getByText("Created the note and opened it.")).toBeInTheDocument();
});
```

**Step 2: Run test to verify it fails**

Run: `npx vitest run src/lib/GlobalChat.test.ts`
Expected: FAIL because `GlobalChat.svelte` and controller chat stores do not exist yet

**Step 3: Write minimal implementation**

- Add controller chat store types in `src/lib/stores.ts`
- Create `GlobalChat.svelte` with:
  - transcript list
  - input box
  - current focus header
  - loading state while a turn is running
- Mount the panel in `src/App.svelte`
- In `App.svelte`, watch `focusTarget` and call `update_controller_chat_focus` while the session is live
- Listen for controller chat transcript events and note-open events, then update `activeNote`, `focusTarget`, and `noteEntries`

Keep the first UI iteration functional and compact. Do not add session-provider controls or non-notes actions.

**Step 4: Run test to verify it passes**

Run: `npx vitest run src/lib/GlobalChat.test.ts`
Expected: PASS

**Step 5: Run broader frontend verification**

Run: `npx vitest run src/App.test.ts src/lib/GlobalChat.test.ts src/lib/NotesEditor.test.ts`
Expected: PASS

**Step 6: Commit**

```bash
git add src/lib/GlobalChat.svelte src/lib/GlobalChat.test.ts src/lib/stores.ts src/App.svelte src/lib/project-listing.ts
git commit -m "feat: add global controller chat UI"
```

### Task 5: Add real-agent validation for controller chat behavior

**Files:**
- Create: `scripts/validate-controller-chat.ts`
- Create: `src/lib/controller-chat-e2e.test.ts`
- Modify: `package.json`
- Test: `scripts/validate-controller-chat.ts`

**Step 1: Write the failing validation harness**

Create a validation entrypoint that follows `the-controller-validating-agent-applications`:
- diagnostics step that checks `codex` is installed and executable
- scenario runner with 3 runs per scenario
- binary PASS/FAIL verdicts

Define at least these scenarios:
- happy path: "fetch GitHub issue 123 and write it to a note"
- multi-turn: "put it in the same note" after a first turn created the note
- error recovery: issue lookup fails and no note is created

Use a real agent run, but provide deterministic external tools by prepending `PATH` with test fixture binaries such as a stub `gh` script that returns fixed output.

**Step 2: Run validation to verify it fails**

Run: `node scripts/validate-controller-chat.ts`
Expected: FAIL because controller chat commands and transcript events are not implemented yet

**Step 3: Write minimal implementation**

- Add the validation harness
- Add lightweight frontend integration coverage if needed to mount the chat and inspect emitted transcript state
- Add an `npm` script such as:

```json
{
  "scripts": {
    "validate:controller-chat": "node scripts/validate-controller-chat.ts"
  }
}
```

Document in the harness why deterministic `gh` fixtures are used while still running the real agent runtime.

**Step 4: Run focused verification**

Run: `node scripts/validate-controller-chat.ts`
Expected: PASS with 3/3 passes per scenario

**Step 5: Run completion verification**

Run: `npx vitest run src/lib/GlobalChat.test.ts src/lib/NotesEditor.test.ts && cd src-tauri && cargo test controller_chat`
Expected: PASS

Run: `npm run validate:controller-chat`
Expected: PASS

This task must follow `the-controller-verification-before-completion` before any completion claim.

**Step 6: Commit**

```bash
git add scripts/validate-controller-chat.ts src/lib/controller-chat-e2e.test.ts package.json
git commit -m "test: add controller chat agent validation"
```
