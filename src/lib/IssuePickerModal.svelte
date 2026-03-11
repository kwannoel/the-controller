<script lang="ts">
  import { onMount } from "svelte";
  import { command } from "$lib/backend";
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

  type IssueGroup = { label: string; items: { issue: GithubIssue; index: number }[] };

  let groups: IssueGroup[] = $derived.by(() => {
    const high: IssueGroup = { label: "High Priority", items: [] };
    const low: IssueGroup = { label: "Low Priority", items: [] };
    const other: IssueGroup = { label: "Unprioritized", items: [] };

    issues.forEach((issue, index) => {
      if (issue.labels.some(l => l.name === "priority:high")) {
        high.items.push({ issue, index });
      } else if (issue.labels.some(l => l.name === "priority:low")) {
        low.items.push({ issue, index });
      } else {
        other.items.push({ issue, index });
      }
    });

    return [high, other, low].filter(g => g.items.length > 0);
  });

  onMount(() => {
    window.addEventListener("keydown", handleKeydown, { capture: true });

    (async () => {
      try {
        const allIssues = await command<GithubIssue[]>("list_github_issues", { repoPath });
        issues = allIssues
          .filter(issue => !issue.labels.some(l => l.name === "in-progress"))
          .sort((a, b) => {
            const priorityOf = (issue: GithubIssue) => {
              if (issue.labels.some(l => l.name === "priority:high")) return 0;
              if (issue.labels.some(l => l.name === "priority:low")) return 2;
              return 1; // unprioritized in the middle
            };
            return priorityOf(a) - priorityOf(b);
          });
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
        {#each groups as group}
          <li class="group-header">{group.label}</li>
          {#each group.items as { issue, index }}
            <li>
              <button class="issue-btn" class:selected={selectedIndex === index} onclick={() => onSelect(issue)}>
                <span class="issue-number">#{issue.number}</span>
                <span class="issue-title">{issue.title}</span>
              </button>
            </li>
          {/each}
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
    background: rgba(0, 0, 0, 0.7);
    backdrop-filter: blur(16px);
    display: flex;
    align-items: flex-start;
    justify-content: center;
    padding-top: 20vh;
    z-index: 100;
  }
  .modal {
    background: var(--bg-elevated);
    border: 1px solid var(--border-default);
    border-radius: 8px;
    width: 420px;
    padding: 24px;
    display: flex;
    flex-direction: column;
    gap: 12px;
    box-shadow: 0 8px 32px rgba(0, 0, 0, 0.6);
  }
  .modal-header {
    font-size: 16px;
    font-weight: 600;
    color: var(--text-emphasis);
  }
  .status {
    color: var(--text-secondary);
    font-size: 13px;
  }
  .status.error {
    color: var(--status-error);
  }
  .issue-list {
    list-style: none;
    margin: 0;
    padding: 0;
    max-height: 50vh;
    overflow-y: auto;
  }
  .issue-list li {
    border-bottom: 1px solid var(--border-default);
  }
  .issue-list li.group-header {
    border-bottom: none;
  }
  .issue-list li:last-child {
    border-bottom: none;
  }
  .issue-btn {
    width: 100%;
    display: flex;
    gap: 8px;
    align-items: center;
    padding: 10px 8px;
    background: none;
    border: none;
    color: var(--text-primary);
    font-size: 13px;
    cursor: pointer;
    text-align: left;
    box-shadow: none;
  }
  .issue-btn:hover,
  .issue-btn.selected {
    background: var(--bg-hover);
    border-radius: 4px;
  }
  .group-header {
    font-size: 11px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    color: var(--text-secondary);
    padding: 10px 8px 4px;
  }
  .group-header:first-child {
    padding-top: 4px;
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
  .no-issue {
    color: var(--text-secondary);
  }
  .no-issue.selected,
  .no-issue:hover {
    color: var(--text-primary);
  }
</style>
