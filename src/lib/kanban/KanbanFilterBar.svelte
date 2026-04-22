<script lang="ts">
  import type { GithubIssue } from "../stores";
  import { isStatusLabel } from "./columns";
  import type { Filters } from "./filter";

  interface Props {
    issues: GithubIssue[];
    filters: Filters;
    onchange: (next: Filters) => void;
  }

  let { issues, filters, onchange }: Props = $props();

  let assigneeOptions = $derived.by(() => {
    const set = new Set<string>();
    for (const issue of issues) {
      for (const a of issue.assignees ?? []) set.add(a.login);
    }
    return [...set].sort();
  });

  let labelOptions = $derived.by(() => {
    const set = new Set<string>();
    for (const issue of issues) {
      for (const l of issue.labels) {
        if (!isStatusLabel(l.name)) set.add(l.name);
      }
    }
    return [...set].sort();
  });

  let milestoneOptions = $derived.by(() => {
    const set = new Set<string>();
    for (const issue of issues) {
      const m = issue.milestone?.title;
      if (m) set.add(m);
    }
    return [...set].sort();
  });

  function toggleAssignee(login: string) {
    const next = new Set(filters.assignees);
    if (next.has(login)) next.delete(login);
    else next.add(login);
    onchange({ ...filters, assignees: next });
  }

  function toggleLabel(name: string) {
    const next = new Set(filters.labels);
    if (next.has(name)) next.delete(name);
    else next.add(name);
    onchange({ ...filters, labels: next });
  }

  function selectMilestone(e: Event) {
    const v = (e.currentTarget as HTMLSelectElement).value;
    onchange({ ...filters, milestone: v === "" ? null : v });
  }

  function clearAll() {
    onchange({ assignees: new Set(), labels: new Set(), milestone: null });
  }

  let hasActive = $derived(
    filters.assignees.size > 0 ||
      filters.labels.size > 0 ||
      filters.milestone !== null,
  );
</script>

<div class="filter-bar" data-testid="kanban-filter-bar">
  {#if assigneeOptions.length > 0}
    <div class="filter-group">
      <span class="filter-label">Assignee</span>
      {#each assigneeOptions as login (login)}
        <button
          type="button"
          class="chip"
          class:active={filters.assignees.has(login)}
          onclick={() => toggleAssignee(login)}
        >{login}</button>
      {/each}
    </div>
  {/if}

  {#if labelOptions.length > 0}
    <div class="filter-group">
      <span class="filter-label">Label</span>
      {#each labelOptions as name (name)}
        <button
          type="button"
          class="chip"
          class:active={filters.labels.has(name)}
          onclick={() => toggleLabel(name)}
        >{name}</button>
      {/each}
    </div>
  {/if}

  {#if milestoneOptions.length > 0}
    <div class="filter-group">
      <span class="filter-label">Milestone</span>
      <select onchange={selectMilestone} value={filters.milestone ?? ""}>
        <option value="">Any</option>
        {#each milestoneOptions as title (title)}
          <option value={title}>{title}</option>
        {/each}
      </select>
    </div>
  {/if}

  {#if hasActive}
    <button type="button" class="clear-btn" onclick={clearAll}>Clear filters</button>
  {/if}
</div>

<style>
  .filter-bar {
    display: flex;
    flex-wrap: wrap;
    gap: 12px;
    align-items: center;
    padding: 8px 20px;
    border-bottom: 1px solid var(--border-default);
    background: var(--bg-elevated);
  }

  .filter-group {
    display: flex;
    flex-wrap: wrap;
    gap: 4px;
    align-items: center;
  }

  .filter-label {
    font-size: 11px;
    color: var(--text-secondary);
    text-transform: uppercase;
    letter-spacing: 0.05em;
    margin-right: 4px;
  }

  .chip {
    font-size: 11px;
    padding: 2px 8px;
    border-radius: 10px;
    border: 1px solid var(--border-default);
    background: transparent;
    color: var(--text-secondary);
    cursor: pointer;
  }

  .chip.active {
    background: var(--text-emphasis);
    color: var(--bg-void);
    border-color: var(--text-emphasis);
  }

  select {
    font-size: 11px;
    padding: 2px 6px;
    background: var(--bg-void);
    border: 1px solid var(--border-default);
    color: var(--text-primary);
    border-radius: 4px;
  }

  .clear-btn {
    margin-left: auto;
    font-size: 11px;
    padding: 2px 8px;
    border: 1px solid var(--border-default);
    background: transparent;
    color: var(--text-secondary);
    border-radius: 4px;
    cursor: pointer;
  }
</style>
