# Prompt Extraction & Project Prompt Library

## Problem

Once a session is launched, there's no way to retrieve and reuse its prompt. Users can't iterate on prompts, debug agent behavior by inspecting prompts, or share prompt configurations.

## Design

### Data Model

Add a `prompts` field to `Project` in `project.json`:

```rust
struct SavedPrompt {
    id: Uuid,
    name: String,           // auto-generated: first ~60 chars of text
    text: String,
    created_at: String,
    source_session_label: String,
}
```

`Project.prompts: Vec<SavedPrompt>` — stored alongside sessions in `project.json`.

### Workflow

**Save prompt (`P`):** Focus on a session, press `P`. The session's `initial_prompt` is saved to `project.prompts[]`. Brief visual confirmation.

- Sessions with no prompt: no-op (ignored silently).
- Sessions with a GitHub issue: save the issue-derived prompt text.
- Duplicates allowed.

**Load prompt (`p`):** Press `p` to open a prompt picker modal (similar to the issue picker). Selecting a prompt creates a new session with this initial prompt:

```
You are a prompt engineer, here is a prompt, your goal is to collaborate with me to make it better:

<prompt>
{saved_prompt_text}
</prompt>
```

### Components

**Backend (Rust):**
- `SavedPrompt` struct in `models.rs`
- `prompts: Vec<SavedPrompt>` field on `Project` (with `#[serde(default)]`)
- `save_session_prompt(project_id, session_id)` — extracts prompt from session, appends to project prompts
- `list_project_prompts(project_id)` — returns project's saved prompts

**Frontend (Svelte):**
- Keybinding: `P` triggers `save_session_prompt` for the focused session
- Keybinding: `p` opens prompt picker modal
- `PromptPicker.svelte` — modal listing saved prompts, selecting one creates a new session with the prompt-engineer prefix

### Not in Scope

- Direct prompt editing in UI (use Claude as prompt engineer instead)
- Deleting saved prompts
- Import/export between projects
