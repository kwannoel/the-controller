<script lang="ts">
  import { onMount } from "svelte";
  import { fromStore } from "svelte/store";
  import { invoke } from "@tauri-apps/api/core";
  import { showToast } from "./toast";
  import { focusTarget, projects, type GithubIssue, type Project, type FocusTarget } from "./stores";

  interface Props {
    onClose: () => void;
  }

  let { onClose }: Props = $props();

  let issues: GithubIssue[] = $state([]);
  let currentIndex = $state(0);
  let loading = $state(false);
  let error: string | null = $state(null);
  let swipeDirection: "left" | "right" | null = $state(null);
  let triageCount = $state({ high: 0, low: 0, skipped: 0 });

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
      // Only show issues without a priority label and not in-progress
      issues = allIssues.filter(issue =>
        !issue.labels.some(l => l.name === "in-progress") &&
        !issue.labels.some(l => l.name.startsWith("priority:"))
      );
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

    const issue = currentIssue;
    const label = `priority: ${priority}`;
    swipeDirection = priority === "high" ? "right" : "left";

    // Update count
    if (priority === "high") triageCount.high++;
    else triageCount.low++;

    // Wait for animation
    await new Promise(r => setTimeout(r, 300));
    swipeDirection = null;
    currentIndex++;

    // Fire and forget the label assignment
    invoke("add_github_label", {
      repoPath,
      issueNumber: issue.number,
      label,
      description: priority === "high" ? "Important, should be tackled soon" : "Nice to have, can wait",
      color: priority === "high" ? "F38BA8" : "A6E3A1",
    }).catch((e: unknown) => showToast(`Failed to label #${issue.number}: ${e}`, "error"));
  }

  function skip() {
    if (!currentIssue) return;
    triageCount.skipped++;
    swipeDirection = null;
    currentIndex++;
  }

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === "Escape") {
      e.preventDefault();
      e.stopPropagation();
      onClose();
      return;
    }

    if (swipeDirection) return; // animating

    if (e.key === "ArrowRight" || e.key === "h") {
      e.preventDefault();
      e.stopPropagation();
      assignPriority("high");
    } else if (e.key === "ArrowLeft" || e.key === "l") {
      e.preventDefault();
      e.stopPropagation();
      assignPriority("low");
    } else if (e.key === "s" || e.key === "ArrowDown") {
      e.preventDefault();
      e.stopPropagation();
      skip();
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
      <h2>Triage Issues</h2>
      <div class="triage-stats">
        <span class="stat high">{triageCount.high} high</span>
        <span class="stat low">{triageCount.low} low</span>
        <span class="stat skipped">{triageCount.skipped} skipped</span>
      </div>
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
        <div class="done-summary">
          <span class="stat high">{triageCount.high} high</span>
          <span class="stat low">{triageCount.low} low</span>
          <span class="stat skipped">{triageCount.skipped} skipped</span>
        </div>
        <div class="done-hint">Press <kbd>Esc</kbd> to close</div>
      </div>
    {:else}
      <div class="card-area">
        <div class="label-hint left">
          <span class="label-arrow">←</span>
          <span class="label-text low">Low</span>
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
          <span class="label-text high">High</span>
          <span class="label-arrow">→</span>
        </div>
      </div>

      <div class="hotkey-bar">
        <div class="hotkey-group">
          <kbd>←</kbd> / <kbd>l</kbd>
          <span class="hotkey-desc">Low priority</span>
        </div>
        <div class="hotkey-group">
          <kbd>↓</kbd> / <kbd>s</kbd>
          <span class="hotkey-desc">Skip</span>
        </div>
        <div class="hotkey-group">
          <kbd>→</kbd> / <kbd>h</kbd>
          <span class="hotkey-desc">High priority</span>
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

  .triage-stats {
    display: flex;
    gap: 12px;
  }

  .stat {
    font-size: 12px;
    font-weight: 500;
    padding: 2px 8px;
    border-radius: 4px;
  }

  .stat.high {
    color: #f38ba8;
    background: rgba(243, 139, 168, 0.1);
  }

  .stat.low {
    color: #a6e3a1;
    background: rgba(166, 227, 161, 0.1);
  }

  .stat.skipped {
    color: #6c7086;
    background: rgba(108, 112, 134, 0.1);
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

  .done-summary {
    display: flex;
    gap: 12px;
  }

  .done-hint {
    margin-top: 8px;
    color: #6c7086;
    font-size: 13px;
  }
</style>
