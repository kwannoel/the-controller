<script lang="ts">
  import { onMount } from "svelte";
  import { fromStore } from "svelte/store";
  import { invoke } from "@tauri-apps/api/core";
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
      const allIssues = await invoke<GithubIssue[]>("list_github_issues", { repoPath: path });
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

  async function assignPriority(priority: "high" | "low") {
    if (!currentIssue || !repoPath) return;

    swipeDirection = priority === "high" ? "right" : "left";

    await new Promise(r => setTimeout(r, 300));
    swipeDirection = null;
    pendingPriority = priority;
    step = "complexity";
  }

  async function assignComplexity(complexity: "simple" | "complex") {
    if (!currentIssue || !repoPath) return;

    const issue = currentIssue;
    const priority = pendingPriority;
    swipeDirection = complexity === "complex" ? "right" : "left";

    await new Promise(r => setTimeout(r, 300));
    swipeDirection = null;
    advanceCard(issue, priority, complexity);
  }

  function skipPriority() {
    if (!currentIssue) return;
    pendingPriority = null;
    step = "complexity";
  }

  function skipComplexity() {
    if (!currentIssue) return;
    const issue = currentIssue;
    const priority = pendingPriority;
    advanceCard(issue, priority, null);
  }

  function advanceCard(issue: GithubIssue, priority: "high" | "low" | null, complexity: "simple" | "complex" | null) {
    currentIndex++;
    step = "priority";
    pendingPriority = null;

    if (!repoPath) return;
    const path = repoPath;

    // Fire and forget label assignments
    if (priority) {
      invoke("add_github_label", {
        repoPath: path,
        issueNumber: issue.number,
        label: `priority: ${priority}`,
        description: priority === "high" ? "Important, should be tackled soon" : "Nice to have, can wait",
        color: priority === "high" ? "F38BA8" : "A6E3A1",
      }).catch((e: unknown) => showToast(`Failed to label #${issue.number}: ${e}`, "error"));
    }

    if (complexity) {
      invoke("add_github_label", {
        repoPath: path,
        issueNumber: issue.number,
        label: `complexity: ${complexity}`,
        description: complexity === "simple" ? "Quick task, suitable for simple agents" : "Multi-step task, needs capable agents",
        color: complexity === "simple" ? "89DCEB" : "FAB387",
      }).catch((e: unknown) => showToast(`Failed to label #${issue.number}: ${e}`, "error"));
    }

    // Mark issue as triaged
    invoke("add_github_label", {
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
      } else if (e.key === "s" || e.key === "ArrowDown") {
        e.preventDefault();
        e.stopPropagation();
        skipPriority();
      }
    } else {
      if (e.key === "ArrowRight" || e.key === "k") {
        e.preventDefault();
        e.stopPropagation();
        assignComplexity("complex");
      } else if (e.key === "ArrowLeft" || e.key === "j") {
        e.preventDefault();
        e.stopPropagation();
        assignComplexity("simple");
      } else if (e.key === "s" || e.key === "ArrowDown") {
        e.preventDefault();
        e.stopPropagation();
        skipComplexity();
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
      <div class="step-indicator">
        <span class="step-dot" class:active={step === "priority"}></span>
        <span class="step-dot" class:active={step === "complexity"}></span>
        <span class="step-label">{step === "priority" ? "Step 1: Priority" : "Step 2: Complexity"}</span>
      </div>

      <div class="card-area">
        <div class="label-hint left">
          <span class="label-arrow">&#8592;</span>
          <span class="label-text" class:low={step === "priority"} class:simple={step === "complexity"}>
            {step === "priority" ? "Low" : "Simple"}
          </span>
        </div>

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
          {#if currentIssue.labels.length > 0}
            <div class="card-labels">
              {#each currentIssue.labels as label}
                <span class="card-label">{label.name}</span>
              {/each}
            </div>
          {/if}
          <div class="card-counter">{remaining} remaining</div>
        </div>

        <div class="label-hint right">
          <span class="label-text" class:high={step === "priority"} class:complex={step === "complexity"}>
            {step === "priority" ? "High" : "Complex"}
          </span>
          <span class="label-arrow">&#8594;</span>
        </div>
      </div>

      <div class="hotkey-bar">
        <div class="hotkey-group">
          <kbd>&#8592;</kbd> / <kbd>j</kbd>
          <span class="hotkey-desc">{step === "priority" ? "Low priority" : "Simple"}</span>
        </div>
        <div class="hotkey-group">
          <kbd>&#8595;</kbd> / <kbd>s</kbd>
          <span class="hotkey-desc">Skip</span>
        </div>
        <div class="hotkey-group">
          <kbd>&#8594;</kbd> / <kbd>k</kbd>
          <span class="hotkey-desc">{step === "priority" ? "High priority" : "Complex"}</span>
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
    width: 520px;
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
    align-items: center;
    gap: 16px;
    min-height: 180px;
  }

  .label-hint {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 4px;
    width: 48px;
    flex-shrink: 0;
    opacity: 0.5;
  }

  .label-arrow {
    font-size: 20px;
    color: #6c7086;
  }

  .label-text {
    font-size: 11px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.05em;
  }

  .label-text.high {
    color: #f38ba8;
  }

  .label-text.low {
    color: #a6e3a1;
  }

  .label-text.simple {
    color: #89dceb;
  }

  .label-text.complex {
    color: #fab387;
  }

.step-indicator {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .step-dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: #45475a;
  }

  .step-dot.active {
    background: #89b4fa;
  }

  .step-label {
    font-size: 12px;
    color: #6c7086;
    margin-left: 4px;
  }

  .issue-card {
    flex: 1;
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

  .hotkey-bar {
    display: flex;
    justify-content: space-between;
    padding: 12px 0 0;
    border-top: 1px solid #313244;
  }

  .hotkey-group {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: 13px;
    color: #6c7086;
  }

  .hotkey-desc {
    font-size: 12px;
  }

  kbd {
    background: #313244;
    color: #89b4fa;
    padding: 2px 8px;
    border-radius: 4px;
    font-family: monospace;
    font-size: 13px;
    font-weight: 500;
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
