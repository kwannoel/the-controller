# Prompt Extraction & Project Prompt Library — Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use the-controller-executing-plans to implement this plan task-by-task.

**Goal:** Allow users to save session prompts to a project-level library (`P`) and load them into new sessions as prompt-engineering conversations (`p`).

**Architecture:** Two new Tauri commands (`save_session_prompt`, `list_project_prompts`) backed by a `prompts` field on the `Project` model. Frontend adds two hotkeys to the command registry, a `PromptPickerModal` (cloned from `IssuePickerModal` pattern), and wiring in `App.svelte` / `HotkeyManager.svelte`.

**Tech Stack:** Rust (Tauri v2), Svelte 5, existing command registry + hotkey action pattern.

---

### Task 1: Add `SavedPrompt` model and `prompts` field to `Project`

**Files:**
- Modify: `src-tauri/src/models.rs`

**Step 1: Add `SavedPrompt` struct and update `Project`**

In `src-tauri/src/models.rs`, add after the `GithubLabel` struct (after line 107):

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SavedPrompt {
    pub id: Uuid,
    pub name: String,
    pub text: String,
    pub created_at: String,
    pub source_session_label: String,
}
```

Add to the `Project` struct (after `auto_worker` field, line 15):

```rust
    #[serde(default)]
    pub prompts: Vec<SavedPrompt>,
```

**Step 2: Update all `Project` construction sites in tests**

Every test that constructs a `Project` needs `prompts: vec![]`. Search for `Project {` in:
- `src-tauri/src/models.rs` (tests)
- `src-tauri/src/storage.rs` (tests — the `make_project` helper)
- `src-tauri/src/commands.rs` (if any)

**Step 3: Run Rust tests**

Run: `cd src-tauri && cargo test`
Expected: All tests pass (no compilation errors from missing field).

**Step 4: Commit**

```bash
git add src-tauri/src/models.rs src-tauri/src/storage.rs
git commit -m "feat: add SavedPrompt model and prompts field to Project"
```

---

### Task 2: Add `save_session_prompt` Tauri command

**Files:**
- Modify: `src-tauri/src/commands.rs`
- Modify: `src-tauri/src/lib.rs`

**Step 1: Write the command**

Add to `src-tauri/src/commands.rs` (after `set_initial_prompt`, around line 576):

```rust
#[tauri::command]
pub fn save_session_prompt(
    state: State<AppState>,
    project_id: String,
    session_id: String,
) -> Result<(), String> {
    let project_uuid = Uuid::parse_str(&project_id).map_err(|e| e.to_string())?;
    let session_uuid = Uuid::parse_str(&session_id).map_err(|e| e.to_string())?;

    let storage = state.storage.lock().map_err(|e| e.to_string())?;
    let mut project = storage
        .load_project(project_uuid)
        .map_err(|e| e.to_string())?;

    let session = project
        .sessions
        .iter()
        .find(|s| s.id == session_uuid)
        .ok_or_else(|| "Session not found".to_string())?;

    // Build prompt text: use initial_prompt, or derive from github_issue
    let prompt_text = session
        .initial_prompt
        .clone()
        .or_else(|| {
            session.github_issue.as_ref().map(|issue| {
                crate::session_args::build_issue_prompt(
                    issue.number,
                    &issue.title,
                    &issue.url,
                    false,
                )
            })
        })
        .ok_or_else(|| "Session has no prompt to save".to_string())?;

    // Auto-generate name: first ~60 chars
    let name = if prompt_text.len() > 60 {
        format!("{}...", &prompt_text[..60])
    } else {
        prompt_text.clone()
    };

    let saved = crate::models::SavedPrompt {
        id: Uuid::new_v4(),
        name,
        text: prompt_text,
        created_at: chrono::Utc::now().to_rfc3339(),
        source_session_label: session.label.clone(),
    };

    project.prompts.push(saved);
    storage.save_project(&project).map_err(|e| e.to_string())?;

    Ok(())
}
```

**Note:** Check if `chrono` is already a dependency. If not, add it to `src-tauri/Cargo.toml`:

```toml
chrono = { version = "0.4", features = ["serde"] }
```

If chrono is not available, use a simpler approach:
```rust
    let created_at = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs().to_string())
        .unwrap_or_default();
```

**Step 2: Register in `lib.rs`**

Add to the `invoke_handler` list in `src-tauri/src/lib.rs` (after `commands::configure_auto_worker`, line 75):

```rust
            commands::save_session_prompt,
```

**Step 3: Run Rust tests**

Run: `cd src-tauri && cargo test`
Expected: All tests pass.

**Step 4: Commit**

```bash
git add src-tauri/src/commands.rs src-tauri/src/lib.rs src-tauri/Cargo.toml
git commit -m "feat: add save_session_prompt Tauri command"
```

---

### Task 3: Add `list_project_prompts` Tauri command

**Files:**
- Modify: `src-tauri/src/commands.rs`
- Modify: `src-tauri/src/lib.rs`

**Step 1: Write the command**

Add to `src-tauri/src/commands.rs` (after `save_session_prompt`):

```rust
#[tauri::command]
pub fn list_project_prompts(
    state: State<AppState>,
    project_id: String,
) -> Result<Vec<crate::models::SavedPrompt>, String> {
    let project_uuid = Uuid::parse_str(&project_id).map_err(|e| e.to_string())?;
    let storage = state.storage.lock().map_err(|e| e.to_string())?;
    let project = storage
        .load_project(project_uuid)
        .map_err(|e| e.to_string())?;
    Ok(project.prompts)
}
```

**Step 2: Register in `lib.rs`**

Add to the `invoke_handler` list in `src-tauri/src/lib.rs`:

```rust
            commands::list_project_prompts,
```

**Step 3: Run Rust tests**

Run: `cd src-tauri && cargo test`
Expected: All tests pass.

**Step 4: Commit**

```bash
git add src-tauri/src/commands.rs src-tauri/src/lib.rs
git commit -m "feat: add list_project_prompts Tauri command"
```

---

### Task 4: Add frontend types and hotkey action types

**Files:**
- Modify: `src/lib/stores.ts`

**Step 1: Add `SavedPrompt` interface**

Add after `AutoWorkerConfig` (after line 36):

```typescript
export interface SavedPrompt {
  id: string;
  name: string;
  text: string;
  created_at: string;
  source_session_label: string;
}
```

**Step 2: Add `prompts` to `Project` interface**

In the `Project` interface (line 58-67), add after `auto_worker`:

```typescript
  prompts: SavedPrompt[];
```

**Step 3: Add hotkey action types**

Add to the `HotkeyAction` union type (after `"clear-maintainer-reports"`, line 117):

```typescript
  | { type: "save-session-prompt"; sessionId: string; projectId: string }
  | { type: "pick-prompt-for-session"; projectId: string }
```

**Step 4: Run frontend tests**

Run: `npx vitest run`
Expected: All tests pass (type-only changes shouldn't break anything).

**Step 5: Commit**

```bash
git add src/lib/stores.ts
git commit -m "feat: add SavedPrompt type and prompt hotkey actions"
```

---

### Task 5: Add command registry entries and hotkey handler

**Files:**
- Modify: `src/lib/commands.ts`
- Modify: `src/lib/HotkeyManager.svelte`

**Step 1: Add command IDs**

In `src/lib/commands.ts`, add to the `CommandId` type (after `"clear-agent-reports"`, line 31):

```typescript
  | "save-prompt"
  | "load-prompt"
```

**Step 2: Add command definitions**

Add to the `commands` array in the Sessions section (after `finish-branch`, line 71):

```typescript
  { id: "save-prompt", key: "P", section: "Sessions", description: "Save focused session's prompt", mode: "development" },
  { id: "load-prompt", key: "p", section: "Sessions", description: "Load saved prompt into new session", mode: "development" },
```

**Step 3: Add handler cases in `HotkeyManager.svelte`**

In `handleHotkey()` (after the `finish-branch` case, around line 362), add:

```typescript
      case "save-prompt": {
        if (currentFocus?.type === "session") {
          dispatchAction({
            type: "save-session-prompt",
            sessionId: currentFocus.sessionId,
            projectId: currentFocus.projectId,
          });
        }
        return true;
      }
      case "load-prompt": {
        const project = getFocusedProject();
        if (project) {
          dispatchAction({ type: "pick-prompt-for-session", projectId: project.id });
        }
        return true;
      }
```

**Step 4: Run frontend tests**

Run: `npx vitest run`
Expected: All tests pass.

**Step 5: Commit**

```bash
git add src/lib/commands.ts src/lib/HotkeyManager.svelte
git commit -m "feat: add save-prompt and load-prompt hotkeys"
```

---

### Task 6: Create `PromptPickerModal.svelte`

**Files:**
- Create: `src/lib/PromptPickerModal.svelte`

**Step 1: Create the modal component**

Clone the `IssuePickerModal` pattern. This component:
- Fetches prompts via `invoke("list_project_prompts", { projectId })`
- Lists them with `j`/`k` navigation, `l`/`Enter` to select, `Escape` to close
- Shows prompt name (truncated text) and source session label
- Calls `onSelect(prompt)` when a prompt is chosen

```svelte
<script lang="ts">
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import type { SavedPrompt } from "./stores";

  interface Props {
    projectId: string;
    onSelect: (prompt: SavedPrompt) => void;
    onClose: () => void;
  }

  let { projectId, onSelect, onClose }: Props = $props();

  let prompts: SavedPrompt[] = $state([]);
  let loading = $state(true);
  let error: string | null = $state(null);
  let selectedIndex = $state(0);

  onMount(() => {
    window.addEventListener("keydown", handleKeydown, { capture: true });

    (async () => {
      try {
        prompts = await invoke<SavedPrompt[]>("list_project_prompts", { projectId });
      } catch (e) {
        error = String(e);
      } finally {
        loading = false;
      }
    })();

    return () => {
      window.removeEventListener("keydown", handleKeydown, { capture: true });
    };
  });

  function confirm() {
    if (prompts.length > 0) {
      onSelect(prompts[selectedIndex]);
    }
  }

  function scrollSelectedIntoView() {
    requestAnimationFrame(() => {
      const el = document.querySelector('.prompt-list .prompt-btn.selected');
      el?.scrollIntoView({ block: 'nearest' });
    });
  }

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === "Escape") {
      e.preventDefault();
      e.stopPropagation();
      onClose();
      return;
    }

    if (loading || error || prompts.length === 0) return;

    switch (e.key) {
      case "j":
        e.preventDefault();
        e.stopPropagation();
        selectedIndex = (selectedIndex + 1) % prompts.length;
        scrollSelectedIntoView();
        break;
      case "k":
        e.preventDefault();
        e.stopPropagation();
        selectedIndex = (selectedIndex - 1 + prompts.length) % prompts.length;
        scrollSelectedIntoView();
        break;
      case "l":
      case "Enter":
        e.preventDefault();
        e.stopPropagation();
        confirm();
        break;
    }
  }
</script>

<div class="overlay" onclick={onClose} role="dialog">
  <!-- svelte-ignore a11y_click_events_have_key_events -->
  <div class="modal" onclick={(e) => e.stopPropagation()} role="presentation">
    <div class="modal-header">Load Saved Prompt</div>
    {#if loading}
      <div class="status">Loading prompts...</div>
    {:else if error}
      <div class="status error">{error}</div>
    {:else if prompts.length === 0}
      <div class="status">No saved prompts</div>
    {:else}
      <ul class="prompt-list">
        {#each prompts as prompt, index (prompt.id)}
          <li>
            <button class="prompt-btn" class:selected={selectedIndex === index} onclick={() => onSelect(prompt)}>
              <span class="prompt-source">{prompt.source_session_label}</span>
              <span class="prompt-name">{prompt.name}</span>
            </button>
          </li>
        {/each}
      </ul>
    {/if}
  </div>
</div>

<style>
  .overlay {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.6);
    display: flex;
    align-items: flex-start;
    justify-content: center;
    padding-top: 20vh;
    z-index: 100;
  }
  .modal {
    background: #1e1e2e;
    border: 1px solid #313244;
    border-radius: 8px;
    width: 480px;
    padding: 24px;
    display: flex;
    flex-direction: column;
    gap: 12px;
  }
  .modal-header {
    font-size: 16px;
    font-weight: 600;
    color: #cdd6f4;
  }
  .status {
    color: #6c7086;
    font-size: 13px;
  }
  .status.error {
    color: #f38ba8;
  }
  .prompt-list {
    list-style: none;
    margin: 0;
    padding: 0;
    max-height: 50vh;
    overflow-y: auto;
  }
  .prompt-list li {
    border-bottom: 1px solid rgba(49, 50, 68, 0.5);
  }
  .prompt-list li:last-child {
    border-bottom: none;
  }
  .prompt-btn {
    width: 100%;
    display: flex;
    gap: 8px;
    align-items: center;
    padding: 10px 8px;
    background: none;
    border: none;
    color: #cdd6f4;
    font-size: 13px;
    cursor: pointer;
    text-align: left;
    box-shadow: none;
  }
  .prompt-btn:hover,
  .prompt-btn.selected {
    background: #313244;
    border-radius: 4px;
  }
  .prompt-source {
    color: #89b4fa;
    font-weight: 500;
    white-space: nowrap;
    flex-shrink: 0;
    font-size: 11px;
  }
  .prompt-name {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
</style>
```

**Step 2: Run frontend tests**

Run: `npx vitest run`
Expected: All tests pass.

**Step 3: Commit**

```bash
git add src/lib/PromptPickerModal.svelte
git commit -m "feat: add PromptPickerModal component"
```

---

### Task 7: Wire up App.svelte — save prompt action + prompt picker modal + session creation

**Files:**
- Modify: `src/App.svelte`

**Step 1: Import PromptPickerModal and SavedPrompt**

Add to imports (after `IssuePickerModal` import, line 14):

```typescript
import PromptPickerModal from "./lib/PromptPickerModal.svelte";
```

Add `SavedPrompt` to the stores import (line 20):

```typescript
import { ..., type SavedPrompt } from "./lib/stores";
```

**Step 2: Add state variable**

Add after `triagePanelOpen` (line 24):

```typescript
let promptPickerTarget: { projectId: string } | null = $state(null);
```

**Step 3: Subscribe to hotkey actions**

Add to the `$effect` subscription (after `toggle-triage-panel` handler, around line 53):

```typescript
      } else if (action?.type === "save-session-prompt") {
        saveSessionPrompt(action.projectId, action.sessionId);
      } else if (action?.type === "pick-prompt-for-session") {
        promptPickerTarget = { projectId: action.projectId };
      }
```

**Step 4: Add `saveSessionPrompt` handler**

Add after `toggleAutoWorkerEnabled` (around line 100):

```typescript
  async function saveSessionPrompt(projectId: string, sessionId: string) {
    try {
      await invoke("save_session_prompt", { projectId, sessionId });
      showToast("Prompt saved", "info");
    } catch (e) {
      showToast(String(e), "error");
    }
  }
```

**Step 5: Add `handlePromptPicked` handler**

Add after `handleIssuePickerSkip` (around line 149):

```typescript
  async function handlePromptPicked(prompt: SavedPrompt) {
    const target = promptPickerTarget!;
    promptPickerTarget = null;

    const wrappedPrompt = `You are a prompt engineer, here is a prompt, your goal is to collaborate with me to make it better:\n\n<prompt>\n${prompt.text}\n</prompt>`;

    try {
      const sessionId: string = await invoke("create_session", {
        projectId: target.projectId,
        kind: "claude",
        initialPrompt: wrappedPrompt,
      });
      await activateNewSession(sessionId, target.projectId);
      focusTerminalSoon();
    } catch (e) {
      showToast(String(e), "error");
    }
  }
```

**Step 6: Render PromptPickerModal**

Add after the `IssuePickerModal` block (after line 282):

```svelte
    {#if promptPickerTarget}
      <PromptPickerModal
        projectId={promptPickerTarget.projectId}
        onSelect={handlePromptPicked}
        onClose={() => { promptPickerTarget = null; }}
      />
    {/if}
```

**Step 7: Run frontend tests**

Run: `npx vitest run`
Expected: All tests pass.

**Step 8: Commit**

```bash
git add src/App.svelte
git commit -m "feat: wire prompt save/load into App.svelte"
```

---

### Task 8: Manual integration test

**Step 1: Start the app**

Run: `npm run tauri dev`

**Step 2: Test save prompt (`P`)**

1. Create a session with an issue (press `c`, pick an issue)
2. Navigate focus to the session in the sidebar (press `Escape` to leave terminal, use `j`/`k`)
3. Press `P`
4. Verify toast says "Prompt saved"

**Step 3: Test load prompt (`p`)**

1. Navigate to the same project
2. Press `p`
3. Verify the PromptPickerModal opens showing the saved prompt
4. Use `j`/`k` to navigate, `l` or `Enter` to select
5. Verify a new session is created with the prompt-engineer prefix
6. Verify the terminal shows Claude working on the prompt

**Step 4: Test edge cases**

- Press `P` on a session with no prompt — should do nothing (toast: "Session has no prompt to save")
- Press `p` on a project with no saved prompts — modal shows "No saved prompts"
- Press `Escape` in the prompt picker — modal closes

**Step 5: Final commit (if any fixes needed)**

```bash
git add -A
git commit -m "fix: address integration test findings"
```
