<script lang="ts">
  import { onMount } from "svelte";
  import { fromStore } from "svelte/store";
  import { command } from "$lib/backend";
  import { showToast } from "./toast";
  import { focusTarget, projects, type GithubIssue, type Project, type FocusTarget, type TriageCategory } from "./stores";

  interface Props {
    category: TriageCategory;
    onClose: () => void;
  }

  let { category, onClose }: Props = $props();

  let issues: GithubIssue[] = $state([]);
  let currentIndex = $state(0);
  let loading = $state(false);
  let error: string | null = $state(null);
  let swipeDirection: "left" | "right" | null = $state(null);
  let step: "priority" | "complexity" = $state("priority");
  let pendingPriority: "high" | "low" | null = $state(null);

  const projectsState = fromStore(projects);
  let projectList: Project[] = $derived(projectsState.current);
  const focusTargetState = fromStore(focusTarget);
  let currentFocus: FocusTarget = $derived(focusTargetState.current);

  let currentIssue: GithubIssue | null = $derived(
    currentIndex < issues.length ? issues[currentIndex] : null
  );

  let remaining = $derived(issues.length - currentIndex);

  // Separate triage-related labels from other labels
  const triageLabelRe = /^(priority|complexity):\s?/;
  let triageLabels = $derived(
    currentIssue?.labels.filter(l => triageLabelRe.test(l.name)) ?? []
  );
  let otherLabels = $derived(
    currentIssue?.labels.filter(l => !triageLabelRe.test(l.name)) ?? []
  );

  let currentPriorityLabel = $derived(
    triageLabels.find(l => l.name.startsWith("priority:"))?.name.replace(/^priority:\s?/, "") ?? "none"
  );
  let currentComplexityLabel = $derived.by(() => {
    const raw = triageLabels.find(l => l.name.startsWith("complexity:"))?.name.replace(/^complexity:\s?/, "") ?? "none";
    // Maintainer uses "simple", triage panel uses "low" — normalize
    return raw === "simple" ? "low" : raw;
  });

  let project: Project | null = $derived(
    currentFocus?.projectId
      ? projectList.find((p) => p.id === currentFocus!.projectId) ?? null
      : projectList[0] ?? null
  );
  let repoPath: string | null = $derived(project?.repo_path ?? null);

  $effect(() => {
    if (repoPath) {
      fetchUntriagedIssues(repoPath);
    }
  });

  async function fetchUntriagedIssues(path: string) {
    loading = true;
    error = null;
    try {
      const allIssues = await command<GithubIssue[]>("list_github_issues", { repoPath: path });
      // Filter based on triage category
      issues = allIssues.filter(issue => {
        if (issue.labels.some(l => l.name === "in-progress")) return false;
        const isTriaged = issue.labels.some(l => l.name === "triaged");
        return category === "triaged" ? isTriaged : !isTriaged;
      });
      currentIndex = 0;
    } catch (e) {
      error = String(e);
      issues = [];
    } finally {
      loading = false;
    }
  }

  function assignPriority(priority: "high" | "low") {
    if (!currentIssue || !repoPath) return;

    pendingPriority = priority;
    step = "complexity";
  }

  function goBackToPriority() {
    step = "priority";
    pendingPriority = null;
  }

  async function assignComplexity(complexity: "low" | "high") {
    if (!currentIssue || !repoPath) return;

    const issue = currentIssue;
    const priority = pendingPriority;
    swipeDirection = complexity === "high" ? "right" : "left";

    await new Promise(r => setTimeout(r, 300));
    swipeDirection = null;
    advanceCard(issue, priority, complexity);
  }

  function advanceCard(issue: GithubIssue, priority: "high" | "low" | null, complexity: "low" | "high" | null) {
    currentIndex++;
    step = "priority";
    pendingPriority = null;

    if (!repoPath) return;
    const path = repoPath;

    // Fire and forget label assignments
    if (priority) {
      command("add_github_label", {
        repoPath: path,
        issueNumber: issue.number,
        label: `priority:${priority}`,
        description: priority === "high" ? "Important, should be tackled soon" : "Nice to have, can wait",
        color: priority === "high" ? "F38BA8" : "A6E3A1",
      }).catch((e: unknown) => showToast(`Failed to label #${issue.number}: ${e}`, "error"));
    }

    if (complexity) {
      command("add_github_label", {
        repoPath: path,
        issueNumber: issue.number,
        label: complexity === "low" ? "complexity:low" : "complexity:high",
        description: complexity === "low" ? "Quick task, suitable for simple agents" : "Multi-step task, needs capable agents",
        color: complexity === "low" ? "89DCEB" : "FAB387",
      }).catch((e: unknown) => showToast(`Failed to label #${issue.number}: ${e}`, "error"));
    }

    // Mark issue as triaged
    command("add_github_label", {
      repoPath: path,
      issueNumber: issue.number,
      label: "triaged",
      description: "Issue has been triaged",
      color: "CBA6F7",
    }).catch((e: unknown) => showToast(`Failed to mark #${issue.number} as triaged: ${e}`, "error"));
  }

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === "Escape") {
      e.preventDefault();
      e.stopPropagation();
      onClose();
      return;
    }

    if (e.metaKey || e.ctrlKey || e.altKey) return;
    if (swipeDirection) return; // animating

    if (step === "priority") {
      if (e.key === "ArrowRight" || e.key === "k") {
        e.preventDefault();
        e.stopPropagation();
        assignPriority("high");
      } else if (e.key === "ArrowLeft" || e.key === "j") {
        e.preventDefault();
        e.stopPropagation();
        assignPriority("low");
      }
    } else {
      if (e.key === "h") {
        e.preventDefault();
        e.stopPropagation();
        goBackToPriority();
      } else if (e.key === "ArrowRight" || e.key === "k") {
        e.preventDefault();
        e.stopPropagation();
        assignComplexity("high");
      } else if (e.key === "ArrowLeft" || e.key === "j") {
        e.preventDefault();
        e.stopPropagation();
        assignComplexity("low");
      }
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
  <div class="triage-container" onclick={(e) => e.stopPropagation()} role="presentation">
    <div class="triage-header">
      <h2>Triage: {category === "untriaged" ? "Untriaged" : "Triaged"}</h2>
    </div>

    {#if loading}
      <div class="status">Loading issues...</div>
    {:else if error}
      <div class="status error">{error}</div>
    {:else if issues.length === 0}
      <div class="status">No untriaged issues found</div>
    {:else if !currentIssue}
      <div class="done-container">
        <div class="done-icon">✓</div>
        <div class="done-text">All issues triaged!</div>
        <div class="done-hint">Press <kbd>Esc</kbd> to close</div>
      </div>
    {:else}
      <div class="card-area">
        <div
          class="issue-card"
          class:swipe-left={swipeDirection === "left"}
          class:swipe-right={swipeDirection === "right"}
        >
          <div class="card-number">#{currentIssue.number}</div>
          <div class="card-title">{currentIssue.title}</div>
          {#if currentIssue.body}
            <div class="card-body">{currentIssue.body}</div>
          {/if}
          {#if otherLabels.length > 0}
            <div class="card-labels">
              {#each otherLabels as label}
                <span class="card-label">{label.name}</span>
              {/each}
            </div>
          {/if}
          <div class="card-counter">{remaining} remaining</div>
        </div>

        <div class="ranking-panel">
          {#if step === "priority"}
            <div class="ranking-options">
              <button class="ranking-option" onclick={() => assignPriority("low")}>
                <span class="ranking-key">j</span>
                <span class="ranking-label-group">
                  <span class="ranking-label">Low priority</span>
                  {#if currentPriorityLabel === "low"}<span class="current-tag">(current)</span>{/if}
                </span>
              </button>
              <button class="ranking-option" onclick={() => assignPriority("high")}>
                <span class="ranking-key">k</span>
                <span class="ranking-label-group">
                  <span class="ranking-label">High priority</span>
                  {#if currentPriorityLabel === "high"}<span class="current-tag">(current)</span>{/if}
                </span>
              </button>
            </div>
          {:else}
            <div class="ranking-options">
              <button class="ranking-option" onclick={() => assignComplexity("low")}>
                <span class="ranking-key">j</span>
                <span class="ranking-label-group">
                  <span class="ranking-label">Low complexity</span>
                  {#if currentComplexityLabel === "low"}<span class="current-tag">(current)</span>{/if}
                </span>
              </button>
              <button class="ranking-option" onclick={() => assignComplexity("high")}>
                <span class="ranking-key">k</span>
                <span class="ranking-label-group">
                  <span class="ranking-label">High complexity</span>
                  {#if currentComplexityLabel === "high"}<span class="current-tag">(current)</span>{/if}
                </span>
              </button>
              <button class="ranking-option back-option" onclick={() => goBackToPriority()}>
                <span class="ranking-key">h</span>
                <span class="ranking-label">Back</span>
              </button>
            </div>
          {/if}
        </div>
      </div>

    {/if}
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

  .triage-container {
    background: #1e1e2e;
    border: 1px solid #313244;
    border-radius: 12px;
    width: 580px;
    padding: 32px;
    display: flex;
    flex-direction: column;
    gap: 24px;
  }

  .triage-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
  }

  .triage-header h2 {
    color: #cdd6f4;
    font-size: 18px;
    font-weight: 600;
    margin: 0;
  }


  .status {
    padding: 32px;
    color: #6c7086;
    font-size: 14px;
    text-align: center;
  }

  .status.error {
    color: #f38ba8;
  }

  .card-area {
    display: flex;
    align-items: stretch;
    gap: 16px;
    min-height: 180px;
  }

  .ranking-panel {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 10px;
    width: 160px;
    flex-shrink: 0;
    justify-content: center;
  }

  .ranking-options {
    display: flex;
    flex-direction: column;
    gap: 6px;
    width: 100%;
  }

  .ranking-option {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 6px;
    padding: 6px 12px;
    border-radius: 6px;
    border: 1px solid #313244;
    background: #181825;
    cursor: pointer;
    transition: background 0.15s, border-color 0.15s;
  }

  .ranking-option:hover {
    background: #313244;
  }

  .ranking-option .ranking-key {
    font-size: 13px;
    color: #6c7086;
  }

  .ranking-label-group {
    display: flex;
    flex-direction: column;
  }

  .ranking-option .ranking-label {
    font-size: 13px;
    font-weight: 600;
    color: #cdd6f4;
  }

  .back-option {
    margin-top: 4px;
    border-color: transparent;
    background: transparent;
  }

  .back-option .ranking-label {
    color: #6c7086;
    font-weight: 400;
  }

  .current-tag {
    display: block;
    font-size: 11px;
    color: #6c7086;
    font-weight: 400;
  }

  .issue-card {
    flex: 1;
    min-width: 0;
    background: #181825;
    border: 1px solid #313244;
    border-radius: 8px;
    padding: 24px;
    display: flex;
    flex-direction: column;
    gap: 12px;
    transition: transform 0.3s ease, opacity 0.3s ease;
  }

  .issue-card.swipe-left {
    transform: translateX(-200px) rotate(-8deg);
    opacity: 0;
  }

  .issue-card.swipe-right {
    transform: translateX(200px) rotate(8deg);
    opacity: 0;
  }

  .card-number {
    color: #89b4fa;
    font-size: 14px;
    font-weight: 600;
  }

  .card-title {
    color: #cdd6f4;
    font-size: 16px;
    line-height: 1.4;
  }

  .card-body {
    color: #a6adc8;
    font-size: 13px;
    line-height: 1.5;
    max-height: 160px;
    overflow-y: auto;
    white-space: pre-wrap;
    overflow-wrap: break-word;
    word-break: break-word;
    border-top: 1px solid #313244;
    padding-top: 8px;
  }

  .card-labels {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
    margin-top: 4px;
  }

  .card-label {
    font-size: 11px;
    color: #bac2de;
    background: #313244;
    padding: 2px 8px;
    border-radius: 4px;
  }

  .card-counter {
    color: #6c7086;
    font-size: 12px;
    margin-top: 4px;
  }


  .done-container {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 12px;
    padding: 32px;
  }

  .done-icon {
    font-size: 32px;
    color: #a6e3a1;
  }

  .done-text {
    font-size: 16px;
    font-weight: 600;
    color: #cdd6f4;
  }

  .done-hint {
    margin-top: 8px;
    color: #6c7086;
    font-size: 13px;
  }
</style>
