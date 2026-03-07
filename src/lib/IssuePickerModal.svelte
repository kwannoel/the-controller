<script lang="ts">
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import type { GithubIssue } from "./stores";

  interface Props {
    repoPath: string;
    onSelect: (issue: GithubIssue) => void;
    onSkip: () => void;
    onClose: () => void;
  }

  let { repoPath, onSelect, onSkip, onClose }: Props = $props();

  let issues: GithubIssue[] = $state([]);
  let loading = $state(true);
  let error: string | null = $state(null);
  let selectedIndex = $state(0);

  // Total items: issues + "No issue" (last)
  let itemCount = $derived(issues.length + 1);

  onMount(() => {
    window.addEventListener("keydown", handleKeydown, { capture: true });

    (async () => {
      try {
        const allIssues = await invoke<GithubIssue[]>("list_github_issues", { repoPath });
        issues = allIssues.filter(issue =>
          !issue.labels.some(l => l.name === "in-progress")
        );
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
    if (selectedIndex === issues.length) {
      onSkip();
    } else {
      onSelect(issues[selectedIndex]);
    }
  }

  function scrollSelectedIntoView() {
    // Wait a tick for Svelte to update the DOM with the new selected class
    requestAnimationFrame(() => {
      const el = document.querySelector('.issue-list .issue-btn.selected');
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

    if (loading || error) return;

    switch (e.key) {
      case "j":
        e.preventDefault();
        e.stopPropagation();
        selectedIndex = (selectedIndex + 1) % itemCount;
        scrollSelectedIntoView();
        break;
      case "k":
        e.preventDefault();
        e.stopPropagation();
        selectedIndex = (selectedIndex - 1 + itemCount) % itemCount;
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
    <div class="modal-header">Assign Issue to New Session</div>
    {#if loading}
      <div class="status">Loading issues...</div>
    {:else if error}
      <div class="status error">{error}</div>
    {:else}
      <ul class="issue-list">
        {#each issues as issue, i}
          <li>
            <button class="issue-btn" class:selected={selectedIndex === i} onclick={() => onSelect(issue)}>
              <span class="issue-number">#{issue.number}</span>
              <span class="issue-title">{issue.title}</span>
            </button>
          </li>
        {/each}
        <li>
          <button class="issue-btn no-issue" class:selected={selectedIndex === issues.length} onclick={onSkip}>
            No issue
          </button>
        </li>
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
    width: 420px;
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
  .issue-list {
    list-style: none;
    margin: 0;
    padding: 0;
    max-height: 50vh;
    overflow-y: auto;
  }
  .issue-list li {
    border-bottom: 1px solid rgba(49, 50, 68, 0.5);
  }
  .issue-list li:last-child {
    border-bottom: none;
  }
  .issue-btn {
    width: 100%;
    display: flex;
    gap: 8px;
    align-items: baseline;
    padding: 10px 8px;
    background: none;
    border: none;
    color: #cdd6f4;
    font-size: 13px;
    cursor: pointer;
    text-align: left;
    box-shadow: none;
  }
  .issue-btn:hover,
  .issue-btn.selected {
    background: #313244;
    border-radius: 4px;
  }
  .issue-number {
    color: #89b4fa;
    font-weight: 500;
    white-space: nowrap;
    flex-shrink: 0;
  }
  .issue-title {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .no-issue {
    color: #6c7086;
  }
  .no-issue.selected,
  .no-issue:hover {
    color: #cdd6f4;
  }
</style>
