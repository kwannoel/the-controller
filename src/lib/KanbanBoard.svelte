<script lang="ts">
  import { fromStore } from "svelte/store";
  import { command } from "$lib/backend";
  import { projects, focusTarget, type GithubIssue } from "./stores";
  import { showToast } from "./toast";
  import {
    COLUMNS,
    columnForIssue,
    LABEL_BY_COLUMN,
    LABEL_COLOR,
    type Column,
  } from "./kanban/columns";
  import { applyOrdering, moveIssue, orderKey, type OrderMap } from "./kanban/ordering";
  import { applyFilters, type Filters } from "./kanban/filter";
  import KanbanColumn from "./kanban/KanbanColumn.svelte";
  import KanbanFilterBar from "./kanban/KanbanFilterBar.svelte";

  const projectsState = fromStore(projects);
  const focusTargetState = fromStore(focusTarget);

  let focusedProject = $derived.by(() => {
    const f = focusTargetState.current;
    if (!f || !("projectId" in f)) return null;
    return projectsState.current.find((p) => p.id === f.projectId) ?? null;
  });

  let repoPath = $derived(focusedProject?.repo_path ?? null);

  let issues: GithubIssue[] = $state([]);
  let loading = $state(false);
  let error: string | null = $state(null);
  let orderMap: OrderMap = $state({});
  let draggingIssue: GithubIssue | null = $state(null);
  let filters: Filters = $state({
    assignees: new Set(),
    labels: new Set(),
    milestone: null,
  });

  let filteredIssues = $derived(applyFilters(issues, filters));

  let issuesByColumn = $derived.by(() => {
    const repo = repoPath ?? "";
    const grouped: Record<Column, GithubIssue[]> = {
      backlog: [],
      todo: [],
      "in-progress": [],
      "in-review": [],
      done: [],
    };
    for (const issue of filteredIssues) {
      grouped[columnForIssue(issue)].push(issue);
    }
    for (const col of COLUMNS) {
      grouped[col] = applyOrdering(grouped[col], col, repo, orderMap);
    }
    return grouped;
  });

  $effect(() => {
    const path = repoPath;
    if (!path) {
      issues = [];
      error = null;
      return;
    }
    loadIssues(path);
  });

  $effect(() => {
    loadOrder();
  });

  async function loadIssues(path: string) {
    loading = true;
    error = null;
    try {
      issues = await command<GithubIssue[]>("list_github_issues", {
        repoPath: path,
      });
    } catch (e) {
      error = String(e);
      issues = [];
    } finally {
      loading = false;
    }
  }

  async function loadOrder() {
    try {
      const loaded = await command<unknown>("kanban_load_order");
      orderMap =
        loaded && typeof loaded === "object" ? (loaded as OrderMap) : {};
    } catch {
      orderMap = {};
    }
  }

  async function saveOrder(next: OrderMap) {
    try {
      await command("kanban_save_order", { order: next });
    } catch (e) {
      showToast(`Failed to save order: ${e}`, "error");
    }
  }

  function onDragStart(issue: GithubIssue) {
    draggingIssue = issue;
  }

  function onDragEnd() {
    draggingIssue = null;
  }

  async function onDrop(toColumn: Column, toIndex: number) {
    const issue = draggingIssue;
    const repo = repoPath;
    draggingIssue = null;
    if (!issue || !repo) return;

    const fromColumn = columnForIssue(issue);
    const fromKey = orderKey(repo, fromColumn);
    const toKey = orderKey(repo, toColumn);

    const previousIssues = issues;
    const previousOrder = orderMap;

    // Optimistic update.
    orderMap = moveIssue(orderMap, fromKey, toKey, issue.number, toIndex);
    if (fromColumn !== toColumn) {
      issues = issues.map((i) => {
        if (i.number !== issue.number) return i;
        const withoutStatus = i.labels.filter(
          (l) => l.name !== LABEL_BY_COLUMN[fromColumn],
        );
        return {
          ...i,
          labels: [...withoutStatus, { name: LABEL_BY_COLUMN[toColumn] }],
        };
      });
    }

    try {
      if (fromColumn !== toColumn) {
        const hasFromLabel = previousIssues
          .find((i) => i.number === issue.number)
          ?.labels.some((l) => l.name === LABEL_BY_COLUMN[fromColumn]);
        if (hasFromLabel) {
          await command("remove_github_label", {
            repoPath: repo,
            issueNumber: issue.number,
            label: LABEL_BY_COLUMN[fromColumn],
          });
        }
        await command("add_github_label", {
          repoPath: repo,
          issueNumber: issue.number,
          label: LABEL_BY_COLUMN[toColumn],
          description: `Kanban column: ${toColumn}`,
          color: LABEL_COLOR[toColumn],
        });
      }
      await saveOrder(orderMap);
    } catch (e) {
      issues = previousIssues;
      orderMap = previousOrder;
      showToast(`Failed to move issue: ${e}`, "error");
    }
  }
</script>

<div class="kanban-board" data-testid="kanban-board">
  <header class="board-header">
    <h1>Kanban</h1>
    {#if focusedProject}
      <span class="board-project">{focusedProject.name}</span>
    {:else}
      <span class="board-project board-project--muted">Focus a project to load its board</span>
    {/if}
    {#if loading}<span class="board-status">Loading...</span>{/if}
    {#if error}<span class="board-status board-status--error">{error}</span>{/if}
  </header>
  <KanbanFilterBar
    {issues}
    {filters}
    onchange={(next) => (filters = next)}
  />
  <div class="columns">
    {#each COLUMNS as column (column)}
      <KanbanColumn
        {column}
        issues={issuesByColumn[column]}
        ondragstart={onDragStart}
        ondragend={onDragEnd}
        ondrop={onDrop}
      />
    {/each}
  </div>
</div>

<style>
  .kanban-board {
    display: flex;
    flex-direction: column;
    height: 100%;
    width: 100%;
    background: var(--bg-void);
    color: var(--text-primary);
    overflow: hidden;
  }

  .board-header {
    display: flex;
    align-items: baseline;
    gap: 16px;
    padding: 16px 20px 12px;
    border-bottom: 1px solid var(--border-default);
  }

  .board-header h1 {
    font-size: 16px;
    font-weight: 600;
    margin: 0;
  }

  .board-project {
    font-size: 13px;
    color: var(--text-secondary);
  }

  .board-project--muted {
    font-style: italic;
  }

  .board-status {
    font-size: 12px;
    color: var(--text-secondary);
    margin-left: auto;
  }

  .board-status--error {
    color: var(--color-danger, #f38ba8);
  }

  .columns {
    display: grid;
    grid-template-columns: repeat(5, minmax(220px, 1fr));
    gap: 12px;
    padding: 12px;
    overflow-x: auto;
    flex: 1;
    min-height: 0;
  }
</style>
