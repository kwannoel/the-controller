<script lang="ts">
  import { onMount } from "svelte";
  import { fromStore } from "svelte/store";
  import { command } from "$lib/backend";
  import { focusTarget, projects, type AssignedIssue, type Project, type FocusTarget } from "./stores";

  interface Props {
    onClose: () => void;
  }

  let { onClose }: Props = $props();

  let issues: AssignedIssue[] = $state([]);
  let loading = $state(false);
  let error: string | null = $state(null);

  const projectsState = fromStore(projects);
  let projectList: Project[] = $derived(projectsState.current);
  const focusTargetState = fromStore(focusTarget);
  let currentFocus: FocusTarget = $derived(focusTargetState.current);

  let project: Project | null = $derived(
    currentFocus?.projectId
      ? projectList.find((p) => p.id === currentFocus!.projectId) ?? null
      : projectList[0] ?? null
  );
  let repoPath: string | null = $derived(project?.repo_path ?? null);

  $effect(() => {
    if (repoPath) {
      fetchAssignedIssues(repoPath);
    }
  });

  async function fetchAssignedIssues(path: string) {
    loading = true;
    error = null;
    try {
      issues = await command<AssignedIssue[]>("list_assigned_issues", { repoPath: path });
      // Sort by updatedAt ascending (stalest first)
      issues.sort((a, b) => a.updatedAt.localeCompare(b.updatedAt));
    } catch (e) {
      error = String(e);
      issues = [];
    } finally {
      loading = false;
    }
  }

  function formatDate(iso: string): string {
    const date = new Date(iso);
    const now = new Date();
    const diffMs = now.getTime() - date.getTime();
    const diffDays = Math.floor(diffMs / (1000 * 60 * 60 * 24));

    if (diffDays === 0) return "today";
    if (diffDays === 1) return "1 day ago";
    if (diffDays < 30) return `${diffDays} days ago`;
    if (diffDays < 365) {
      const months = Math.floor(diffDays / 30);
      return months === 1 ? "1 month ago" : `${months} months ago`;
    }
    const years = Math.floor(diffDays / 365);
    return years === 1 ? "1 year ago" : `${years} years ago`;
  }

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === "Escape") {
      e.preventDefault();
      e.stopPropagation();
      onClose();
    }
  }

  onMount(() => {
    window.addEventListener("keydown", handleKeydown, { capture: true });
    return () => {
      window.removeEventListener("keydown", handleKeydown, { capture: true });
    };
  });
</script>

<div class="overlay" onclick={onClose} role="dialog">
  <!-- svelte-ignore a11y_click_events_have_key_events -->
  <div class="panel-container" onclick={(e) => e.stopPropagation()} role="presentation">
    <div class="panel-header">
      <h2>Assigned Issues</h2>
      <span class="count">{issues.length} issue{issues.length !== 1 ? "s" : ""}</span>
    </div>

    {#if loading}
      <div class="status">Loading assigned issues...</div>
    {:else if error}
      <div class="status error">{error}</div>
    {:else if issues.length === 0}
      <div class="status">No assigned open issues found</div>
    {:else}
      <div class="issue-list">
        {#each issues as issue}
          <div class="issue-row">
            <div class="issue-main">
              <span class="issue-number">#{issue.number}</span>
              <span class="issue-title">{issue.title}</span>
            </div>
            <div class="issue-meta">
              <span class="assignees">
                {#each issue.assignees as assignee, i}
                  <span class="assignee">@{assignee.login}</span>{#if i < issue.assignees.length - 1},{/if}
                {/each}
              </span>
              <span class="updated">{formatDate(issue.updatedAt)}</span>
            </div>
            {#if issue.labels.length > 0}
              <div class="issue-labels">
                {#each issue.labels as label}
                  <span class="label">{label.name}</span>
                {/each}
              </div>
            {/if}
          </div>
        {/each}
      </div>
    {/if}

    <div class="panel-footer">
      <kbd>Esc</kbd> close
    </div>
  </div>
</div>

<style>
  .overlay {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.7);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 100;
  }

  .panel-container {
    background: var(--bg-surface);
    border: 1px solid var(--border-default);
    border-radius: 12px;
    width: 640px;
    max-height: 80vh;
    padding: 24px;
    display: flex;
    flex-direction: column;
    gap: 16px;
  }

  .panel-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
  }

  .panel-header h2 {
    color: var(--text-primary);
    font-size: 18px;
    font-weight: 600;
    margin: 0;
  }

  .count {
    color: var(--text-secondary);
    font-size: 13px;
  }

  .status {
    padding: 32px;
    color: var(--text-secondary);
    font-size: 14px;
    text-align: center;
  }

  .status.error {
    color: var(--status-error);
  }

  .issue-list {
    display: flex;
    flex-direction: column;
    gap: 2px;
    overflow-y: auto;
    max-height: 60vh;
  }

  .issue-row {
    padding: 10px 12px;
    border-radius: 6px;
    background: var(--bg-base);
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .issue-row:hover {
    background: var(--bg-surface);
    outline: 1px solid var(--border-default);
  }

  .issue-main {
    display: flex;
    align-items: baseline;
    gap: 8px;
  }

  .issue-number {
    color: var(--text-emphasis);
    font-size: 13px;
    font-weight: 600;
    flex-shrink: 0;
  }

  .issue-title {
    color: var(--text-primary);
    font-size: 14px;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .issue-meta {
    display: flex;
    align-items: center;
    gap: 12px;
    font-size: 12px;
  }

  .assignees {
    color: var(--status-idle);
  }

  .assignee {
    font-weight: 500;
  }

  .updated {
    color: var(--text-secondary);
  }

  .issue-labels {
    display: flex;
    flex-wrap: wrap;
    gap: 4px;
    margin-top: 2px;
  }

  .label {
    font-size: 11px;
    color: var(--text-primary);
    background: var(--bg-hover);
    padding: 1px 6px;
    border-radius: 4px;
  }

  .panel-footer {
    display: flex;
    justify-content: center;
    gap: 8px;
    color: var(--text-secondary);
    font-size: 12px;
    padding-top: 8px;
    border-top: 1px solid var(--border-default);
  }

  .panel-footer kbd {
    background: var(--bg-hover);
    padding: 1px 6px;
    border-radius: 3px;
    font-size: 11px;
    color: var(--text-primary);
  }
</style>
