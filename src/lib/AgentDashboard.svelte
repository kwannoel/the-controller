<script lang="ts">
  import { fromStore } from "svelte/store";
  import { untrack } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { focusTarget, projects, maintainerStatuses, maintainerErrors, autoWorkerStatuses, hotkeyAction, type Project, type FocusTarget, type MaintainerRunLog, type MaintainerStatus, type AutoWorkerStatus } from "./stores";
  import { showToast } from "./toast";

  let runLogs: MaintainerRunLog[] = $state([]);
  let loading = $state(false);
  let triggerLoading = $state(false);

  // Panel navigation state
  let selectedIndex = $state(0);
  let openLogIndex: number | null = $state(null);
  let detailBlockIndex = $state(0);

  const projectsState = fromStore(projects);
  let projectList: Project[] = $derived(projectsState.current);
  const focusTargetState = fromStore(focusTarget);
  let currentFocus: FocusTarget = $derived(focusTargetState.current);

  let focusedAgent = $derived(
    currentFocus?.type === "agent" || currentFocus?.type === "agent-panel"
      ? currentFocus
      : null
  );

  let panelFocused = $derived(currentFocus?.type === "agent-panel");

  let project = $derived(
    focusedAgent
      ? projectList.find((p) => p.id === focusedAgent!.projectId) ?? null
      : null
  );

  let openLog = $derived(
    openLogIndex !== null ? runLogs[openLogIndex] ?? null : null
  );

  // All issues in the open log (for detail navigation)
  let openLogIssues = $derived(
    openLog ? [...openLog.issues_filed, ...openLog.issues_updated] : []
  );

  // Fetch history and reset panel state when switching agents
  let prevAgentKey: string | null = $state(null);
  $effect(() => {
    const key = focusedAgent ? `${focusedAgent.projectId}:${focusedAgent.agentKind}` : null;
    const prev = untrack(() => prevAgentKey);
    if (key !== prev) {
      prevAgentKey = key;
      selectedIndex = 0;
      openLogIndex = null;
      detailBlockIndex = 0;
      runLogs = [];

      const pid = untrack(() => project?.id ?? null);
      if (pid && focusedAgent?.agentKind === "maintainer") {
        fetchHistory(pid);
      }
    }
  });

  // Handle panel navigation actions
  $effect(() => {
    const unsub = hotkeyAction.subscribe((action) => {
      if (!action) return;
      if (action.type === "agent-panel-navigate") {
        handleNavigate(action.direction);
      } else if (action.type === "agent-panel-select") {
        handleSelect();
      } else if (action.type === "agent-panel-escape") {
        handleEscape();
      } else if (action.type === "trigger-maintainer-check") {
        triggerCheck();
      } else if (action.type === "clear-maintainer-reports") {
        clearRunLogs();
      }
    });
    return unsub;
  });

  function handleNavigate(direction: 1 | -1) {
    if (focusedAgent?.agentKind !== "maintainer") return;

    if (openLogIndex !== null) {
      // Detail view: scroll through issue blocks
      const maxBlock = openLogIssues.length; // 0 = summary, 1..N = issues
      detailBlockIndex = Math.max(0, Math.min(maxBlock, detailBlockIndex + direction));
      scrollBlockIntoView();
    } else {
      // List view: move selection
      if (runLogs.length === 0) return;
      selectedIndex = Math.max(0, Math.min(runLogs.length - 1, selectedIndex + direction));
      scrollReportIntoView();
    }
  }

  function handleSelect() {
    if (focusedAgent?.agentKind !== "maintainer") return;
    if (openLogIndex !== null) return;
    if (runLogs.length === 0) return;
    openLogIndex = selectedIndex;
    detailBlockIndex = 0;
  }

  function handleEscape() {
    if (openLogIndex !== null) {
      openLogIndex = null;
      detailBlockIndex = 0;
    } else if (focusedAgent) {
      focusTarget.set({ type: "agent", agentKind: focusedAgent.agentKind, projectId: focusedAgent.projectId });
    }
  }

  function scrollBlockIntoView() {
    requestAnimationFrame(() => {
      const el = document.querySelector(`[data-block-index="${detailBlockIndex}"]`);
      if (el) el.scrollIntoView({ behavior: "smooth", block: "nearest" });
    });
  }

  function scrollReportIntoView() {
    requestAnimationFrame(() => {
      const el = document.querySelector(`[data-report-index="${selectedIndex}"]`);
      if (el) el.scrollIntoView({ behavior: "smooth", block: "nearest" });
    });
  }

  async function fetchHistory(projectId: string) {
    loading = true;
    try {
      const result = await invoke<MaintainerRunLog[]>("get_maintainer_history", { projectId });
      if (prevAgentKey === `${projectId}:maintainer`) {
        runLogs = result;
      }
    } catch {
      if (prevAgentKey === `${projectId}:maintainer`) {
        runLogs = [];
      }
    } finally {
      if (prevAgentKey === `${projectId}:maintainer`) {
        loading = false;
      }
    }
  }

  async function triggerCheck() {
    if (!project) return;
    triggerLoading = true;
    try {
      await invoke<MaintainerRunLog>("trigger_maintainer_check", { projectId: project.id });
      runLogs = await invoke<MaintainerRunLog[]>("get_maintainer_history", { projectId: project.id });
      showToast("Maintainer check complete", "info");
    } catch (e) {
      showToast(String(e), "error");
    } finally {
      triggerLoading = false;
    }
  }

  async function clearRunLogs() {
    if (!project) return;
    try {
      await invoke("clear_maintainer_reports", { projectId: project.id });
      runLogs = [];
      openLogIndex = null;
      selectedIndex = 0;
      showToast("Maintainer logs cleared", "info");
    } catch (e) {
      showToast(String(e), "error");
    }
  }

  let nextRunText = $state("");

  function computeNextRunText(): string {
    if (!project?.maintainer.enabled) return "--";
    if (runLogs.length === 0) return "--";
    const lastRun = new Date(runLogs[0].timestamp).getTime();
    const intervalMs = project.maintainer.interval_minutes * 60 * 1000;
    const nextRun = lastRun + intervalMs;
    const diffMs = nextRun - Date.now();
    if (diffMs <= 0) return "0:00";
    const totalSecs = Math.floor(diffMs / 1000);
    const mins = Math.floor(totalSecs / 60);
    const secs = totalSecs % 60;
    const secsStr = secs.toString().padStart(2, "0");
    return mins > 0 ? `${mins}:${secsStr}` : `0:${secsStr}`;
  }

  $effect(() => {
    nextRunText = computeNextRunText();
    const id = setInterval(() => { nextRunText = computeNextRunText(); }, 1_000);
    return () => clearInterval(id);
  });

  const maintainerStatusesState = fromStore(maintainerStatuses);
  let maintainerStatus: MaintainerStatus | null = $derived(
    project ? (maintainerStatusesState.current.get(project.id) ?? null) : null
  );

  const maintainerErrorsState = fromStore(maintainerErrors);
  let maintainerError: string | null = $derived(
    project ? (maintainerErrorsState.current.get(project.id) ?? null) : null
  );

  let maintainerStatusText = $derived(
    !project?.maintainer.enabled ? "off"
      : maintainerStatus === "running" ? "running"
      : maintainerStatus === "error" ? "error"
      : "pending"
  );

  const autoWorkerStatusesState = fromStore(autoWorkerStatuses);
  let autoWorkerStatus: AutoWorkerStatus | null = $derived(
    project ? (autoWorkerStatusesState.current.get(project.id) ?? null) : null
  );

  function formatTimestamp(ts: string): string {
    return new Date(ts).toLocaleString();
  }

  function actionColor(action: string): string {
    return action === "filed" ? "#a6e3a1" : "#89b4fa";
  }
</script>

<div class="dashboard" class:panel-focused={panelFocused}>
  {#if !focusedAgent || !project}
    <div class="empty-state">
      <div class="empty-title">No agent selected</div>
      <div class="empty-hint">Navigate to an agent with <kbd>j</kbd> / <kbd>k</kbd> and press <kbd>l</kbd></div>
    </div>
  {:else if focusedAgent.agentKind === "auto-worker"}
    <div class="dashboard-header">
      <h2>{project.name}</h2>
      <span class="header-subtitle">Auto-worker</span>
    </div>
    <section class="section">
      <div class="section-header">
        <span class="section-title">Auto-worker</span>
        <span class="badge" class:enabled={project.auto_worker.enabled}>
          {project.auto_worker.enabled ? "ON" : "OFF"}
        </span>
        {#if autoWorkerStatus?.status === "working"}
          <span class="status-running">Working</span>
        {/if}
      </div>
      <div class="section-body">
        {#if !project.auto_worker.enabled}
          <p class="muted">Disabled — press <kbd>o</kbd> to enable</p>
        {:else if autoWorkerStatus?.status === "working"}
          <div class="worker-info">
            <span class="worker-label">Working on:</span>
            <span class="worker-issue">#{autoWorkerStatus.issue_number} {autoWorkerStatus.issue_title}</span>
          </div>
        {:else}
          <p class="muted">Waiting for eligible issues</p>
        {/if}
      </div>
    </section>
    <section class="section">
      <div class="section-header">
        <span class="section-title">Work policy</span>
      </div>
      <div class="policy-body">
        <div class="policy-row">
          <span class="policy-label">Required</span>
          <div class="policy-labels">
            <span class="policy-tag required">priority: high</span>
            <span class="policy-tag required">complexity: low</span>
            <span class="policy-tag required">triaged</span>
          </div>
        </div>
        <div class="policy-row">
          <span class="policy-label">Excluded</span>
          <div class="policy-labels">
            <span class="policy-tag excluded">in-progress</span>
            <span class="policy-tag excluded">finished-by-worker</span>
          </div>
        </div>
      </div>
    </section>
  {:else if focusedAgent.agentKind === "maintainer"}
    <div class="dashboard-header">
      <h2>{project.name}</h2>
      <span class="header-subtitle">Maintainer</span>
    </div>
    <section class="section">
      <div class="section-header">
        <span class="section-title">Maintainer</span>
        <span class="badge" class:enabled={project.maintainer.enabled}>
          {project.maintainer.enabled ? "ON" : "OFF"}
        </span>
        {#if maintainerStatus === "running"}
          <span class="maintainer-status running">running</span>
        {:else if maintainerStatus === "error"}
          <span class="maintainer-status error">error</span>
        {/if}
      </div>

      <div class="schedule-row">
        <span>Interval: {project.maintainer.interval_minutes}m</span>
        <span>Timer: {nextRunText}</span>
        <span>Status: {maintainerStatusText}</span>
      </div>

      {#if maintainerError}
        <div class="error-banner">
          <span class="error-label">Error</span>
          <span class="error-message">{maintainerError}</span>
        </div>
      {/if}
    </section>

    <section class="section report-section">
      {#if loading}
        <div class="section-body">
          <p class="muted">Loading...</p>
        </div>
      {:else if openLog}
        <div class="detail-view">
          <div class="detail-header">
            <span class="detail-back">Run logs</span>
            <span class="detail-timestamp">{formatTimestamp(openLog.timestamp)}</span>
            <span class="detail-summary">{openLog.summary}</span>
          </div>
          <div class="detail-blocks">
            <div
              class="detail-block"
              class:block-focused={panelFocused && detailBlockIndex === 0}
              data-block-index="0"
            >
              <div class="run-summary">
                <span class="summary-stat">{openLog.issues_filed.length} filed</span>
                <span class="summary-stat">{openLog.issues_updated.length} updated</span>
                <span class="summary-stat">{openLog.issues_unchanged} unchanged</span>
              </div>
            </div>
            {#each openLogIssues as issue, i}
              <div
                class="detail-block"
                class:block-focused={panelFocused && detailBlockIndex === i + 1}
                data-block-index={i + 1}
              >
                <div class="issue-item">
                  <span class="issue-action" style="color: {actionColor(issue.action)}">{issue.action}</span>
                  <span class="issue-number">#{issue.issue_number}</span>
                  <span class="issue-title">{issue.title}</span>
                  <div class="issue-labels">
                    {#each issue.labels.filter(l => l !== "filed-by-maintainer") as label}
                      <span class="issue-label">{label}</span>
                    {/each}
                  </div>
                </div>
              </div>
            {/each}
          </div>
        </div>
      {:else}
        <div class="report-list">
          {#if runLogs.length === 0}
            <div class="section-body">
              <p class="muted">No run logs yet</p>
              {#if project.maintainer.enabled}
                <button class="btn" onclick={triggerCheck} disabled={triggerLoading}>
                  {triggerLoading ? "Running..." : "(r) Run check now"}
                </button>
              {/if}
            </div>
          {:else}
            {#each runLogs as log, i}
              <!-- svelte-ignore a11y_no_static_element_interactions -->
              <!-- svelte-ignore a11y_click_events_have_key_events -->
              <div
                class="report-item"
                class:selected={panelFocused && selectedIndex === i}
                data-report-index={i}
                onclick={() => { selectedIndex = i; openLogIndex = i; detailBlockIndex = 0; }}
              >
                <span class="log-dot"></span>
                <span class="report-timestamp">{formatTimestamp(log.timestamp)}</span>
                <span class="report-summary-preview">{log.summary}</span>
              </div>
            {/each}
          {/if}
        </div>
      {/if}
    </section>

    {#if !panelFocused}
      <div class="panel-hint">
        <span class="muted">Press <kbd>l</kbd> to browse run logs</span>
      </div>
    {/if}
  {/if}
</div>

<style>
  .dashboard { width: 100%; height: 100%; overflow-y: auto; background: #11111b; color: #cdd6f4; outline: 2px solid transparent; outline-offset: -2px; transition: outline-color 0.15s; }
  .dashboard.panel-focused { outline-color: rgba(137, 180, 250, 0.4); border-radius: 4px; }
  .empty-state { display: flex; flex-direction: column; align-items: center; justify-content: center; height: 100%; gap: 8px; }
  .empty-title { font-size: 16px; font-weight: 500; }
  .empty-hint { color: #6c7086; font-size: 13px; }
  .empty-hint kbd, .muted kbd, .panel-hint kbd { background: #313244; color: #89b4fa; padding: 1px 6px; border-radius: 3px; font-family: monospace; font-size: 12px; }
  .dashboard-header { padding: 16px 24px; border-bottom: 1px solid #313244; display: flex; align-items: baseline; }
  .dashboard-header h2 { font-size: 16px; font-weight: 600; margin: 0; }
  .header-subtitle { font-size: 12px; color: #6c7086; margin-left: 8px; }
  .section { border-bottom: 1px solid #313244; }
  .section-header { padding: 12px 24px; display: flex; align-items: center; gap: 8px; border-bottom: 1px solid rgba(49, 50, 68, 0.5); }
  .section-title { font-size: 13px; font-weight: 600; flex: 1; }
  .badge { font-size: 10px; padding: 1px 6px; border-radius: 3px; background: #313244; color: #6c7086; }
  .badge.enabled { background: rgba(166, 227, 161, 0.2); color: #a6e3a1; }
  .status-running { font-size: 11px; color: #89b4fa; }
  .schedule-row { padding: 8px 24px; display: flex; justify-content: space-between; font-size: 11px; color: #6c7086; border-bottom: 1px solid rgba(49, 50, 68, 0.5); }
  .section-body { padding: 16px 24px; }
  .muted { color: #6c7086; font-size: 13px; margin: 0; }
  .worker-info { display: flex; flex-direction: column; gap: 4px; }
  .worker-label { color: #6c7086; font-size: 11px; }
  .worker-issue { font-size: 13px; }
  .maintainer-status { font-size: 11px; font-weight: 500; text-transform: capitalize; }
  .maintainer-status.running { color: #89b4fa; }
  .maintainer-status.error { color: #f38ba8; }
  .btn { background: #313244; border: none; color: #cdd6f4; padding: 6px 12px; border-radius: 4px; font-size: 12px; cursor: pointer; box-shadow: none; margin-top: 8px; }
  .btn:hover { background: #45475a; }
  .btn:disabled { opacity: 0.5; cursor: not-allowed; }

  /* Run log list */
  .report-section { border-bottom: none; flex: 1; }
  .report-list { display: flex; flex-direction: column; }
  .report-item {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 10px 24px;
    cursor: pointer;
    font-size: 12px;
    border-bottom: 1px solid rgba(49, 50, 68, 0.3);
  }
  .report-item:hover { background: rgba(49, 50, 68, 0.3); }
  .report-item.selected {
    background: rgba(137, 180, 250, 0.1);
    outline: 1px solid rgba(137, 180, 250, 0.4);
    outline-offset: -1px;
  }
  .log-dot { width: 8px; height: 8px; border-radius: 50%; flex-shrink: 0; background: #89b4fa; }
  .report-timestamp { color: #6c7086; font-size: 11px; white-space: nowrap; flex-shrink: 0; }
  .report-summary-preview { flex: 1; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; color: #bac2de; }

  /* Detail view */
  .detail-view { display: flex; flex-direction: column; }
  .detail-header {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 10px 24px;
    border-bottom: 1px solid rgba(49, 50, 68, 0.5);
    font-size: 12px;
  }
  .detail-back { color: #6c7086; }
  .detail-timestamp { color: #6c7086; font-size: 11px; }
  .detail-summary { font-size: 11px; color: #bac2de; margin-left: auto; }
  .detail-blocks { padding: 12px 24px; display: flex; flex-direction: column; gap: 8px; }
  .detail-block { border-radius: 6px; transition: outline-color 0.15s; outline: 2px solid transparent; outline-offset: 2px; }
  .detail-block.block-focused { outline-color: rgba(137, 180, 250, 0.5); }

  .run-summary { padding: 12px; border-radius: 6px; background: rgba(49, 50, 68, 0.3); display: flex; gap: 16px; border-left: 3px solid #89b4fa; }
  .summary-stat { font-size: 13px; color: #cdd6f4; }

  .issue-item { padding: 8px 12px; background: rgba(49, 50, 68, 0.2); border-radius: 4px; font-size: 12px; display: flex; flex-direction: column; gap: 2px; }
  .issue-action { font-weight: 600; font-size: 11px; text-transform: uppercase; }
  .issue-number { color: #6c7086; font-size: 11px; }
  .issue-title { color: #cdd6f4; }
  .issue-labels { display: flex; gap: 4px; flex-wrap: wrap; margin-top: 2px; }
  .issue-label { font-size: 10px; padding: 1px 6px; border-radius: 3px; background: #313244; color: #6c7086; }

  .policy-body { padding: 12px 24px; display: flex; flex-direction: column; gap: 8px; }
  .policy-row { display: flex; align-items: baseline; gap: 10px; }
  .policy-label { font-size: 11px; color: #6c7086; width: 60px; flex-shrink: 0; }
  .policy-labels { display: flex; gap: 4px; flex-wrap: wrap; }
  .policy-tag { font-size: 10px; padding: 1px 6px; border-radius: 3px; }
  .policy-tag.required { background: rgba(166, 227, 161, 0.15); color: #a6e3a1; }
  .policy-tag.excluded { background: rgba(243, 139, 168, 0.15); color: #f38ba8; }

  .error-banner { padding: 8px 24px; background: rgba(243, 139, 168, 0.1); border-bottom: 1px solid rgba(243, 139, 168, 0.2); display: flex; align-items: baseline; gap: 8px; font-size: 12px; }
  .error-label { color: #f38ba8; font-weight: 600; font-size: 11px; text-transform: uppercase; flex-shrink: 0; }
  .error-message { color: #bac2de; word-break: break-word; }

  .panel-hint { padding: 12px 24px; }
</style>
