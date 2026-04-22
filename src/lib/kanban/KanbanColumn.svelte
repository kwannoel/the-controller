<script lang="ts">
  import type { GithubIssue } from "../stores";
  import type { Column } from "./columns";
  import { COLUMN_TITLES, EMPTY_STATE } from "./columns";
  import KanbanCard from "./KanbanCard.svelte";

  interface Props {
    column: Column;
    issues: GithubIssue[];
    ondragstart?: (issue: GithubIssue) => void;
    ondragend?: () => void;
    ondrop?: (column: Column, toIndex: number) => void;
  }

  let { column, issues, ondragstart, ondragend, ondrop }: Props = $props();

  let isOver = $state(false);
  let bodyEl: HTMLDivElement | undefined = $state();

  const VIRTUALIZE_THRESHOLD = 50;
  const CARD_HEIGHT = 80;

  let scrollTop = $state(0);
  let viewportHeight = $state(0);

  let useVirtual = $derived(issues.length > VIRTUALIZE_THRESHOLD);
  let startIndex = $derived(
    useVirtual ? Math.max(0, Math.floor(scrollTop / CARD_HEIGHT) - 3) : 0,
  );
  let endIndex = $derived(
    useVirtual
      ? Math.min(
          issues.length,
          Math.ceil((scrollTop + viewportHeight) / CARD_HEIGHT) + 3,
        )
      : issues.length,
  );
  let visibleIssues = $derived(
    useVirtual ? issues.slice(startIndex, endIndex) : issues,
  );
  let topSpacer = $derived(useVirtual ? startIndex * CARD_HEIGHT : 0);
  let bottomSpacer = $derived(
    useVirtual ? (issues.length - endIndex) * CARD_HEIGHT : 0,
  );

  function onScroll(e: Event) {
    if (!useVirtual) return;
    const el = e.currentTarget as HTMLDivElement;
    scrollTop = el.scrollTop;
    viewportHeight = el.clientHeight;
  }

  function onDragOver(e: DragEvent) {
    e.preventDefault();
    if (e.dataTransfer) e.dataTransfer.dropEffect = "move";
    isOver = true;
  }

  function onDragLeave() {
    isOver = false;
  }

  function onDrop(e: DragEvent) {
    e.preventDefault();
    isOver = false;
    if (!bodyEl) return;
    const y = e.clientY - bodyEl.getBoundingClientRect().top + bodyEl.scrollTop;
    const toIndex = Math.max(0, Math.floor(y / CARD_HEIGHT));
    ondrop?.(column, Math.min(toIndex, issues.length));
  }
</script>

<section
  class="column"
  class:drop-target={isOver}
  data-column={column}
  data-testid="kanban-column-{column}"
>
  <header class="column-header">
    <span class="column-title">{COLUMN_TITLES[column]}</span>
    <span class="column-count">{issues.length}</span>
  </header>
  <div
    class="column-body"
    bind:this={bodyEl}
    onscroll={onScroll}
    ondragover={onDragOver}
    ondragleave={onDragLeave}
    ondrop={onDrop}
    role="list"
  >
    {#if issues.length === 0}
      <p class="empty-state">{EMPTY_STATE[column]}</p>
    {:else}
      {#if topSpacer > 0}
        <div class="spacer" style="height: {topSpacer}px"></div>
      {/if}
      {#each visibleIssues as issue (issue.number)}
        <KanbanCard {issue} {ondragstart} {ondragend} />
      {/each}
      {#if bottomSpacer > 0}
        <div class="spacer" style="height: {bottomSpacer}px"></div>
      {/if}
    {/if}
  </div>
</section>

<style>
  .column {
    display: flex;
    flex-direction: column;
    background: var(--bg-elevated);
    border: 1px solid var(--border-default);
    border-radius: 8px;
    min-width: 0;
    min-height: 0;
  }

  .column.drop-target {
    border-color: var(--text-emphasis);
    background: color-mix(in srgb, var(--bg-elevated) 85%, var(--text-emphasis) 15%);
  }

  .column-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 10px 12px;
    font-size: 13px;
    font-weight: 600;
    border-bottom: 1px solid var(--border-default);
    color: var(--text-primary);
  }

  .column-count {
    font-size: 11px;
    color: var(--text-secondary);
    font-weight: 400;
  }

  .column-body {
    flex: 1;
    overflow-y: auto;
    padding: 8px;
    min-height: 0;
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  .empty-state {
    margin: 0;
    padding: 16px 8px;
    font-size: 12px;
    color: var(--text-secondary);
    font-style: italic;
    text-align: center;
  }

  .spacer {
    flex-shrink: 0;
  }
</style>
