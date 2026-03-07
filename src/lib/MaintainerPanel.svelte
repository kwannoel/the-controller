<script lang="ts">
  import { fromStore } from "svelte/store";
  import { invoke } from "@tauri-apps/api/core";
  import { focusTarget, projects, type Project, type FocusTarget, type MaintainerReport } from "./stores";
  import { showToast } from "./toast";

  let report: MaintainerReport | null = $state(null);
  let loading = $state(false);
  let triggerLoading = $state(false);
  let currentProjectId: string | null = $state(null);

  const projectsState = fromStore(projects);
  let projectList: Project[] = $derived(projectsState.current);
  const focusTargetState = fromStore(focusTarget);
  let currentFocus: FocusTarget = $derived(focusTargetState.current);

  let project = $derived(
    currentFocus?.projectId
      ? projectList.find((p) => p.id === currentFocus!.projectId)
      : projectList[0] ?? null
  );

  $effect(() => {
    const pid = project?.id ?? null;
    if (pid && pid !== currentProjectId) {
      currentProjectId = pid;
      fetchStatus(pid);
    }
  });

  async function fetchStatus(projectId: string) {
    loading = true;
    try {
      report = await invoke<MaintainerReport | null>("get_maintainer_status", { projectId });
    } catch (e) {
      report = null;
    } finally {
      loading = false;
    }
  }

  async function triggerCheck() {
    if (!project) return;
    triggerLoading = true;
    try {
      report = await invoke<MaintainerReport>("trigger_maintainer_check", { projectId: project.id });
      showToast("Maintainer check complete", "info");
    } catch (e) {
      showToast(String(e), "error");
    } finally {
      triggerLoading = false;
    }
  }

  async function toggleMaintainer() {
    if (!project) return;
    const newEnabled = !project.maintainer.enabled;
    try {
      await invoke("configure_maintainer", {
        projectId: project.id,
        enabled: newEnabled,
        intervalMinutes: project.maintainer.interval_minutes,
      });
      const result: Project[] = await invoke("list_projects");
      projects.set(result);
    } catch (e) {
      showToast(String(e), "error");
    }
  }

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
</script>

<aside class="maintainer-panel">
  <div class="panel-header">
    <span class="panel-title">Maintainer</span>
    {#if project}
      <button class="btn-toggle" class:enabled={project.maintainer.enabled} onclick={toggleMaintainer}>
        {project.maintainer.enabled ? "ON" : "OFF"}
      </button>
    {/if}
  </div>

  {#if !project}
    <div class="status">No project selected</div>
  {:else if loading}
    <div class="status">Loading...</div>
  {:else if !report}
    <div class="status">
      <p>No reports yet</p>
      <button class="btn-run" onclick={triggerCheck} disabled={triggerLoading}>
        {triggerLoading ? "Running..." : "Run check now"}
      </button>
    </div>
  {:else}
    <div class="report-summary" class:passing={report.status === "passing"} class:warnings={report.status === "warnings"} class:failing={report.status === "failing"}>
      <span class="summary-text">{report.summary}</span>
      <span class="timestamp">{new Date(report.timestamp).toLocaleString()}</span>
    </div>

    {#if report.findings.length > 0}
      <ul class="findings-list">
        {#each report.findings as finding}
          <li class="finding-item">
            <span class="finding-severity" style="color: {severityColor(finding.severity)}">{finding.severity}</span>
            <span class="finding-category">{finding.category}</span>
            <span class="finding-desc">{finding.description}</span>
            <span class="finding-action">{actionLabel(finding.action_taken)}</span>
          </li>
        {/each}
      </ul>
    {/if}

    <div class="panel-actions">
      <button class="btn-run" onclick={triggerCheck} disabled={triggerLoading}>
        {triggerLoading ? "Running..." : "Run again"}
      </button>
    </div>
  {/if}
</aside>

<style>
  .maintainer-panel {
    width: 320px;
    min-width: 320px;
    height: 100vh;
    background: #1e1e2e;
    border-left: 1px solid #313244;
    display: flex;
    flex-direction: column;
    overflow: hidden;
  }

  .panel-header {
    padding: 12px 16px;
    font-size: 13px;
    font-weight: 600;
    color: #cdd6f4;
    border-bottom: 1px solid #313244;
    display: flex;
    align-items: center;
    justify-content: space-between;
  }

  .btn-toggle {
    background: #313244;
    border: none;
    color: #6c7086;
    padding: 2px 8px;
    border-radius: 4px;
    font-size: 11px;
    cursor: pointer;
    box-shadow: none;
  }

  .btn-toggle.enabled {
    background: rgba(166, 227, 161, 0.2);
    color: #a6e3a1;
  }

  .status {
    padding: 16px;
    color: #6c7086;
    font-size: 13px;
    display: flex;
    flex-direction: column;
    gap: 12px;
  }

  .btn-run {
    background: #313244;
    border: none;
    color: #cdd6f4;
    padding: 6px 12px;
    border-radius: 4px;
    font-size: 12px;
    cursor: pointer;
    box-shadow: none;
  }

  .btn-run:hover { background: #45475a; }
  .btn-run:disabled { opacity: 0.5; cursor: not-allowed; }

  .report-summary {
    padding: 12px 16px;
    border-bottom: 1px solid #313244;
    font-size: 13px;
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .report-summary.passing { border-left: 3px solid #a6e3a1; }
  .report-summary.warnings { border-left: 3px solid #f9e2af; }
  .report-summary.failing { border-left: 3px solid #f38ba8; }

  .summary-text { color: #cdd6f4; }
  .timestamp { color: #6c7086; font-size: 11px; }

  .findings-list {
    list-style: none;
    margin: 0;
    padding: 0;
    overflow-y: auto;
    flex: 1;
  }

  .finding-item {
    padding: 8px 16px;
    border-bottom: 1px solid rgba(49, 50, 68, 0.5);
    font-size: 12px;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .finding-severity { font-weight: 600; font-size: 11px; text-transform: uppercase; }
  .finding-category { color: #89b4fa; font-size: 11px; }
  .finding-desc { color: #cdd6f4; }
  .finding-action { color: #6c7086; font-size: 11px; font-style: italic; }

  .panel-actions {
    padding: 12px 16px;
    border-top: 1px solid #313244;
  }
</style>
