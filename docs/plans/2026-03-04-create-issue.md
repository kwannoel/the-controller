# Create GitHub Issue via `i` Key Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add `i` keybinding that opens a title-only modal, generates an issue body via Claude CLI, creates the issue via `gh`, and optimistically inserts it into the task panel.

**Architecture:** New `create_github_issue` Tauri command chains three steps: extract GitHub remote (reuse `extract_github_repo`), generate body via `claude --print`, create issue via `gh issue create`. Frontend dispatches `create-issue` action from `i` key, `App.svelte` hosts the modal, and `TaskPanel` exposes a method to optimistically insert the new issue.

**Tech Stack:** Rust (tokio::process::Command for `claude` and `gh`), Svelte 5 (runes), Catppuccin Mocha theme.

---

### Task 1: Add `create_github_issue` Tauri command (Rust)

**Files:**
- Modify: `src-tauri/src/commands.rs` (add command before `#[cfg(test)]` at line 745)
- Modify: `src-tauri/src/lib.rs:46` (register new command)

**Step 1: Write the command**

In `src-tauri/src/commands.rs`, add before the `#[cfg(test)]` block:

```rust
#[tauri::command]
pub async fn create_github_issue(
    repo_path: String,
    title: String,
) -> Result<crate::models::GithubIssue, String> {
    // Step 1: Extract GitHub owner/repo
    let repo_path_clone = repo_path.clone();
    let nwo = tokio::task::spawn_blocking(move || extract_github_repo(&repo_path_clone))
        .await
        .map_err(|e| format!("Task failed: {}", e))??;

    // Step 2: Generate issue body via Claude CLI
    let prompt = format!(
        "Write a concise GitHub issue body for an issue titled: \"{}\". \
         Include a Summary section and a Details section. \
         Keep it under 200 words. Return only the markdown body, nothing else.",
        title
    );
    let body_output = tokio::process::Command::new("claude")
        .args(["--print", &prompt])
        .env_remove("CLAUDECODE")
        .output()
        .await
        .map_err(|e| format!("Failed to run claude: {}", e))?;

    let body = if body_output.status.success() {
        String::from_utf8_lossy(&body_output.stdout).trim().to_string()
    } else {
        // Fallback: create issue without body if Claude fails
        String::new()
    };

    // Step 3: Create the issue via gh CLI
    let output = tokio::process::Command::new("gh")
        .args([
            "issue", "create",
            "--repo", &nwo,
            "--title", &title,
            "--body", &body,
            "--json", "number,title,url,labels",
        ])
        .output()
        .await
        .map_err(|e| format!("Failed to run gh: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("gh issue create failed: {}", stderr));
    }

    let issue: crate::models::GithubIssue =
        serde_json::from_slice(&output.stdout)
            .map_err(|e| format!("Failed to parse gh output: {}", e))?;

    Ok(issue)
}
```

**Step 2: Register the command**

In `src-tauri/src/lib.rs`, add `commands::create_github_issue,` after `commands::list_github_issues,` (line 47).

**Step 3: Run tests**

Run: `cd src-tauri && cargo test`
Expected: All existing tests pass (no new unit tests needed — the command is a composition of already-tested `extract_github_repo` + external CLI calls).

**Step 4: Commit**

```bash
git add src-tauri/src/commands.rs src-tauri/src/lib.rs
git commit -m "feat: add create_github_issue Tauri command (#21)"
```

---

### Task 2: Add `create-issue` hotkey action, `i` keybinding, and help entry

**Files:**
- Modify: `src/lib/stores.ts:31-44` (add action type)
- Modify: `src/lib/HotkeyManager.svelte:287-376` (add `i` case)
- Modify: `src/lib/HotkeyHelp.svelte:10-25` (add help entry)

**Step 1: Add action type**

In `src/lib/stores.ts`, add to the `HotkeyAction` union (after `"toggle-archive-view"` on line 43):

```typescript
  | { type: "create-issue"; projectId: string; repoPath: string }
```

**Step 2: Add `i` keybinding**

In `src/lib/HotkeyManager.svelte`, add a new case in `handleHotkey` before the `case "t":` line (before line 350):

```typescript
      case "i":
        if (currentFocus?.type === "project" || currentFocus?.type === "session") {
          const project = projectList.find(p => p.id === currentFocus.projectId);
          if (project) {
            dispatchAction({ type: "create-issue", projectId: project.id, repoPath: project.repo_path });
          }
        }
        return true;
```

**Step 3: Add help entry**

In `src/lib/HotkeyHelp.svelte`, add after the `t` / "Toggle GitHub issues panel" entry (after line 22):

```typescript
    { key: "i", description: "Create GitHub issue for focused project" },
```

**Step 4: Run frontend tests**

Run: `npx vitest run`
Expected: All existing tests pass.

**Step 5: Commit**

```bash
git add src/lib/stores.ts src/lib/HotkeyManager.svelte src/lib/HotkeyHelp.svelte
git commit -m "feat: add create-issue hotkey action and i keybinding (#21)"
```

---

### Task 3: Create `CreateIssueModal.svelte` component

**Files:**
- Create: `src/lib/CreateIssueModal.svelte`

**Step 1: Create the component**

Model it after `NewProjectModal.svelte` — same overlay/modal pattern, single input, Catppuccin Mocha styling.

```svelte
<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { onMount } from "svelte";
  import { showToast } from "./toast";

  interface GithubIssue {
    number: number;
    title: string;
    url: string;
    labels: { name: string }[];
  }

  interface Props {
    repoPath: string;
    onCreated: (issue: GithubIssue) => void;
    onClose: () => void;
  }

  let { repoPath, onCreated, onClose }: Props = $props();

  let title = $state("");
  let loading = $state(false);
  let titleInput: HTMLInputElement | undefined = $state();

  onMount(() => {
    titleInput?.focus();
  });

  async function create() {
    if (!title.trim() || loading) return;
    loading = true;
    try {
      const issue = await invoke<GithubIssue>("create_github_issue", {
        repoPath,
        title: title.trim(),
      });
      onCreated(issue);
    } catch (e) {
      showToast(String(e), "error");
      loading = false;
    }
  }

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === "Escape") {
      e.preventDefault();
      e.stopPropagation();
      onClose();
    } else if (e.key === "Enter") {
      e.preventDefault();
      create();
    }
  }
</script>

<div class="overlay" onclick={onClose} onkeydown={handleKeydown} role="dialog">
  <!-- svelte-ignore a11y_click_events_have_key_events -->
  <div class="modal" onclick={(e) => e.stopPropagation()} role="presentation">
    <div class="modal-header">New GitHub Issue</div>
    <input
      bind:this={titleInput}
      bind:value={title}
      placeholder="Issue title"
      class="input"
      disabled={loading}
    />
    <button
      class="btn-primary"
      onclick={create}
      disabled={!title.trim() || loading}
    >
      {loading ? "Creating..." : "Create Issue"}
    </button>
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
    width: 380px;
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
  .input {
    background: #313244;
    color: #cdd6f4;
    border: 1px solid #45475a;
    padding: 10px 12px;
    border-radius: 6px;
    font-size: 14px;
    outline: none;
    width: 100%;
    box-sizing: border-box;
  }
  .input:focus {
    border-color: #89b4fa;
  }
  .btn-primary {
    background: #89b4fa;
    color: #1e1e2e;
    border: none;
    padding: 10px;
    border-radius: 6px;
    font-size: 14px;
    font-weight: 600;
    cursor: pointer;
  }
  .btn-primary:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }
</style>
```

**Step 2: Commit**

```bash
git add src/lib/CreateIssueModal.svelte
git commit -m "feat: create CreateIssueModal component (#21)"
```

---

### Task 4: Wire modal and optimistic insert into App.svelte and TaskPanel

**Files:**
- Modify: `src/App.svelte` (host modal, handle action, wire to TaskPanel)
- Modify: `src/lib/TaskPanel.svelte` (expose `insertIssue` function, accept `onRef` prop)

**Step 1: Add optimistic insert to TaskPanel**

In `src/lib/TaskPanel.svelte`, export a function that the parent can call. Change the script section:

Add after the `Props` interface (after line 7):

```typescript
  export function insertIssue(issue: GithubIssue) {
    // Optimistic insert at the top
    issues = [issue, ...issues];
  }
```

**Step 2: Wire App.svelte**

In `src/App.svelte`:

1. Add imports:

```typescript
  import CreateIssueModal from "./lib/CreateIssueModal.svelte";
  import { appConfig, onboardingComplete, hotkeyAction, showKeyHints, sidebarVisible, taskPanelVisible, focusTarget, projects, type Config, type FocusTarget, type Project } from "./lib/stores";
```

(Replace the existing stores import on line 11 to include `focusTarget`, `projects`.)

2. Add state variables (after `taskPanelIsVisible` on line 17):

```typescript
  let createIssueTarget: { projectId: string; repoPath: string } | null = $state(null);
  let taskPanelRef: { insertIssue: (issue: any) => void } | undefined = $state();
  let projectList: Project[] = $state([]);
  let currentFocus: FocusTarget = $state(null);
```

3. Add store subscriptions (after the `taskPanelVisible` effect ending around line 32):

```typescript
  $effect(() => {
    const unsub = projects.subscribe((v) => { projectList = v; });
    return unsub;
  });

  $effect(() => {
    const unsub = focusTarget.subscribe((v) => { currentFocus = v; });
    return unsub;
  });
```

4. Extend the `hotkeyAction` subscriber (around line 35) to handle `create-issue`:

```typescript
  $effect(() => {
    const unsub = hotkeyAction.subscribe((action) => {
      if (action?.type === "toggle-help") {
        showKeyHints.update((v) => !v);
      } else if (action?.type === "create-issue") {
        createIssueTarget = { projectId: action.projectId, repoPath: action.repoPath };
      }
    });
    return unsub;
  });
```

5. Add handler function:

```typescript
  function handleIssueCreated(issue: any) {
    createIssueTarget = null;
    // Open task panel and optimistically insert the issue
    taskPanelVisible.set(true);
    // Small delay to let TaskPanel mount if it wasn't visible
    setTimeout(() => {
      taskPanelRef?.insertIssue(issue);
    }, 50);
  }
```

6. In the template, add `bind:this` on TaskPanel and the modal:

Change the TaskPanel line to:

```svelte
      {#if taskPanelIsVisible}
        <TaskPanel visible={taskPanelIsVisible} bind:this={taskPanelRef} />
      {/if}
```

Add the modal (after `HotkeyHelp`, before the closing `{/if}` for `needsOnboarding`):

```svelte
    {#if createIssueTarget}
      <CreateIssueModal
        repoPath={createIssueTarget.repoPath}
        onCreated={handleIssueCreated}
        onClose={() => { createIssueTarget = null; }}
      />
    {/if}
```

**Step 3: Run frontend tests**

Run: `npx vitest run`
Expected: All existing tests pass.

**Step 4: Commit**

```bash
git add src/App.svelte src/lib/TaskPanel.svelte
git commit -m "feat: wire CreateIssueModal and optimistic insert (#21)"
```

---

### Task 5: Final verification

**Step 1: Run all tests**

```bash
cd src-tauri && cargo test
npx vitest run
```

Expected: All tests pass.

**Step 2: Manual smoke test**

Run: `npm run tauri dev`
1. Focus a project that has a GitHub remote
2. Press `i` — modal appears with title input
3. Type a title, press Enter — loading state shows
4. Issue gets created, task panel opens with the new issue at the top
5. Press `i` with no focus — nothing happens
6. Press `?` — help shows `i` shortcut
7. Press Escape in the modal — modal closes without creating

**Step 3: Final commit (if any cleanup needed)**
