<script lang="ts">
  import { fromStore } from "svelte/store";
  import { invoke } from "@tauri-apps/api/core";
  import { focusTarget, projects, maintainerStatuses, autoWorkerStatuses, hotkeyAction, type Project, type FocusTarget, type MaintainerReport, type MaintainerStatus, type AutoWorkerStatus } from "./stores";
  import { showToast } from "./toast";

  let reports: MaintainerReport[] = $state([]);
  let loading = $state(false);
  let triggerLoading = $state(false);
  let currentProjectId: string | null = $state(null);

  // Panel navigation state
  let selectedIndex = $state(0);
  let openReportIndex: number | null = $state(null);
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

  let openReport = $derived(
    openReportIndex !== null ? reports[openReportIndex] ?? null : null
  );

  // Fetch history when project changes
  $effect(() => {
    const pid = project?.id ?? null;
    if (pid && pid !== currentProjectId) {
      currentProjectId = pid;
      if (focusedAgent?.agentKind === "maintainer") {
        fetchHistory(pid);
      }
    }
  });

  // Reset panel state when switching agents
  let prevAgentKey: string | null = $state(null);
  $effect(() => {
    const key = focusedAgent ? `${focusedAgent.projectId}:${focusedAgent.agentKind}` : null;
    if (key !== prevAgentKey) {
      prevAgentKey = key;
      selectedIndex = 0;
      openReportIndex = null;
      detailBlockIndex = 0;
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
        clearReports();
      }
    });
    return unsub;
  });

  function handleNavigate(direction: 1 | -1) {
    if (focusedAgent?.agentKind !== "maintainer") return;

    if (openReportIndex !== null) {
      // Detail view: scroll through blocks
      const report = reports[openReportIndex];
      if (!report) return;
      const maxBlock = report.findings.length; // 0 = summary, 1..N = findings
      detailBlockIndex = Math.max(0, Math.min(maxBlock, detailBlockIndex + direction));
      scrollBlockIntoView();
    } else {
      // List view: move selection
      if (reports.length === 0) return;
      selectedIndex = Math.max(0, Math.min(reports.length - 1, selectedIndex + direction));
      scrollReportIntoView();
    }
  }

  function handleSelect() {
    if (focusedAgent?.agentKind !== "maintainer") return;
    if (openReportIndex !== null) return;
    if (reports.length === 0) return;
    openReportIndex = selectedIndex;
    detailBlockIndex = 0;
  }

  function handleEscape() {
    if (openReportIndex !== null) {
      openReportIndex = null;
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
      reports = await invoke<MaintainerReport[]>("get_maintainer_history", { projectId });
    } catch {
      reports = [];
    } finally {
      loading = false;
    }
  }

  async function triggerCheck() {
    if (!project) return;
    triggerLoading = true;
    try {
      await invoke<MaintainerReport>("trigger_maintainer_check", { projectId: project.id });
      reports = await invoke<MaintainerReport[]>("get_maintainer_history", { projectId: project.id });
      showToast("Maintainer check complete", "info");
    } catch (e) {
      showToast(String(e), "error");
    } finally {
      triggerLoading = false;
    }
  }

  async function clearReports() {
    if (!project) return;
    try {
      await invoke("clear_maintainer_reports", { projectId: project.id });
      reports = [];
      openReportIndex = null;
      selectedIndex = 0;
      showToast("Maintainer reports cleared", "info");
    } catch (e) {
      showToast(String(e), "error");
    }
  }

  let nextRunText = $state("");

  function computeNextRunText(): string {
    if (!project?.maintainer.enabled) return "Disabled";
    if (reports.length === 0) return "Pending";
    const lastRun = new Date(reports[0].timestamp).getTime();
    const intervalMs = project.maintainer.interval_minutes * 60 * 1000;
    const nextRun = lastRun + intervalMs;
    const diffMs = nextRun - Date.now();
    if (diffMs <= 0) return "Due now";
    const totalSecs = Math.floor(diffMs / 1000);
    const mins = Math.floor(totalSecs / 60);
    const secs = totalSecs % 60;
    return mins > 0 ? `${mins}m ${secs}s` : `${secs}s`;
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

  const autoWorkerStatusesState = fromStore(autoWorkerStatuses);
  let autoWorkerStatus: AutoWorkerStatus | null = $derived(
    project ? (autoWorkerStatusesState.current.get(project.id) ?? null) : null
  );

  function severityColor(severity: string): string {
    switch (severity) {
      case "error": return "#f38ba8";
      case "warning": return "#f9e2af";
      default: return "#89b4fa";
    }
  }

  function actionLabel(action: MaintainerReport["findings"][0]["action_taken"]): string {
    if (action.type === "fixed") return "Auto-fixed";
    if (action.type === "reported") return "Reported";
    if (action.type === "pr_created") return "PR created";
    return "Unknown";
  }

  function statusColor(status: string): string {
    switch (status) {
      case "passing": return "#a6e3a1";
      case "warnings": return "#f9e2af";
      case "failing": return "#f38ba8";
      default: return "#6c7086";
    }
  }

  function formatTimestamp(ts: string): string {
    return new Date(ts).toLocaleString();
  }
</script>

<div class="dashboard">
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
        {#if maintainerStatus && maintainerStatus !== "idle"}
          <span class="maintainer-status" class:passing={maintainerStatus === "passing"} class:warnings={maintainerStatus === "warnings"} class:failing={maintainerStatus === "failing"} class:running={maintainerStatus === "running"}>
            {maintainerStatus}
          </span>
        {/if}
      </div>

      {#if project.maintainer.enabled}
        <div class="schedule-row">
          <span>Interval: {project.maintainer.interval_minutes}m</span>
          <span>Next: {nextRunText}</span>
        </div>
      {/if}
    </section>

    <section class="section report-section">
      {#if loading}
        <div class="section-body">
          <p class="muted">Loading...</p>
        </div>
      {:else if openReport}
        <div class="detail-view">
          <div class="detail-header">
            <span class="detail-back">Reports</span>
            <span class="detail-timestamp">{formatTimestamp(openReport.timestamp)}</span>
            <span class="detail-status" style="color: {statusColor(openReport.status)}">{openReport.status}</span>
          </div>
          <div class="detail-blocks">
            <div
              class="detail-block"
              class:block-focused={panelFocused && detailBlockIndex === 0}
              data-block-index="0"
            >
              <div class="report-summary" class:passing={openReport.status === "passing"} class:warnings={openReport.status === "warnings"} class:failing={openReport.status === "failing"}>
                <span class="summary-text">{openReport.summary}</span>
              </div>
            </div>
            {#each openReport.findings as finding, i}
              <div
                class="detail-block"
                class:block-focused={panelFocused && detailBlockIndex === i + 1}
                data-block-index={i + 1}
              >
                <div class="finding">
                  <span class="finding-severity" style="color: {severityColor(finding.severity)}">{finding.severity}</span>
                  <span class="finding-category">{finding.category}</span>
                  <span class="finding-desc">{finding.description}</span>
                  <span class="finding-action">{actionLabel(finding.action_taken)}</span>
                </div>
              </div>
            {/each}
          </div>
        </div>
      {:else}
        <div class="report-list">
          {#if reports.length === 0}
            <div class="section-body">
              <p class="muted">No reports yet</p>
              {#if project.maintainer.enabled}
                <button class="btn" onclick={triggerCheck} disabled={triggerLoading}>
                  {triggerLoading ? "Running..." : "(r) Run check now"}
                </button>
              {/if}
            </div>
          {:else}
            {#each reports as report, i}
              <!-- svelte-ignore a11y_no_static_element_interactions -->
              <!-- svelte-ignore a11y_click_events_have_key_events -->
              <div
                class="report-item"
                class:selected={panelFocused && selectedIndex === i}
                data-report-index={i}
                onclick={() => { selectedIndex = i; openReportIndex = i; detailBlockIndex = 0; }}
              >
                <span class="report-status-dot" style="background: {statusColor(report.status)}"></span>
                <span class="report-timestamp">{formatTimestamp(report.timestamp)}</span>
                <span class="report-summary-preview">{report.summary}</span>
              </div>
            {/each}
          {/if}
        </div>
      {/if}
    </section>

    {#if !panelFocused}
      <div class="panel-hint">
        <span class="muted">Press <kbd>l</kbd> to browse reports</span>
      </div>
    {/if}
  {/if}
</div>

<style>
  .dashboard { width: 100%; height: 100%; overflow-y: auto; background: #11111b; color: #cdd6f4; }
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
  .maintainer-status.passing { color: #a6e3a1; }
  .maintainer-status.warnings { color: #f9e2af; }
  .maintainer-status.failing { color: #f38ba8; }
  .maintainer-status.running { color: #89b4fa; }
  .btn { background: #313244; border: none; color: #cdd6f4; padding: 6px 12px; border-radius: 4px; font-size: 12px; cursor: pointer; box-shadow: none; margin-top: 8px; }
  .btn:hover { background: #45475a; }
  .btn:disabled { opacity: 0.5; cursor: not-allowed; }

  /* Report list */
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
  .report-status-dot { width: 8px; height: 8px; border-radius: 50%; flex-shrink: 0; }
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
  .detail-status { font-size: 11px; font-weight: 500; text-transform: capitalize; margin-left: auto; }
  .detail-blocks { padding: 12px 24px; display: flex; flex-direction: column; gap: 8px; }
  .detail-block { border-radius: 6px; transition: outline-color 0.15s; outline: 2px solid transparent; outline-offset: 2px; }
  .detail-block.block-focused { outline-color: rgba(137, 180, 250, 0.5); }

  .report-summary { padding: 12px; border-radius: 6px; background: rgba(49, 50, 68, 0.3); display: flex; flex-direction: column; gap: 4px; }
  .report-summary.passing { border-left: 3px solid #a6e3a1; }
  .report-summary.warnings { border-left: 3px solid #f9e2af; }
  .report-summary.failing { border-left: 3px solid #f38ba8; }
  .summary-text { font-size: 13px; }

  .finding { padding: 8px 12px; background: rgba(49, 50, 68, 0.2); border-radius: 4px; font-size: 12px; display: flex; flex-direction: column; gap: 2px; }
  .finding-severity { font-weight: 600; font-size: 11px; text-transform: uppercase; }
  .finding-category { color: #89b4fa; font-size: 11px; }
  .finding-desc { color: #cdd6f4; }
  .finding-action { color: #6c7086; font-size: 11px; font-style: italic; }

  .panel-hint { padding: 12px 24px; }
</style>
