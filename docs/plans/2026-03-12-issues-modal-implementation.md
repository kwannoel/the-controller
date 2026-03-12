# Issues Modal Revamp — Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use the-controller-executing-plans to implement this plan task-by-task.

**Goal:** Replace 4 scattered issue modals with a single keyboard-driven `IssuesModal` (hub → create | find → assign).

**Architecture:** Single `IssuesModal.svelte` with a `view` state machine (`hub | create | find`). The create flow reuses the existing title → priority → complexity stages. The find flow is a split-pane: filtered list on the left, issue detail on the right. Keybinding, store, and App.svelte wiring updated to match.

**Tech Stack:** Svelte 5 (runes), TypeScript, `@tauri-apps/plugin-opener` for opening URLs, existing `command()` wrapper for Tauri backend calls.

**Design doc:** `docs/plans/2026-03-12-issues-modal-design.md`

---

### Task 1: Update stores — replace old actions with `open-issues-modal`

**Files:**
- Modify: `src/lib/stores.ts:236-278` (HotkeyAction type)

**Step 1: Edit HotkeyAction type**

In `src/lib/stores.ts`, replace these lines in the `HotkeyAction` union:

```typescript
  | { type: "create-issue"; projectId: string; repoPath: string }
  | {
      type: "pick-issue-for-session";
      projectId: string;
      repoPath: string;
      kind?: string;
      background?: boolean;
    }
```
and:
```typescript
  | { type: "toggle-triage-panel"; category?: TriageCategory }
  | { type: "toggle-assigned-issues-panel" }
```

With:
```typescript
  | { type: "open-issues-modal"; projectId: string; repoPath: string }
  | {
      type: "assign-issue-to-session";
      projectId: string;
      repoPath: string;
      issue: GithubIssue;
    }
```

Keep `pick-issue-for-session` for now — it's still used by auto-worker background issue assignment. We'll remove it in a later cleanup if unused.

Actually, check: `pick-issue-for-session` is dispatched from HotkeyManager `create-session` case. Since `c` now creates a session directly without picking, we need to change that dispatch too. But `pick-issue-for-session` is also used from auto-worker with `background: true`. Let's keep it in the union for the auto-worker path, but remove it from the `c` key path.

**Step 2: Remove `TriageCategory` type**

Remove the `TriageCategory` type export:
```typescript
export type TriageCategory = "untriaged" | "triaged";
```

**Step 3: Verify types compile**

Run: `npx tsc --noEmit 2>&1 | head -30`

This will show errors — that's expected. The type changes will cascade. Commit after all tasks are done.

---

### Task 2: Update commands — replace old command IDs with `open-issues-modal`

**Files:**
- Modify: `src/lib/commands.ts`

**Step 1: Replace command definitions**

In the `CommandId` type, remove:
- `"create-issue"`
- `"triage-untriaged"`
- `"triage-triaged"`
- `"assigned-issues"`

Add:
- `"open-issues-modal"`

In the `commands` array, replace:
```typescript
  { id: "create-issue", key: "i", section: "Projects", description: "Create GitHub issue for focused project", mode: "development" },
  { id: "triage-untriaged", key: "t", section: "Projects", description: "Triage issues (untriaged)", mode: "development" },
  { id: "triage-triaged", key: "T", section: "Projects", description: "View triaged issues", mode: "development" },
  { id: "assigned-issues", key: "e", section: "Projects", description: "View assigned but uncompleted issues", mode: "development" },
```

With:
```typescript
  { id: "open-issues-modal", key: "i", section: "Projects", description: "Issues (create, find, assign)", mode: "development" },
```

**Step 2: Update `create-session` description**

Change:
```typescript
  { id: "create-session", key: "c", section: "Sessions", description: "Create session with issue", mode: "development" },
```
To:
```typescript
  { id: "create-session", key: "c", section: "Sessions", description: "Create session", mode: "development" },
```

---

### Task 3: Update HotkeyManager — wire `open-issues-modal`, simplify `create-session`

**Files:**
- Modify: `src/lib/HotkeyManager.svelte`

**Step 1: Replace `create-issue` case with `open-issues-modal`**

Replace the `create-issue` case (lines 345-347):
```typescript
      case "create-issue":
        dispatchCreateIssue();
        return true;
```
With:
```typescript
      case "open-issues-modal": {
        const project = getFocusedProject();
        if (!project) return true;
        dispatchAction({ type: "open-issues-modal", projectId: project.id, repoPath: project.repo_path });
        return true;
      }
```

**Step 2: Change `create-session` to dispatch directly (no issue picker)**

Replace the `create-session` case (lines 292-294):
```typescript
      case "create-session":
        dispatchIssuePicker({ kind: currentSessionProvider });
        return true;
```
With:
```typescript
      case "create-session": {
        const project = getFocusedProject();
        if (!project) return true;
        dispatchAction({ type: "create-session", projectId: project.id, kind: currentSessionProvider });
        return true;
      }
```

**Step 3: Remove dead cases**

Remove:
```typescript
      case "triage-untriaged":
        dispatchAction({ type: "toggle-triage-panel", category: "untriaged" });
        return true;
      case "triage-triaged":
        dispatchAction({ type: "toggle-triage-panel", category: "triaged" });
        return true;
      case "assigned-issues":
        dispatchAction({ type: "toggle-assigned-issues-panel" });
        return true;
```

**Step 4: Remove `dispatchCreateIssue` and `dispatchIssuePicker` functions**

Delete these two functions — they're no longer called:
```typescript
  function dispatchIssuePicker(...) { ... }
  function dispatchCreateIssue() { ... }
```

---

### Task 4: Create `IssuesModal.svelte`

**Files:**
- Create: `src/lib/IssuesModal.svelte`

**Step 1: Write the full component**

```svelte
<script lang="ts">
  import { onMount } from "svelte";
  import { command } from "$lib/backend";
  import { openUrl } from "@tauri-apps/plugin-opener";
  import type { GithubIssue } from "./stores";

  type Priority = "high" | "low";
  type Complexity = "high" | "low";
  type View = "hub" | "create" | "find";
  type CreateStage = "title" | "priority" | "complexity";

  interface Props {
    repoPath: string;
    projectId: string;
    onClose: () => void;
    onCreateIssue: (title: string, priority: Priority, complexity: Complexity) => void;
    onAssignIssue: (issue: GithubIssue) => void;
  }

  let { repoPath, projectId, onClose, onCreateIssue, onAssignIssue }: Props = $props();

  // -- View state machine --
  let view: View = $state("hub");

  // -- Create view state --
  let createStage: CreateStage = $state("title");
  let issueTitle = $state("");
  let selectedPriority: Priority = $state("low");
  let titleInput: HTMLInputElement | undefined = $state();

  // -- Find view state --
  let searchQuery = $state("");
  let allIssues: GithubIssue[] = $state([]);
  let loading = $state(false);
  let error: string | null = $state(null);
  let selectedIndex = $state(0);
  let searchInput: HTMLInputElement | undefined = $state();

  let filteredIssues = $derived.by(() => {
    if (!searchQuery.trim()) return allIssues;
    const q = searchQuery.toLowerCase();
    return allIssues.filter(issue =>
      issue.title.toLowerCase().includes(q) ||
      (issue.body ?? "").toLowerCase().includes(q) ||
      issue.labels.some(l => l.name.toLowerCase().includes(q))
    );
  });

  let selectedIssue: GithubIssue | null = $derived(
    filteredIssues.length > 0 && selectedIndex < filteredIssues.length
      ? filteredIssues[selectedIndex]
      : null
  );

  // -- Overlay ref for focus --
  let overlayEl: HTMLDivElement | undefined = $state();

  function enterCreate() {
    view = "create";
    createStage = "title";
    issueTitle = "";
    selectedPriority = "low";
    requestAnimationFrame(() => titleInput?.focus());
  }

  async function enterFind() {
    view = "find";
    searchQuery = "";
    selectedIndex = 0;
    requestAnimationFrame(() => searchInput?.focus());

    if (allIssues.length === 0) {
      loading = true;
      error = null;
      try {
        allIssues = await command<GithubIssue[]>("list_github_issues", { repoPath });
      } catch (e) {
        error = String(e);
      } finally {
        loading = false;
      }
    }
  }

  function goToHub() {
    view = "hub";
    requestAnimationFrame(() => overlayEl?.focus());
  }

  function scrollSelectedIntoView() {
    requestAnimationFrame(() => {
      const el = document.querySelector(".issues-modal .issue-item.selected");
      el?.scrollIntoView({ block: "nearest" });
    });
  }

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === "Escape") {
      e.preventDefault();
      e.stopPropagation();

      if (view === "create") {
        if (createStage === "complexity") {
          createStage = "priority";
          requestAnimationFrame(() => overlayEl?.focus());
        } else if (createStage === "priority") {
          createStage = "title";
          requestAnimationFrame(() => titleInput?.focus());
        } else {
          goToHub();
        }
      } else if (view === "find") {
        if (searchQuery) {
          searchQuery = "";
          selectedIndex = 0;
        } else {
          goToHub();
        }
      } else {
        onClose();
      }
      return;
    }

    // -- Hub keys --
    if (view === "hub") {
      if (e.key === "c") {
        e.preventDefault();
        e.stopPropagation();
        enterCreate();
      } else if (e.key === "f") {
        e.preventDefault();
        e.stopPropagation();
        enterFind();
      }
      return;
    }

    // -- Create keys --
    if (view === "create") {
      if (createStage === "title" && e.key === "Enter") {
        e.preventDefault();
        if (!issueTitle.trim()) return;
        createStage = "priority";
        requestAnimationFrame(() => overlayEl?.focus());
        return;
      }
      if (createStage === "priority") {
        if (e.key === "j") {
          e.preventDefault();
          selectedPriority = "low";
          createStage = "complexity";
          requestAnimationFrame(() => overlayEl?.focus());
        } else if (e.key === "k") {
          e.preventDefault();
          selectedPriority = "high";
          createStage = "complexity";
          requestAnimationFrame(() => overlayEl?.focus());
        }
        return;
      }
      if (createStage === "complexity") {
        if (e.key === "j") {
          e.preventDefault();
          onCreateIssue(issueTitle.trim(), selectedPriority, "low");
        } else if (e.key === "k") {
          e.preventDefault();
          onCreateIssue(issueTitle.trim(), selectedPriority, "high");
        }
      }
      return;
    }

    // -- Find keys --
    if (view === "find") {
      // Navigation (only when not typing in search input OR using arrow keys)
      const inSearch = document.activeElement === searchInput;

      if (e.key === "ArrowDown" || (!inSearch && e.key === "j")) {
        e.preventDefault();
        e.stopPropagation();
        if (filteredIssues.length > 0) {
          selectedIndex = (selectedIndex + 1) % filteredIssues.length;
          scrollSelectedIntoView();
        }
        return;
      }
      if (e.key === "ArrowUp" || (!inSearch && e.key === "k")) {
        e.preventDefault();
        e.stopPropagation();
        if (filteredIssues.length > 0) {
          selectedIndex = (selectedIndex - 1 + filteredIssues.length) % filteredIssues.length;
          scrollSelectedIntoView();
        }
        return;
      }

      if (!inSearch && e.key === "a" && selectedIssue) {
        e.preventDefault();
        e.stopPropagation();
        onAssignIssue(selectedIssue);
        return;
      }

      if (e.key === "Enter" && selectedIssue) {
        e.preventDefault();
        e.stopPropagation();
        openUrl(selectedIssue.url);
        return;
      }
    }
  }

  // Reset selectedIndex when filtered results change
  $effect(() => {
    if (selectedIndex >= filteredIssues.length) {
      selectedIndex = Math.max(0, filteredIssues.length - 1);
    }
  });

  onMount(() => {
    overlayEl?.focus();
  });
</script>

<!-- svelte-ignore a11y_no_noninteractive_tabindex -->
<div
  class="overlay"
  bind:this={overlayEl}
  tabindex="0"
  onclick={onClose}
  onkeydown={handleKeydown}
  role="dialog"
>
  <!-- svelte-ignore a11y_click_events_have_key_events -->
  <div
    class="issues-modal"
    class:wide={view === "find"}
    onclick={(e) => e.stopPropagation()}
    role="presentation"
  >
    {#if view === "hub"}
      <div class="modal-header">Issues</div>
      <div class="hub-menu">
        <button class="hub-option" onclick={enterCreate}>
          <span class="hub-key">c</span>
          <span>Create issue</span>
        </button>
        <button class="hub-option" onclick={enterFind}>
          <span class="hub-key">f</span>
          <span>Find issues</span>
        </button>
      </div>
      <div class="hint">Press Esc to close</div>

    {:else if view === "create"}
      <div class="modal-header">New Issue</div>
      {#if createStage === "title"}
        <input
          bind:this={titleInput}
          bind:value={issueTitle}
          placeholder="Issue title"
          class="input"
        />
        <div class="hint">Press Enter to continue</div>
      {:else if createStage === "priority"}
        <div class="title-preview">{issueTitle}</div>
        <div class="option-row">
          <span class="option-key low">j</span> Low Priority
          <span class="option-key high">k</span> High Priority
        </div>
        <div class="hint">Press Esc to go back</div>
      {:else}
        <div class="title-preview">{issueTitle}</div>
        <div class="selected-badge {selectedPriority}">{selectedPriority} priority</div>
        <div class="option-row">
          <span class="option-key simple">j</span> Low Complexity
          <span class="option-key complex">k</span> High Complexity
        </div>
        <div class="hint">Press Esc to go back</div>
      {/if}

    {:else if view === "find"}
      <div class="find-layout">
        <div class="find-left">
          <input
            bind:this={searchInput}
            bind:value={searchQuery}
            placeholder="Search issues..."
            class="input"
          />
          {#if loading}
            <div class="status">Loading issues...</div>
          {:else if error}
            <div class="status error">{error}</div>
          {:else if filteredIssues.length === 0}
            <div class="status">No issues found</div>
          {:else}
            <ul class="issue-list">
              {#each filteredIssues as issue, i}
                <li>
                  <button
                    class="issue-item"
                    class:selected={i === selectedIndex}
                    onclick={() => { selectedIndex = i; }}
                  >
                    <span class="issue-number">#{issue.number}</span>
                    <span class="issue-title">{issue.title}</span>
                  </button>
                </li>
              {/each}
            </ul>
          {/if}
        </div>
        <div class="find-right">
          {#if selectedIssue}
            <div class="detail-number">#{selectedIssue.number}</div>
            <div class="detail-title">{selectedIssue.title}</div>
            {#if selectedIssue.labels.length > 0}
              <div class="detail-labels">
                {#each selectedIssue.labels as label}
                  <span class="detail-label">{label.name}</span>
                {/each}
              </div>
            {/if}
            {#if selectedIssue.body}
              <div class="detail-body">{selectedIssue.body}</div>
            {/if}
            <div class="detail-actions">
              <span class="action-hint"><kbd>a</kbd> assign to session</span>
              <span class="action-hint"><kbd>Enter</kbd> open in browser</span>
            </div>
          {:else}
            <div class="status">Select an issue</div>
          {/if}
        </div>
      </div>
      <div class="hint">
        <kbd>j/k</kbd> navigate &middot; <kbd>Esc</kbd> {searchQuery ? "clear search" : "back"}
      </div>
    {/if}
  </div>
</div>

<style>
  .overlay {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.7);
    backdrop-filter: blur(16px);
    display: flex;
    align-items: flex-start;
    justify-content: center;
    padding-top: 15vh;
    z-index: 100;
    outline: none;
  }

  .issues-modal {
    background: var(--bg-elevated);
    border: 1px solid var(--border-default);
    border-radius: 8px;
    width: 380px;
    padding: 24px;
    display: flex;
    flex-direction: column;
    gap: 12px;
    box-shadow: 0 8px 32px rgba(0, 0, 0, 0.6);
    transition: width 0.15s ease;
  }

  .issues-modal.wide {
    width: 720px;
  }

  .modal-header {
    font-size: 16px;
    font-weight: 600;
    color: var(--text-emphasis);
  }

  /* -- Hub -- */
  .hub-menu {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .hub-option {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 10px 12px;
    background: none;
    border: 1px solid transparent;
    border-radius: 6px;
    color: var(--text-primary);
    font-size: 14px;
    cursor: pointer;
    text-align: left;
    box-shadow: none;
  }

  .hub-option:hover {
    background: var(--bg-hover);
    border-color: var(--border-default);
  }

  .hub-key {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 24px;
    height: 24px;
    background: var(--bg-hover);
    border: 1px solid var(--border-default);
    border-radius: 4px;
    font-size: 13px;
    font-weight: 600;
    color: var(--text-emphasis);
  }

  /* -- Shared -- */
  .input {
    background: var(--bg-hover);
    color: var(--text-primary);
    border: 1px solid var(--border-default);
    padding: 10px 12px;
    border-radius: 6px;
    font-size: 14px;
    outline: none;
    width: 100%;
    box-sizing: border-box;
  }

  .input:focus {
    border-color: var(--text-emphasis);
  }

  .hint {
    color: var(--text-secondary);
    font-size: 12px;
    text-align: center;
  }

  .hint kbd {
    background: var(--bg-hover);
    padding: 1px 5px;
    border-radius: 3px;
    font-size: 11px;
    color: var(--text-primary);
  }

  .status {
    color: var(--text-secondary);
    font-size: 13px;
    padding: 16px;
    text-align: center;
  }

  .status.error {
    color: var(--status-error);
  }

  /* -- Create view -- */
  .title-preview {
    color: var(--text-primary);
    font-size: 14px;
    padding: 10px 12px;
    background: var(--bg-hover);
    border-radius: 6px;
  }

  .option-row {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 20px;
    font-size: 15px;
    color: var(--text-primary);
    padding: 8px 0;
  }

  .option-key {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 24px;
    height: 24px;
    background: var(--bg-hover);
    border: 1px solid var(--border-default);
    border-radius: 4px;
    font-size: 13px;
    font-weight: 600;
    margin-right: 4px;
  }

  .option-key.high {
    color: var(--status-error);
    border-color: var(--status-error);
  }

  .option-key.low {
    color: var(--status-idle);
    border-color: var(--status-idle);
  }

  .option-key.simple {
    color: var(--text-emphasis);
    border-color: var(--text-emphasis);
  }

  .option-key.complex {
    color: var(--status-working);
    border-color: var(--status-working);
  }

  .selected-badge {
    font-size: 12px;
    padding: 4px 10px;
    border-radius: 4px;
    text-align: center;
    text-transform: capitalize;
  }

  .selected-badge.high {
    color: var(--status-error);
    background: rgba(196, 64, 64, 0.1);
  }

  .selected-badge.low {
    color: var(--status-idle);
    background: rgba(74, 158, 110, 0.1);
  }

  /* -- Find view -- */
  .find-layout {
    display: flex;
    gap: 16px;
    min-height: 300px;
    max-height: 55vh;
  }

  .find-left {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .find-right {
    width: 300px;
    flex-shrink: 0;
    display: flex;
    flex-direction: column;
    gap: 8px;
    overflow-y: auto;
    border-left: 1px solid var(--border-default);
    padding-left: 16px;
  }

  .issue-list {
    list-style: none;
    margin: 0;
    padding: 0;
    overflow-y: auto;
    flex: 1;
  }

  .issue-list li {
    border-bottom: 1px solid var(--border-default);
  }

  .issue-list li:last-child {
    border-bottom: none;
  }

  .issue-item {
    width: 100%;
    display: flex;
    gap: 8px;
    align-items: center;
    padding: 8px;
    background: none;
    border: none;
    color: var(--text-primary);
    font-size: 13px;
    cursor: pointer;
    text-align: left;
    box-shadow: none;
  }

  .issue-item:hover,
  .issue-item.selected {
    background: var(--bg-hover);
    border-radius: 4px;
  }

  .issue-number {
    color: var(--text-emphasis);
    font-weight: 500;
    white-space: nowrap;
    flex-shrink: 0;
  }

  .issue-title {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  /* -- Detail pane -- */
  .detail-number {
    color: var(--text-emphasis);
    font-size: 14px;
    font-weight: 600;
  }

  .detail-title {
    color: var(--text-primary);
    font-size: 15px;
    line-height: 1.4;
  }

  .detail-labels {
    display: flex;
    flex-wrap: wrap;
    gap: 4px;
  }

  .detail-label {
    font-size: 11px;
    color: var(--text-primary);
    background: var(--bg-hover);
    padding: 2px 8px;
    border-radius: 4px;
  }

  .detail-body {
    color: var(--text-secondary);
    font-size: 13px;
    line-height: 1.5;
    white-space: pre-wrap;
    overflow-wrap: break-word;
    word-break: break-word;
    border-top: 1px solid var(--border-default);
    padding-top: 8px;
    flex: 1;
    overflow-y: auto;
  }

  .detail-actions {
    display: flex;
    flex-direction: column;
    gap: 4px;
    margin-top: auto;
    padding-top: 8px;
    border-top: 1px solid var(--border-default);
  }

  .action-hint {
    color: var(--text-secondary);
    font-size: 12px;
  }

  .action-hint kbd {
    background: var(--bg-hover);
    padding: 1px 5px;
    border-radius: 3px;
    font-size: 11px;
    color: var(--text-primary);
  }
</style>
```

---

### Task 5: Update App.svelte — wire IssuesModal, remove old modals

**Files:**
- Modify: `src/App.svelte`

**Step 1: Update imports**

Replace:
```typescript
  import CreateIssueModal from "./lib/CreateIssueModal.svelte";
  import IssuePickerModal from "./lib/IssuePickerModal.svelte";
```
and:
```typescript
  import TriagePanel from "./lib/TriagePanel.svelte";
  import AssignedIssuesPanel from "./lib/AssignedIssuesPanel.svelte";
```

With:
```typescript
  import IssuesModal from "./lib/IssuesModal.svelte";
```

Remove `TriageCategory` from the stores import if it's there.

**Step 2: Replace state variables**

Replace:
```typescript
  let createIssueTarget: { projectId: string; repoPath: string } | null = $state(null);
  let issuePickerTarget: { projectId: string; repoPath: string; kind?: string; background?: boolean } | null = $state(null);
  let triagePanelOpen: TriageCategory | null = $state(null);
  let assignedIssuesPanelOpen = $state(false);
```

With:
```typescript
  let issuesModalTarget: { projectId: string; repoPath: string } | null = $state(null);
```

**Step 3: Update hotkey action handler**

In the `$effect` that subscribes to `hotkeyAction`, replace:
```typescript
      } else if (action?.type === "create-issue") {
        createIssueTarget = { projectId: action.projectId, repoPath: action.repoPath };
      } else if (action?.type === "pick-issue-for-session") {
        issuePickerTarget = { projectId: action.projectId, repoPath: action.repoPath, kind: action.kind, background: action.background };
```

With:
```typescript
      } else if (action?.type === "open-issues-modal") {
        issuesModalTarget = { projectId: action.projectId, repoPath: action.repoPath };
      } else if (action?.type === "assign-issue-to-session") {
        createSessionWithIssue(action.projectId, action.repoPath, action.issue);
```

Remove:
```typescript
      } else if (action?.type === "toggle-triage-panel") {
        if (action.category) {
          triagePanelOpen = triagePanelOpen ? null : action.category;
        }
      } else if (action?.type === "toggle-assigned-issues-panel") {
        assignedIssuesPanelOpen = !assignedIssuesPanelOpen;
```

Keep `pick-issue-for-session` handling — the auto-worker dispatches it with `background: true`.

**Step 4: Keep `handleIssueSubmit` but update the closing logic**

Change the first line of `handleIssueSubmit`:
```typescript
    const repoPath = createIssueTarget!.repoPath;
    createIssueTarget = null;
```
To:
```typescript
    const repoPath = issuesModalTarget!.repoPath;
    issuesModalTarget = null;
```

**Step 5: Remove `handleIssuePicked` and `handleIssuePickerSkip`**

Delete these functions — no longer needed since the IssuesModal's `onAssignIssue` directly triggers `assign-issue-to-session`.

Actually, keep `handleIssuePicked` renamed or inline it for the `pick-issue-for-session` auto-worker path. Wait — that path still uses `issuePickerTarget`. Let's think about this:

The auto-worker path dispatches `pick-issue-for-session` with `background: true`. This currently opens `IssuePickerModal`. Since we're removing that modal, we need the auto-worker to work differently. But looking at the code, the auto-worker dispatches this from `HotkeyManager` only via the `c` key. The actual auto-worker background sessions are created differently (in auto_worker.rs on the backend). So `pick-issue-for-session` with `background: true` is only used when the user manually triggers it.

Let's simplify: remove `pick-issue-for-session` from HotkeyManager entirely. If it's needed for the auto-worker, we handle it in the `$effect`. But since `c` now creates sessions directly, there's no frontend path that dispatches `pick-issue-for-session` anymore except perhaps from AgentDashboard. Let's check — no, AgentDashboard only dispatches `open-issue-in-browser`. So we can safely remove `pick-issue-for-session` from the hotkey action union and the `$effect` handler, along with `handleIssuePicked` and `handleIssuePickerSkip`.

Delete:
- `handleIssuePicked` function
- `handleIssuePickerSkip` function
- The `pick-issue-for-session` case in the `$effect`

And in `stores.ts`, remove `pick-issue-for-session` from `HotkeyAction`.

**Step 6: Add `handleAssignIssue` function**

Add this function (near the other handler functions):
```typescript
  function handleAssignIssue(issue: GithubIssue) {
    const target = issuesModalTarget!;
    issuesModalTarget = null;
    createSessionWithIssue(target.projectId, target.repoPath, issue);
  }
```

**Step 7: Update template**

Replace:
```svelte
    {#if createIssueTarget}
      <CreateIssueModal
        onSubmit={handleIssueSubmit}
        onClose={() => { createIssueTarget = null; }}
      />
    {/if}
    {#if issuePickerTarget}
      <IssuePickerModal
        repoPath={issuePickerTarget.repoPath}
        onSelect={handleIssuePicked}
        onSkip={handleIssuePickerSkip}
        onClose={() => { issuePickerTarget = null; }}
      />
    {/if}
```
and:
```svelte
    {#if triagePanelOpen}
      <TriagePanel category={triagePanelOpen} onClose={() => { triagePanelOpen = null; }} />
    {/if}
    {#if assignedIssuesPanelOpen}
      <AssignedIssuesPanel onClose={() => { assignedIssuesPanelOpen = false; }} />
    {/if}
```

With:
```svelte
    {#if issuesModalTarget}
      <IssuesModal
        repoPath={issuesModalTarget.repoPath}
        projectId={issuesModalTarget.projectId}
        onClose={() => { issuesModalTarget = null; }}
        onCreateIssue={handleIssueSubmit}
        onAssignIssue={handleAssignIssue}
      />
    {/if}
```

---

### Task 6: Delete old component files

**Files:**
- Delete: `src/lib/CreateIssueModal.svelte`
- Delete: `src/lib/IssuePickerModal.svelte`
- Delete: `src/lib/TriagePanel.svelte`
- Delete: `src/lib/AssignedIssuesPanel.svelte`
- Delete: `src/lib/TriagePanel.test.ts`
- Delete: `src/test/CreateIssueModalMock.svelte`
- Delete: `src/test/IssuePickerModalMock.svelte`

Run:
```bash
rm src/lib/CreateIssueModal.svelte \
   src/lib/IssuePickerModal.svelte \
   src/lib/TriagePanel.svelte \
   src/lib/AssignedIssuesPanel.svelte \
   src/lib/TriagePanel.test.ts \
   src/test/CreateIssueModalMock.svelte \
   src/test/IssuePickerModalMock.svelte
```

---

### Task 7: Update App.test.ts — fix mocks and test the new flow

**Files:**
- Modify: `src/App.test.ts`

**Step 1: Replace old modal mocks**

Remove:
```typescript
vi.mock("./lib/CreateIssueModal.svelte", async () => ({
  default: (await import("./test/CreateIssueModalMock.svelte")).default,
}));
vi.mock("./lib/IssuePickerModal.svelte", async () => ({
  default: (await import("./test/IssuePickerModalMock.svelte")).default,
}));
```

Add mock for IssuesModal:
```typescript
vi.mock("./lib/IssuesModal.svelte", async () => ({
  default: (await import("./test/IssuesModalMock.svelte")).default,
}));
```

**Step 2: Create IssuesModalMock**

Create `src/test/IssuesModalMock.svelte`:
```svelte
<script lang="ts">
  import type { GithubIssue } from "../lib/stores";

  type Priority = "high" | "low";
  type Complexity = "high" | "low";

  let {
    onCreateIssue,
    onAssignIssue,
    onClose,
  }: {
    repoPath: string;
    projectId: string;
    onCreateIssue: (title: string, priority: Priority, complexity: Complexity) => void;
    onAssignIssue: (issue: GithubIssue) => void;
    onClose: () => void;
  } = $props();

  const mockIssue: GithubIssue = {
    number: 42,
    title: "Mock issue",
    url: "https://example.com/issues/42",
    body: "Mock issue body",
    labels: [],
  };
</script>

<div role="dialog">
  <button type="button" data-testid="mock-create-issue-submit" onclick={() => onCreateIssue("Mock issue", "low", "low")}>
    Submit issue
  </button>
  <button type="button" data-testid="mock-issue-assign" onclick={() => onAssignIssue(mockIssue)}>
    Assign issue
  </button>
  <button type="button" data-testid="mock-issues-close" onclick={onClose}>Close</button>
</div>
```

**Step 3: Update "App issue creation flow" test**

Change the hotkey action dispatch from:
```typescript
    hotkeyAction.set({
      type: "create-issue",
      projectId: "proj-1",
      repoPath: "/tmp/the-controller",
    });
```
To:
```typescript
    hotkeyAction.set({
      type: "open-issues-modal",
      projectId: "proj-1",
      repoPath: "/tmp/the-controller",
    });
```

**Step 4: Update "App issue picker flow" test**

This test dispatches `pick-issue-for-session` which no longer opens a picker. Replace the test to use `assign-issue-to-session`:

```typescript
describe("App issue assign flow", () => {
  // ... same beforeEach ...

  it("creates a session when assigning an issue from the issues modal", async () => {
    render(App);

    hotkeyAction.set({
      type: "open-issues-modal",
      projectId: "proj-1",
      repoPath: "/tmp/the-controller",
    });

    await fireEvent.click(await screen.findByTestId("mock-issue-assign"));

    await waitFor(() => {
      expect(command).toHaveBeenCalledWith("create_session", expect.objectContaining({
        projectId: "proj-1",
        githubIssue: expect.objectContaining({ number: 42 }),
      }));
    });
  });
});
```

Remove the old "creates background issue sessions with codex even when the requested kind is claude" test — that flow no longer exists via the UI.

**Step 5: Run tests**

Run: `npx vitest run`

All tests should pass. Fix any failures.

---

### Task 8: Final verification and commit

**Step 1: Type check**

Run: `npx tsc --noEmit`

Fix any remaining type errors.

**Step 2: Run all tests**

Run: `npx vitest run`

**Step 3: Manual smoke test**

Run: `npm run tauri dev`

Verify:
- `i` opens the issues modal in hub view
- `c` in the modal goes to create view (title → priority → complexity)
- Escape walks back through create stages to hub
- `f` in the modal goes to find view with search input focused
- Typing filters issues by title/body/labels
- `j/k` and arrow keys navigate the list, detail pane updates
- `a` on a selected issue creates a session
- `Enter` opens the issue in the browser
- Escape in find view clears search first, then goes to hub
- `c` in the sidebar creates a session directly (no issue picker)

**Step 4: Commit**

```bash
git add -A
git commit -m "feat: consolidate issues into single keyboard-driven modal

Replace CreateIssueModal, IssuePickerModal, TriagePanel, and
AssignedIssuesPanel with a unified IssuesModal (i key).
Hub view offers create (c) and find (f) sub-views.
The c key in sidebar now creates sessions directly."
```
