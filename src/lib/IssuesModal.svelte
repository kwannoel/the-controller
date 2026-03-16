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
    allIssues = []; // clear cache so find view refetches after creating
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

      if (!inSearch && e.key === "d" && selectedIssue) {
        e.preventDefault();
        e.stopPropagation();
        const issueToDelete = selectedIssue;
        allIssues = allIssues.filter(i => i.number !== issueToDelete.number);
        command("delete_github_issue", { repoPath, issueNumber: issueToDelete.number });
        return;
      }

      if (!inSearch && e.key === "Enter" && selectedIssue) {
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
              <span class="action-hint"><kbd>d</kbd> delete</span>
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
