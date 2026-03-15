<script lang="ts">
  import { fromStore } from "svelte/store";
  import { untrack } from "svelte";
  import { command } from "$lib/backend";
  import { openUrl } from "$lib/platform";
  import { focusTarget, projects, maintainerStatuses, maintainerErrors, autoWorkerStatuses, hotkeyAction, type Project, type FocusTarget, type MaintainerRunLog, type MaintainerStatus, type AutoWorkerStatus, type MaintainerIssue, type MaintainerIssueDetail, type WorkerReport, type AutoWorkerQueueIssue } from "./stores";
  import { showToast } from "./toast";

  let runLogs: MaintainerRunLog[] = $state([]);
  let loading = $state(false);
  let triggerLoading = $state(false);

  // Panel navigation state
  let selectedIndex = $state(0);
  let openLogIndex: number | null = $state(null);
  let detailBlockIndex = $state(0);

  // View mode: "runs" or "issues"
  type MaintainerViewMode = "runs" | "issues";
  let viewMode: MaintainerViewMode = $state("runs");

  // Issues view state
  let issuesList: MaintainerIssue[] = $state([]);
  let issuesLoading = $state(false);
  let issueDetail: MaintainerIssueDetail | null = $state(null);
  let issueDetailLoading = $state(false);
  let issueSelectedIndex = $state(0);

  let openIssues = $derived(issuesList.filter(i => i.state === "OPEN"));
  let closedIssues = $derived(issuesList.filter(i => i.state === "CLOSED"));
  // Flat list: open issues first, then closed
  let allSortedIssues = $derived([...openIssues, ...closedIssues]);

  // Worker report state
  type AutoWorkerViewMode = "queue" | "reports";
  let autoWorkerViewMode: AutoWorkerViewMode = $state("queue");
  let autoWorkerQueue: AutoWorkerQueueIssue[] = $state([]);
  let autoWorkerQueueLoading = $state(false);
  let autoWorkerQueueOpenIndex: number | null = $state(null);
  let autoWorkerQueueSelectedIndex = $state(0);
  let workerReports: WorkerReport[] = $state([]);
  let workerLoading = $state(false);
  let workerOpenIndex: number | null = $state(null);
  let workerSelectedIndex = $state(0);
  let prevAutoWorkerStatusKey: string | null = $state(null);

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

  let openAutoWorkerIssue = $derived(
    autoWorkerQueueOpenIndex !== null ? autoWorkerQueue[autoWorkerQueueOpenIndex] ?? null : null
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
      issuesList = [];
      issueDetail = null;
      issueSelectedIndex = 0;
      autoWorkerViewMode = "queue";
      autoWorkerQueue = [];
      autoWorkerQueueLoading = false;
      autoWorkerQueueOpenIndex = null;
      autoWorkerQueueSelectedIndex = 0;
      workerReports = [];
      workerOpenIndex = null;
      workerSelectedIndex = 0;
      prevAutoWorkerStatusKey = null;

      const pid = untrack(() => project?.id ?? null);
      if (pid && focusedAgent?.agentKind === "maintainer") {
        fetchHistory(pid);
      }
      if (pid && focusedAgent?.agentKind === "auto-worker") {
        fetchWorkerReports(pid);
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
      } else if (action.type === "toggle-maintainer-view") {
        toggleViewMode();
      } else if (action.type === "open-issue-in-browser") {
        openSelectedIssueInBrowser();
      }
    });
    return unsub;
  });

  function toggleViewMode() {
    if (focusedAgent?.agentKind === "auto-worker") {
      if (autoWorkerViewMode === "queue") {
        autoWorkerViewMode = "reports";
      } else {
        autoWorkerViewMode = "queue";
        if (project) fetchAutoWorkerQueue(project.id);
      }
      return;
    }

    if (focusedAgent?.agentKind === "maintainer") {
      if (viewMode === "runs") {
        viewMode = "issues";
        issueDetail = null;
        issueSelectedIndex = 0;
        if (project) fetchIssues(project.id);
      } else {
        viewMode = "runs";
      }
    }
  }

  async function fetchIssues(projectId: string) {
    issuesLoading = true;
    try {
      const result = await command<MaintainerIssue[]>("get_maintainer_issues", { projectId });
      issuesList = result;
    } catch {
      issuesList = [];
    } finally {
      issuesLoading = false;
    }
  }

  async function fetchIssueDetail(projectId: string, issueNumber: number) {
    issueDetailLoading = true;
    try {
      const result = await command<MaintainerIssueDetail>("get_maintainer_issue_detail", { projectId, issueNumber });
      issueDetail = result;
    } catch (e) {
      showToast(String(e), "error");
      issueDetail = null;
    } finally {
      issueDetailLoading = false;
    }
  }

  function refreshIssues() {
    if (project) fetchIssues(project.id);
  }

  function openSelectedIssueInBrowser() {
    if (focusedAgent?.agentKind === "auto-worker") {
      if (autoWorkerViewMode !== "queue") return;
      if (openAutoWorkerIssue) {
        openUrl(openAutoWorkerIssue.url);
      } else if (autoWorkerQueue.length > 0) {
        const issue = autoWorkerQueue[autoWorkerQueueSelectedIndex];
        if (issue) openUrl(issue.url);
      }
      return;
    }

    if (focusedAgent?.agentKind !== "maintainer") return;
    if (viewMode === "issues") {
      if (issueDetail) {
        openUrl(issueDetail.url);
      } else if (allSortedIssues.length > 0) {
        const issue = allSortedIssues[issueSelectedIndex];
        if (issue) openUrl(issue.url);
      }
    } else if (viewMode === "runs" && openLog && openLogIssues.length > 0 && detailBlockIndex > 0) {
      const issue = openLogIssues[detailBlockIndex - 1];
      if (issue) openUrl(issue.url);
    }
  }

  function handleNavigate(direction: 1 | -1) {
    if (focusedAgent?.agentKind === "auto-worker") {
      if (autoWorkerViewMode === "queue") {
        if (autoWorkerQueueOpenIndex !== null) return;
        if (autoWorkerQueue.length === 0) return;
        autoWorkerQueueSelectedIndex = Math.max(0, Math.min(autoWorkerQueue.length - 1, autoWorkerQueueSelectedIndex + direction));
        scrollAutoWorkerQueueIntoView();
      } else {
        if (workerOpenIndex !== null) return;
        if (workerReports.length === 0) return;
        workerSelectedIndex = Math.max(0, Math.min(workerReports.length - 1, workerSelectedIndex + direction));
        scrollWorkerReportIntoView();
      }
      return;
    }

    if (focusedAgent?.agentKind !== "maintainer") return;

    if (viewMode === "issues") {
      if (issueDetail) {
        // In detail popup, no navigation; just scroll
        return;
      }
      if (allSortedIssues.length === 0) return;
      issueSelectedIndex = Math.max(0, Math.min(allSortedIssues.length - 1, issueSelectedIndex + direction));
      scrollIssueIntoView();
      return;
    }

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
    if (focusedAgent?.agentKind === "auto-worker") {
      if (autoWorkerViewMode === "queue") {
        if (autoWorkerQueueOpenIndex !== null) return;
        if (autoWorkerQueue.length === 0) return;
        autoWorkerQueueOpenIndex = autoWorkerQueueSelectedIndex;
      } else {
        if (workerOpenIndex !== null) return;
        if (workerReports.length === 0) return;
        workerOpenIndex = workerSelectedIndex;
      }
      return;
    }
    if (focusedAgent?.agentKind !== "maintainer") return;

    if (viewMode === "issues") {
      if (issueDetail) return; // already viewing detail
      if (allSortedIssues.length === 0) return;
      const issue = allSortedIssues[issueSelectedIndex];
      if (issue && project) {
        fetchIssueDetail(project.id, issue.number);
      }
      return;
    }

    if (openLogIndex !== null) return;
    if (runLogs.length === 0) return;
    openLogIndex = selectedIndex;
    detailBlockIndex = 0;
  }

  function handleEscape() {
    if (focusedAgent?.agentKind === "auto-worker") {
      if (autoWorkerViewMode === "queue" && autoWorkerQueueOpenIndex !== null) {
        autoWorkerQueueOpenIndex = null;
        return;
      }
      if (autoWorkerViewMode === "reports" && workerOpenIndex !== null) {
        workerOpenIndex = null;
        return;
      }
      if (focusedAgent) {
        focusTarget.set({ type: "agent", agentKind: focusedAgent.agentKind, projectId: focusedAgent.projectId });
      }
      return;
    }
    if (viewMode === "issues") {
      if (issueDetail) {
        issueDetail = null;
        return;
      }
      // Fall through to default escape behavior
    }

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

  function scrollIssueIntoView() {
    requestAnimationFrame(() => {
      const el = document.querySelector(`[data-issue-index="${issueSelectedIndex}"]`);
      if (el) el.scrollIntoView({ behavior: "smooth", block: "nearest" });
    });
  }

  function scrollWorkerReportIntoView() {
    requestAnimationFrame(() => {
      const el = document.querySelector(`[data-worker-report-index="${workerSelectedIndex}"]`);
      if (el) el.scrollIntoView({ behavior: "smooth", block: "nearest" });
    });
  }

  function scrollAutoWorkerQueueIntoView() {
    requestAnimationFrame(() => {
      const el = document.querySelector(`[data-worker-queue-index="${autoWorkerQueueSelectedIndex}"]`);
      if (el) el.scrollIntoView({ behavior: "smooth", block: "nearest" });
    });
  }

  async function fetchAutoWorkerQueue(projectId: string) {
    autoWorkerQueueLoading = true;
    try {
      const result = await command<AutoWorkerQueueIssue[]>("get_auto_worker_queue", { projectId });
      if (prevAgentKey === `${projectId}:auto-worker`) {
        const selectedNumber = autoWorkerQueue[autoWorkerQueueSelectedIndex]?.number ?? null;
        const openNumber = autoWorkerQueueOpenIndex !== null ? autoWorkerQueue[autoWorkerQueueOpenIndex]?.number ?? null : null;
        autoWorkerQueue = result;
        const nextSelectedIndex = selectedNumber === null ? 0 : result.findIndex((issue) => issue.number === selectedNumber);
        autoWorkerQueueSelectedIndex = nextSelectedIndex >= 0 ? nextSelectedIndex : 0;
        if (openNumber === null) {
          autoWorkerQueueOpenIndex = null;
        } else {
          const nextOpenIndex = result.findIndex((issue) => issue.number === openNumber);
          autoWorkerQueueOpenIndex = nextOpenIndex >= 0 ? nextOpenIndex : null;
        }
      }
    } catch (e) {
      if (prevAgentKey === `${projectId}:auto-worker`) {
        autoWorkerQueue = [];
        autoWorkerQueueOpenIndex = null;
      }
      showToast(String(e), "error");
    } finally {
      if (prevAgentKey === `${projectId}:auto-worker`) {
        autoWorkerQueueLoading = false;
      }
    }
  }

  async function fetchHistory(projectId: string) {
    loading = true;
    try {
      const result = await command<MaintainerRunLog[]>("get_maintainer_history", { projectId });
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

  async function fetchWorkerReports(projectId: string) {
    const proj = projectList.find((p) => p.id === projectId);
    if (!proj) return;
    workerLoading = true;
    try {
      const result = await command<WorkerReport[]>("get_worker_reports", { repoPath: proj.repo_path });
      if (prevAgentKey === `${projectId}:auto-worker`) {
        workerReports = result;
      }
    } catch {
      if (prevAgentKey === `${projectId}:auto-worker`) {
        workerReports = [];
      }
    } finally {
      if (prevAgentKey === `${projectId}:auto-worker`) {
        workerLoading = false;
      }
    }
  }

  async function triggerCheck() {
    if (!project) return;
    triggerLoading = true;
    try {
      await command<MaintainerRunLog>("trigger_maintainer_check", { projectId: project.id });
      runLogs = await command<MaintainerRunLog[]>("get_maintainer_history", { projectId: project.id });
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
      await command("clear_maintainer_reports", { projectId: project.id });
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

  function formatDate(ts: string): string {
    return new Date(ts).toLocaleDateString();
  }

  function actionColor(action: string): string {
    return action === "filed" ? "#a6e3a1" : "#89b4fa";
  }

  function stateColor(state: string): string {
    return state === "OPEN" ? "#a6e3a1" : "#cba6f7";
  }

  $effect(() => {
    if (focusedAgent?.agentKind !== "auto-worker" || !project) {
      prevAutoWorkerStatusKey = null;
      return;
    }

    const statusKey = `${project.id}:${autoWorkerStatus?.status ?? "idle"}:${autoWorkerStatus?.issue_number ?? ""}:${autoWorkerStatus?.issue_title ?? ""}`;
    if (statusKey === prevAutoWorkerStatusKey) return;
    prevAutoWorkerStatusKey = statusKey;
    fetchAutoWorkerQueue(project.id);
  });
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
            <span class="policy-tag required">priority:high</span>
            <span class="policy-tag required">complexity:low</span>
          </div>
        </div>
        <div class="policy-row">
          <span class="policy-label">Excluded</span>
          <div class="policy-labels">
            <span class="policy-tag excluded">in-progress</span>
            <span class="policy-tag excluded">assigned-to-auto-worker</span>
          </div>
        </div>
      </div>
    </section>

    <div class="view-tabs">
      <!-- svelte-ignore a11y_click_events_have_key_events -->
      <!-- svelte-ignore a11y_no_static_element_interactions -->
      <span class="view-tab" class:active={autoWorkerViewMode === "queue"} onclick={() => { autoWorkerViewMode = "queue"; if (project) fetchAutoWorkerQueue(project.id); }}>Queue</span>
      <!-- svelte-ignore a11y_click_events_have_key_events -->
      <!-- svelte-ignore a11y_no_static_element_interactions -->
      <span class="view-tab" class:active={autoWorkerViewMode === "reports"} onclick={() => { autoWorkerViewMode = "reports"; }}>Reports</span>
      <span class="view-tab-hint">(t) toggle</span>
    </div>

    {#if autoWorkerViewMode === "queue"}
      <section class="section report-section">
        {#if autoWorkerQueueLoading}
          <div class="section-body">
            <p class="muted">Loading eligible issues...</p>
          </div>
        {:else if openAutoWorkerIssue}
          <div class="issue-detail-popup">
            <div class="issue-detail-header">
              <span class="issue-detail-number">#{openAutoWorkerIssue.number}</span>
              <span class={`queue-state${openAutoWorkerIssue.is_active ? " worker-queue-state" : ""}`}>
                {openAutoWorkerIssue.is_active ? "WORKING" : "QUEUED"}
              </span>
              <span class="issue-detail-hint">(o) open in browser / (Esc) back</span>
            </div>
            <h3 class="issue-detail-title">{openAutoWorkerIssue.title}</h3>
            {#if openAutoWorkerIssue.labels.length > 0}
              <div class="issue-detail-labels">
                {#each openAutoWorkerIssue.labels as label}
                  <span class="issue-label">{label}</span>
                {/each}
              </div>
            {/if}
            {#if openAutoWorkerIssue.body}
              <div class="issue-detail-body">{openAutoWorkerIssue.body}</div>
            {/if}
          </div>
        {:else}
          <div class="issues-list">
            {#if autoWorkerQueue.length === 0}
              <div class="section-body">
                <p class="muted">No eligible issues</p>
              </div>
            {:else}
              {#each autoWorkerQueue as issue, i}
                <!-- svelte-ignore a11y_no_static_element_interactions -->
                <!-- svelte-ignore a11y_click_events_have_key_events -->
                <div
                  class="issues-item"
                  class:selected={panelFocused && autoWorkerQueueSelectedIndex === i}
                  data-worker-queue-index={i}
                  onclick={() => { autoWorkerQueueSelectedIndex = i; autoWorkerQueueOpenIndex = i; }}
                >
                  <span class="issues-state" class:open-dot={!issue.is_active} class:working-dot={issue.is_active}></span>
                  <span class="queue-summary">#{issue.number} {issue.title}</span>
                  {#if issue.is_active}
                    <span class="queue-state worker-queue-state">Working</span>
                  {:else}
                    <span class="queue-state">Queued</span>
                  {/if}
                </div>
              {/each}
            {/if}
          </div>
        {/if}
      </section>
    {:else}
      <section class="section report-section">
        {#if workerLoading}
          <div class="section-body">
            <p class="muted">Loading reports...</p>
          </div>
        {:else if workerOpenIndex !== null && workerReports[workerOpenIndex]}
          {@const report = workerReports[workerOpenIndex]}
          <div class="detail-view">
            <div class="detail-header">
              <span class="detail-back">Reports</span>
              <span class="detail-timestamp">{formatTimestamp(report.updated_at)}</span>
              <span class="detail-summary">#{report.issue_number} {report.title}</span>
            </div>
            <div class="detail-blocks">
              <div class="detail-block">
                <div class="worker-report-body">{report.comment_body}</div>
              </div>
            </div>
          </div>
        {:else}
          <div class="report-list">
            {#if workerReports.length === 0}
              <div class="section-body">
                <p class="muted">No completed work yet</p>
              </div>
            {:else}
              {#each workerReports as report, i}
                <!-- svelte-ignore a11y_no_static_element_interactions -->
                <!-- svelte-ignore a11y_click_events_have_key_events -->
                <div
                  class="report-item"
                  class:selected={panelFocused && workerSelectedIndex === i}
                  data-worker-report-index={i}
                  onclick={() => { workerSelectedIndex = i; workerOpenIndex = i; }}
                >
                  <span class="log-dot"></span>
                  <span class="report-timestamp">{formatTimestamp(report.updated_at)}</span>
                  <span class="report-summary-preview">#{report.issue_number} {report.title}</span>
                </div>
              {/each}
            {/if}
          </div>
        {/if}
      </section>
    {/if}

    {#if !panelFocused}
      <div class="panel-hint">
        <span class="muted">Press <kbd>l</kbd> to browse {autoWorkerViewMode === "queue" ? "eligible issues" : "reports"}</span>
      </div>
    {/if}
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

    <!-- View toggle tabs -->
    <div class="view-tabs">
      <!-- svelte-ignore a11y_click_events_have_key_events -->
      <!-- svelte-ignore a11y_no_static_element_interactions -->
      <span class="view-tab" class:active={viewMode === "runs"} onclick={() => { viewMode = "runs"; }}>Runs</span>
      <!-- svelte-ignore a11y_click_events_have_key_events -->
      <!-- svelte-ignore a11y_no_static_element_interactions -->
      <span class="view-tab" class:active={viewMode === "issues"} onclick={() => { viewMode = "issues"; issueDetail = null; issueSelectedIndex = 0; if (project) fetchIssues(project.id); }}>Issues</span>
      <span class="view-tab-hint">(t) toggle</span>
    </div>

    {#if viewMode === "runs"}
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
                  {#if openLog.issues_skipped > 0}
                    <span class="summary-stat skipped">{openLog.issues_skipped} skipped (closed)</span>
                  {/if}
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
    {:else}
      <!-- Issues view -->
      <section class="section report-section">
        {#if issuesLoading}
          <div class="section-body">
            <p class="muted">Loading issues...</p>
          </div>
        {:else if issueDetail}
          <div class="issue-detail-popup">
            <div class="issue-detail-header">
              <span class="issue-detail-number">#{issueDetail.number}</span>
              <span class="issue-detail-state" style="color: {stateColor(issueDetail.state)}">{issueDetail.state}</span>
              <span class="issue-detail-hint">(o) open in browser / (Esc) back</span>
            </div>
            <h3 class="issue-detail-title">{issueDetail.title}</h3>
            <div class="issue-detail-meta">
              <span>Created: {formatDate(issueDetail.createdAt)}</span>
              {#if issueDetail.closedAt}
                <span>Closed: {formatDate(issueDetail.closedAt)}</span>
              {/if}
            </div>
            {#if issueDetail.labels.length > 0}
              <div class="issue-detail-labels">
                {#each issueDetail.labels.filter(l => l.name !== "filed-by-maintainer") as label}
                  <span class="issue-label">{label.name}</span>
                {/each}
              </div>
            {/if}
            {#if issueDetail.body}
              <div class="issue-detail-body">{issueDetail.body}</div>
            {/if}
          </div>
        {:else}
          <div class="issues-list">
            {#if allSortedIssues.length === 0}
              <div class="section-body">
                <p class="muted">No maintainer issues found</p>
                <button class="btn" onclick={refreshIssues}>Refresh</button>
              </div>
            {:else}
              {#if openIssues.length > 0}
                <div class="issues-section-label">Open ({openIssues.length})</div>
              {/if}
              {#each openIssues as issue, i}
                <!-- svelte-ignore a11y_no_static_element_interactions -->
                <!-- svelte-ignore a11y_click_events_have_key_events -->
                <div
                  class="issues-item"
                  class:selected={panelFocused && issueSelectedIndex === i}
                  data-issue-index={i}
                  onclick={() => { issueSelectedIndex = i; if (project) fetchIssueDetail(project.id, issue.number); }}
                >
                  <span class="issues-state open-dot"></span>
                  <span class="issues-number">#{issue.number}</span>
                  <span class="issues-title">{issue.title}</span>
                  <div class="issues-item-labels">
                    {#each issue.labels.filter(l => l.name !== "filed-by-maintainer") as label}
                      <span class="issue-label">{label.name}</span>
                    {/each}
                  </div>
                  <span class="issues-date">{formatDate(issue.createdAt)}</span>
                </div>
              {/each}
              {#if closedIssues.length > 0}
                <div class="issues-section-label">Closed ({closedIssues.length})</div>
              {/if}
              {#each closedIssues as issue, ci}
                {@const idx = openIssues.length + ci}
                <!-- svelte-ignore a11y_no_static_element_interactions -->
                <!-- svelte-ignore a11y_click_events_have_key_events -->
                <div
                  class="issues-item closed"
                  class:selected={panelFocused && issueSelectedIndex === idx}
                  data-issue-index={idx}
                  onclick={() => { issueSelectedIndex = idx; if (project) fetchIssueDetail(project.id, issue.number); }}
                >
                  <span class="issues-state closed-dot"></span>
                  <span class="issues-number">#{issue.number}</span>
                  <span class="issues-title">{issue.title}</span>
                  <div class="issues-item-labels">
                    {#each issue.labels.filter(l => l.name !== "filed-by-maintainer") as label}
                      <span class="issue-label">{label.name}</span>
                    {/each}
                  </div>
                  <span class="issues-date">{formatDate(issue.createdAt)}</span>
                </div>
              {/each}
            {/if}
          </div>
        {/if}
      </section>
    {/if}

    {#if !panelFocused}
      <div class="panel-hint">
        <span class="muted">Press <kbd>l</kbd> to browse {viewMode === "runs" ? "run logs" : "issues"}</span>
      </div>
    {/if}
  {/if}
</div>

<style>
  .dashboard { width: 100%; height: 100%; overflow-y: auto; background: var(--bg-void); color: var(--text-primary); outline: 2px solid transparent; outline-offset: -2px; transition: outline-color 0.15s; }
  .dashboard.panel-focused { outline-color: rgba(255, 255, 255, 0.4); border-radius: 4px; }
  .empty-state { display: flex; flex-direction: column; align-items: center; justify-content: center; height: 100%; gap: 8px; }
  .empty-title { font-size: 16px; font-weight: 500; }
  .empty-hint { color: var(--text-secondary); font-size: 13px; }
  .empty-hint kbd, .muted kbd, .panel-hint kbd { background: var(--bg-hover); color: var(--text-emphasis); padding: 1px 6px; border-radius: 3px; font-family: var(--font-mono); font-size: 12px; }
  .dashboard-header { padding: 16px 24px; border-bottom: 1px solid var(--border-default); display: flex; align-items: baseline; }
  .dashboard-header h2 { font-size: 16px; font-weight: 600; margin: 0; }
  .header-subtitle { font-size: 12px; color: var(--text-secondary); margin-left: 8px; }
  .section { border-bottom: 1px solid var(--border-default); }
  .section-header { padding: 12px 24px; display: flex; align-items: center; gap: 8px; border-bottom: 1px solid var(--border-subtle); }
  .section-title { font-size: 13px; font-weight: 600; flex: 1; }
  .badge { font-size: 10px; padding: 1px 6px; border-radius: 3px; background: var(--bg-hover); color: var(--text-secondary); }
  .badge.enabled { background: rgba(74, 158, 110, 0.15); color: var(--status-idle); }
  .status-running { font-size: 11px; color: var(--text-emphasis); }
  .schedule-row { padding: 8px 24px; display: flex; justify-content: space-between; font-size: 11px; color: var(--text-secondary); border-bottom: 1px solid var(--border-subtle); }
  .section-body { padding: 16px 24px; }
  .muted { color: var(--text-secondary); font-size: 13px; margin: 0; }
  .worker-info { display: flex; flex-direction: column; gap: 4px; }
  .worker-label { color: var(--text-secondary); font-size: 11px; }
  .worker-issue { font-size: 13px; }
  .maintainer-status { font-size: 11px; font-weight: 500; text-transform: capitalize; }
  .maintainer-status.running { color: var(--text-emphasis); }
  .maintainer-status.error { color: var(--status-error); }
  .btn { background: var(--bg-hover); border: none; color: var(--text-primary); padding: 6px 12px; border-radius: 4px; font-size: 12px; cursor: pointer; box-shadow: none; margin-top: 8px; }
  .btn:hover { background: var(--bg-active); }
  .btn:disabled { opacity: 0.5; cursor: not-allowed; }

  /* View tabs */
  .view-tabs { display: flex; align-items: center; gap: 0; border-bottom: 1px solid var(--border-default); padding: 0 24px; }
  .view-tab { padding: 8px 16px; font-size: 12px; color: var(--text-secondary); cursor: pointer; border-bottom: 2px solid transparent; transition: color 0.15s, border-color 0.15s; }
  .view-tab:hover { color: var(--text-primary); }
  .view-tab.active { color: var(--text-emphasis); border-bottom-color: var(--text-emphasis); }
  .view-tab-hint { font-size: 10px; color: var(--text-tertiary); margin-left: auto; }

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
    border-bottom: 1px solid var(--border-subtle);
  }
  .report-item:hover { background: var(--bg-hover); }
  .report-item.selected {
    background: rgba(255, 255, 255, 0.06);
    outline: 1px solid rgba(255, 255, 255, 0.15);
    outline-offset: -1px;
  }
  .log-dot { width: 8px; height: 8px; border-radius: 50%; flex-shrink: 0; background: var(--text-emphasis); }
  .report-timestamp { color: var(--text-secondary); font-size: 11px; white-space: nowrap; flex-shrink: 0; }
  .report-summary-preview { flex: 1; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; color: var(--text-primary); }

  /* Detail view */
  .detail-view { display: flex; flex-direction: column; }
  .detail-header {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 10px 24px;
    border-bottom: 1px solid var(--border-subtle);
    font-size: 12px;
  }
  .detail-back { color: var(--text-secondary); }
  .detail-timestamp { color: var(--text-secondary); font-size: 11px; }
  .detail-summary { font-size: 11px; color: var(--text-primary); margin-left: auto; }
  .detail-blocks { padding: 12px 24px; display: flex; flex-direction: column; gap: 8px; }
  .detail-block { border-radius: 6px; transition: outline-color 0.15s; outline: 2px solid transparent; outline-offset: 2px; }
  .detail-block.block-focused { outline-color: rgba(255, 255, 255, 0.3); }

  .run-summary { padding: 12px; border-radius: 6px; background: var(--bg-hover); display: flex; gap: 16px; border-left: 3px solid var(--text-emphasis); flex-wrap: wrap; }
  .summary-stat { font-size: 13px; color: var(--text-primary); }
  .summary-stat.skipped { color: var(--text-secondary); }

  .issue-item { padding: 8px 12px; background: var(--bg-elevated); border-radius: 4px; font-size: 12px; display: flex; flex-direction: column; gap: 2px; }
  .issue-action { font-weight: 600; font-size: 11px; text-transform: uppercase; }
  .issue-number { color: var(--text-secondary); font-size: 11px; }
  .issue-title { color: var(--text-primary); }
  .issue-labels { display: flex; gap: 4px; flex-wrap: wrap; margin-top: 2px; }
  .issue-label { font-size: 10px; padding: 1px 6px; border-radius: 3px; background: var(--bg-hover); color: var(--text-secondary); }

  .policy-body { padding: 12px 24px; display: flex; flex-direction: column; gap: 8px; }
  .policy-row { display: flex; align-items: baseline; gap: 10px; }
  .policy-label { font-size: 11px; color: var(--text-secondary); width: 60px; flex-shrink: 0; }
  .policy-labels { display: flex; gap: 4px; flex-wrap: wrap; }
  .policy-tag { font-size: 10px; padding: 1px 6px; border-radius: 3px; }
  .policy-tag.required { background: rgba(74, 158, 110, 0.15); color: var(--status-idle); }
  .policy-tag.excluded { background: rgba(196, 64, 64, 0.12); color: var(--status-error); }

  .error-banner { padding: 8px 24px; background: rgba(196, 64, 64, 0.12); border-bottom: 1px solid rgba(196, 64, 64, 0.2); display: flex; align-items: baseline; gap: 8px; font-size: 12px; }
  .error-label { color: var(--status-error); font-weight: 600; font-size: 11px; text-transform: uppercase; flex-shrink: 0; }
  .error-message { color: var(--text-primary); word-break: break-word; }

  .worker-report-body { padding: 12px; font-size: 12px; color: var(--text-primary); white-space: pre-wrap; word-break: break-word; background: var(--bg-elevated); border-radius: 4px; border-left: 3px solid var(--status-idle); }

  .panel-hint { padding: 12px 24px; }

  /* Issues list */
  .issues-list { display: flex; flex-direction: column; }
  .issues-section-label { padding: 6px 24px; font-size: 11px; font-weight: 600; color: var(--text-secondary); text-transform: uppercase; background: var(--bg-elevated); border-bottom: 1px solid var(--border-subtle); }
  .issues-item {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 8px 24px;
    cursor: pointer;
    font-size: 12px;
    border-bottom: 1px solid var(--border-subtle);
  }
  .issues-item:hover { background: var(--bg-hover); }
  .issues-item.selected {
    background: rgba(255, 255, 255, 0.06);
    outline: 1px solid rgba(255, 255, 255, 0.15);
    outline-offset: -1px;
  }
  .issues-item.closed { opacity: 0.6; }
  .issues-state { width: 8px; height: 8px; border-radius: 50%; flex-shrink: 0; }
  .open-dot { background: var(--status-idle); }
  .working-dot { background: var(--status-working); }
  .closed-dot { background: var(--text-secondary); }
  .issues-number { color: var(--text-secondary); font-size: 11px; flex-shrink: 0; }
  .issues-title { flex: 1; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; color: var(--text-primary); }
  .issues-item-labels { display: flex; gap: 4px; flex-wrap: nowrap; flex-shrink: 0; }
  .issues-date { color: var(--text-tertiary); font-size: 10px; flex-shrink: 0; white-space: nowrap; }
  .queue-summary { flex: 1; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; color: var(--text-primary); }
  .queue-state { font-size: 10px; font-weight: 600; color: var(--text-secondary); text-transform: uppercase; letter-spacing: 0.04em; }
  .worker-queue-state { color: var(--text-emphasis); }

  /* Issue detail popup */
  .issue-detail-popup { padding: 16px 24px; display: flex; flex-direction: column; gap: 12px; }
  .issue-detail-header { display: flex; align-items: center; gap: 10px; font-size: 12px; }
  .issue-detail-number { color: var(--text-secondary); font-weight: 600; }
  .issue-detail-state { font-weight: 600; font-size: 11px; text-transform: uppercase; }
  .issue-detail-hint { font-size: 10px; color: var(--text-tertiary); margin-left: auto; }
  .issue-detail-title { font-size: 15px; font-weight: 600; margin: 0; color: var(--text-primary); }
  .issue-detail-meta { display: flex; gap: 16px; font-size: 11px; color: var(--text-secondary); }
  .issue-detail-labels { display: flex; gap: 4px; flex-wrap: wrap; }
  .issue-detail-body { font-size: 12px; color: var(--text-primary); white-space: pre-wrap; word-break: break-word; padding: 12px; background: var(--bg-elevated); border-radius: 6px; max-height: 400px; overflow-y: auto; line-height: 1.6; }
</style>
