# Global Chat Agent Design

## Definition

Add a global AI chat surface that can operate on the currently focused controller context and perform real work in the project environment. The first shipped behavior should focus on notes workflows, including requests like fetching a GitHub issue and writing it into a note or opening the affected note in the editor.

## Constraints

- Scope v1 to the focused project or session rather than cross-project targeting.
- The chat should behave like a general agent, not a narrow intent parser or proposal-only planner.
- Environment tools may run without confirmation.
- The app must expose explicit bridge operations for app-owned state, starting with notes.
- The chat session may stay live across panel open and close; it should not be recreated for every interaction.
- While the chat session is live, focus changes in the controller should update the agent's working context.
- Notes support should be fully implemented, not stubbed behind speculative abstractions for future tools.
- Any slow backend work must stay off the Tauri main thread.
- Treat this as an agent application change and require real end-to-end agent validation before calling it complete.

## Approaches

### 1. Intent-specific planner with approved operations

Translate prompts into a constrained plan such as `fetch_issue -> create_note -> write_note` and require explicit user confirmation before execution.

Pros: Easy to validate, low-risk mutation model.
Cons: Not the product requested. It behaves like a planner, not a general agent, and it blocks future generalization behind a second redesign.

### 2. Persistent controller chat session with a general agent plus app bridge

Maintain one global chat session for the app. Each user turn runs a real agent in the focused project context with normal environment tools and a controller-specific bridge for app-owned actions such as notes.

Pros: Matches the requested behavior, keeps the agent flexible, and still gives the app a clear control boundary for UI/editor mutations.
Cons: Requires new orchestration, transcript management, and tool result reporting.

### 3. Hidden terminal session masquerading as chat

Spawn a normal CLI session in the background and treat its transcript as chat.

Pros: Reuses existing PTY and tmux machinery.
Cons: Tool interception and structured app actions become awkward, transcript quality is worse, and the UX is much harder to control.

## Chosen Design

Use approach 2. The app will own a persistent global controller chat session and run a real backend agent for each turn. The agent may use environment tools freely, but app-owned mutations go through an explicit controller bridge so the app can keep notes state, focus, and transcript reporting coherent.

## Runtime Model

The controller chat is a persistent app session, not a throwaway prompt box. Opening the chat panel attaches to the existing session. Closing or hiding the panel does not destroy the session by default.

For v1, the persistent session is logical state rather than a permanently busy subprocess. The backend stores transcript, controller focus, and pending turn state. Each user turn invokes the agent against the accumulated transcript and current focus snapshot, then appends the result back into the session. This keeps the chat continuously available without depending on a long-lived interactive terminal protocol.

Use Codex as the initial controller-agent runtime because the backend already uses `codex exec` for headless agentic work. Provider selection for the global chat is out of scope for this iteration.

## Focus Model

The controller chat owns a `controller_focus` record:

- `project_id` and project metadata are required once the session is initialized
- `session_id` is optional and sticky
- `note_filename` is optional and sticky
- `workspace_mode` is an optional hint

The first live focus snapshot seeds `controller_focus`. After that, the app updates it whenever controller focus changes while the chat session is live. The agent receives both the current focus snapshot and the sticky controller focus on each turn. This lets commands such as "put that in the same note" resolve correctly even if the user has moved focus around since the last note action.

## App Bridge

The agent may use any normal environment tools available in the repo context. App-specific actions must go through explicit controller bridge operations so the app can update notes state and focus predictably.

V1 bridge operations:

- `controller.get_focus`
- `controller.list_notes`
- `controller.read_note`
- `controller.create_note`
- `controller.write_note`
- `controller.open_note`

These are real backend operations, not placeholders. The notes bridge should reuse the existing notes storage rules and filename validation. `open_note` should update the app's active note and focus state, not just mutate files on disk.

## Agent/Bridge Protocol

The backend turn runner should give the agent:

- recent chat transcript
- focused project/session/note context
- the controller bridge contract
- an instruction that environment tools are available directly, but controller-owned note actions must be emitted through the bridge protocol

The agent turn result should include:

- assistant text for the user-facing chat
- zero or more controller bridge tool calls
- structured tool results for any controller actions the backend executes

The backend validates and executes controller bridge calls in order, appends tool results into the transcript, and emits UI events for transcript updates and note/focus changes.

## Transcript And Reporting

The chat transcript should render four item types:

- user messages
- assistant messages
- environment/tool activity summaries where available
- controller bridge action rows with explicit outcomes

When the agent changes app state, the transcript should say what happened in plain language. Example: "Fetched GitHub issue #123, created `issue-123.md`, wrote the issue details, and opened the note."

This transcript is the user's audit trail. The app must not silently perform note mutations without a visible record in the same chat.

## UI Integration

Add a dedicated global chat panel at the app level rather than tying the feature to an individual terminal session. The panel should:

- show the persistent transcript
- accept user input while the session is idle
- render controller action rows distinctly from normal assistant text
- indicate the current project/session/note focus driving the chat
- remain connected to the same session when hidden and reopened

The chat panel should not replace the existing notes workspace. Instead, note-opening and note-writing bridge actions should drive the existing notes UI through store updates or emitted events.

## Errors

Environment-tool failures stay with the agent turn. If `gh` fails, the agent can recover or explain the failure. Controller bridge failures should be surfaced as explicit tool failures in the transcript. The backend must not silently swallow bridge errors.

If no focused project exists when the session is first initialized, the chat should show a clear empty state rather than creating an unscoped agent session.

## Validation

- Add failing backend tests for controller focus updates, transcript persistence, and notes bridge execution against project-scoped note storage.
- Add failing frontend tests for rendering the global chat transcript and reflecting note-open or note-write results in the existing notes UI.
- Add a real agent validation harness that runs a controller chat scenario end-to-end with the actual agent runtime and controller bridge.
- Cover at least happy-path, multi-turn memory, and error-recovery scenarios for the controller chat validation suite.
- Run the real-agent scenarios at least three times per scenario before calling the feature complete.
- If the controller bridge integration is reverted, the end-to-end validation must fail.
