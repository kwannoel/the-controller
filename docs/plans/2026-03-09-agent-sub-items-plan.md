# Agent Sub-Items Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use the-controller-executing-plans to implement this plan task-by-task.

**Goal:** Show auto-worker and maintainer as expandable sub-items under each project in the agent workspace sidebar, with dedicated agent pages accessible via `l`/`Esc` navigation.

**Architecture:** Extend `FocusTarget` with an `"agent"` variant. Rewrite `AgentTree.svelte` from flat list to expandable tree (mirroring `ProjectTree.svelte`). Make `HotkeyManager` agent-mode-aware for navigation and focus. Scope `AgentDashboard` to the focused agent.

**Tech Stack:** Svelte 5 (runes), TypeScript, Catppuccin Mocha theme

**Design doc:** `docs/plans/2026-03-09-agent-sub-items-design.md`

---

### Task 1: Add `"agent"` variant to FocusTarget

**Files:**
- Modify: `src/lib/stores.ts:135-139`

**Step 1: Add the agent variant to FocusTarget**

In `src/lib/stores.ts`, change:

```typescript
export type FocusTarget =
  | { type: "terminal"; projectId: string }
  | { type: "session"; sessionId: string; projectId: string }
  | { type: "project"; projectId: string }
  | null;
```

to:

```typescript
export type AgentKind = "auto-worker" | "maintainer";

export type FocusTarget =
  | { type: "terminal"; projectId: string }
  | { type: "session"; sessionId: string; projectId: string }
  | { type: "project"; projectId: string }
  | { type: "agent"; agentKind: AgentKind; projectId: string }
  | null;
```

**Step 2: Verify no type errors**

Run: `npx tsc --noEmit`
Expected: No new errors (existing code handles FocusTarget with `?.type === "project"` etc., so the new variant is safely ignored by existing branches).

**Step 3: Commit**

```bash
git add src/lib/stores.ts
git commit -m "feat: add agent variant to FocusTarget type"
```

---

### Task 2: Rewrite AgentTree as expandable tree

**Files:**
- Modify: `src/lib/sidebar/AgentTree.svelte` (full rewrite)

**Step 1: Rewrite AgentTree.svelte**

Replace the entire component. The new version mirrors `ProjectTree.svelte` structure but with agent sub-items instead of sessions.

```svelte
<script lang="ts">
  import { fromStore } from "svelte/store";
  import { autoWorkerStatuses, maintainerStatuses, type AgentKind, type Project, type FocusTarget, type AutoWorkerStatus, type MaintainerStatus } from "../stores";

  interface Props {
    projects: Project[];
    expandedProjectSet: Set<string>;
    currentFocus: FocusTarget;
    onToggleProject: (projectId: string) => void;
    onProjectFocus: (projectId: string) => void;
    onAgentFocus: (agentKind: AgentKind, projectId: string) => void;
  }

  let { projects, expandedProjectSet, currentFocus, onToggleProject, onProjectFocus, onAgentFocus }: Props = $props();

  const autoWorkerStatusesState = fromStore(autoWorkerStatuses);
  let awStatusMap: Map<string, AutoWorkerStatus> = $derived(autoWorkerStatusesState.current);

  const maintainerStatusesState = fromStore(maintainerStatuses);
  let mStatusMap: Map<string, MaintainerStatus> = $derived(maintainerStatusesState.current);

  function isProjectFocused(projectId: string): boolean {
    return currentFocus?.type === "project" && currentFocus.projectId === projectId;
  }

  function isAgentFocused(projectId: string, kind: AgentKind): boolean {
    return currentFocus?.type === "agent" && currentFocus.projectId === projectId && currentFocus.agentKind === kind;
  }

  function awStatusText(projectId: string, enabled: boolean): string {
    if (!enabled) return "Disabled";
    const s = awStatusMap.get(projectId);
    if (s?.status === "working") return `#${s.issue_number} ${s.issue_title}`;
    return "Waiting for issues";
  }

  function awIsWorking(projectId: string): boolean {
    return awStatusMap.get(projectId)?.status === "working";
  }

  function mStatusValue(projectId: string): MaintainerStatus | null {
    return mStatusMap.get(projectId) ?? null;
  }
</script>

{#each projects as project (project.id)}
  <div class="project-item">
    <!-- svelte-ignore a11y_no_noninteractive_tabindex -->
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <div
      class="project-header"
      class:focus-target={isProjectFocused(project.id)}
      tabindex="0"
      data-project-id={project.id}
      onfocusin={(e: FocusEvent) => {
        if (e.target === e.currentTarget) onProjectFocus(project.id);
      }}
    >
      <button class="btn-expand" onclick={() => onToggleProject(project.id)}>
        {expandedProjectSet.has(project.id) ? "\u25BC" : "\u25B6"}
      </button>
      <span class="project-name">{project.name}</span>
      <span class="agent-count">2</span>
    </div>

    {#if expandedProjectSet.has(project.id)}
      <div class="agent-list">
        <!-- Auto-worker -->
        <!-- svelte-ignore a11y_no_noninteractive_tabindex -->
        <div
          class="agent-item"
          class:focus-target={isAgentFocused(project.id, "auto-worker")}
          data-agent-id="{project.id}:auto-worker"
          tabindex="0"
          onfocusin={() => onAgentFocus("auto-worker", project.id)}
        >
          <span class="status-dot" class:working={awIsWorking(project.id)} class:idle={project.auto_worker.enabled && !awIsWorking(project.id)} class:disabled={!project.auto_worker.enabled}></span>
          <span class="agent-label">Auto-worker</span>
          <span class="agent-badge" class:enabled={project.auto_worker.enabled}>
            {project.auto_worker.enabled ? "ON" : "OFF"}
          </span>
        </div>

        <!-- Maintainer -->
        <!-- svelte-ignore a11y_no_noninteractive_tabindex -->
        <div
          class="agent-item"
          class:focus-target={isAgentFocused(project.id, "maintainer")}
          data-agent-id="{project.id}:maintainer"
          tabindex="0"
          onfocusin={() => onAgentFocus("maintainer", project.id)}
        >
          {@const mStatus = mStatusValue(project.id)}
          <span class="status-dot" class:working={mStatus === "running"} class:idle={project.maintainer.enabled && mStatus !== "running"} class:disabled={!project.maintainer.enabled}></span>
          <span class="agent-label">Maintainer</span>
          <span class="agent-badge" class:enabled={project.maintainer.enabled}>
            {project.maintainer.enabled ? "ON" : "OFF"}
          </span>
        </div>
      </div>
    {/if}
  </div>
{/each}

{#if projects.length === 0}
  <div class="empty">No projects</div>
{/if}

<style>
  .project-item {
    border-bottom: 1px solid #313244;
  }

  .project-header {
    display: flex;
    align-items: center;
    padding: 8px 16px;
    gap: 8px;
  }

  .project-header:hover {
    background: #313244;
  }

  .project-header.focus-target {
    outline: 2px solid #89b4fa;
    outline-offset: -2px;
    border-radius: 4px;
  }

  .btn-expand {
    background: none;
    border: none;
    color: #6c7086;
    cursor: pointer;
    padding: 0;
    font-size: 10px;
    width: 16px;
    text-align: center;
    box-shadow: none;
  }

  .project-name {
    flex: 1;
    font-size: 13px;
    font-weight: 500;
    word-break: break-word;
  }

  .agent-count {
    font-size: 11px;
    color: #6c7086;
    background: #313244;
    padding: 1px 6px;
    border-radius: 8px;
  }

  .agent-list {
    padding: 0;
  }

  .agent-item {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 6px 16px 6px 40px;
    cursor: pointer;
    font-size: 12px;
    outline: none;
  }

  .agent-item:hover {
    background: #313244;
  }

  .agent-item.focus-target {
    outline: 2px solid #89b4fa;
    outline-offset: -2px;
    border-radius: 4px;
  }

  .status-dot {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    flex-shrink: 0;
    background: #6c7086;
  }

  .status-dot.working { background: #f9e2af; }
  .status-dot.idle { background: #a6e3a1; }
  .status-dot.disabled { background: #6c7086; }

  .agent-label {
    flex: 1;
    color: #cdd6f4;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .agent-badge {
    font-size: 10px;
    padding: 1px 6px;
    border-radius: 3px;
    background: #313244;
    color: #6c7086;
    flex-shrink: 0;
  }

  .agent-badge.enabled { background: rgba(166, 227, 161, 0.2); color: #a6e3a1; }

  .empty { padding: 16px; color: #6c7086; font-size: 13px; text-align: center; }
</style>
```

**Step 2: Verify no type errors**

Run: `npx tsc --noEmit`
Expected: Will fail because `Sidebar.svelte` doesn't pass the new props yet. That's expected — fixed in Task 3.

**Step 3: Commit**

```bash
git add src/lib/sidebar/AgentTree.svelte
git commit -m "feat: rewrite AgentTree as expandable tree with agent sub-items"
```

---

### Task 3: Update Sidebar to pass new props to AgentTree

**Files:**
- Modify: `src/lib/Sidebar.svelte:476-483`

**Step 1: Update the AgentTree usage in Sidebar.svelte**

Find the AgentTree usage (around line 476-483):

```svelte
<AgentTree
  projects={projectList}
  {currentFocus}
  onProjectFocus={(projectId) => {
    focusTarget.set({ type: "project", projectId });
  }}
/>
```

Replace with:

```svelte
<AgentTree
  projects={projectList}
  {expandedProjectSet}
  {currentFocus}
  onToggleProject={toggleProject}
  onProjectFocus={(projectId) => {
    focusTarget.set({ type: "project", projectId });
  }}
  onAgentFocus={(agentKind, projectId) => {
    focusTarget.set({ type: "agent", agentKind, projectId });
  }}
/>
```

**Step 2: Add import for AgentKind type**

In `Sidebar.svelte`, update the import from `./stores` to include `AgentKind`:

The stores import at line 5 already imports what we need. The `AgentKind` type flows through `FocusTarget`, so no explicit import is needed in Sidebar — only AgentTree uses it directly.

**Step 3: Add agent focus DOM handling to the focusTarget effect**

In `Sidebar.svelte`, find the `$effect` that handles focus (lines 57-79). Add an `else if` clause for agent focus after the session and project cases:

```typescript
} else if (currentFocus?.type === "agent") {
  if (!expandedProjectSet.has(currentFocus.projectId)) {
    const next = new Set(expandedProjectSet);
    next.add(currentFocus.projectId);
    expandedProjects.set(next);
  }
  if (sidebarEl) {
    requestAnimationFrame(() => {
      const el = sidebarEl?.querySelector<HTMLElement>(`[data-agent-id="${currentFocus.agentKind === "auto-worker" ? currentFocus.projectId + ":auto-worker" : currentFocus.projectId + ":maintainer"}"]`);
      if (el) el.focus();
    });
  }
}
```

Simplified: use the same `data-agent-id` format as AgentTree:

```typescript
} else if (currentFocus?.type === "agent") {
  if (!expandedProjectSet.has(currentFocus.projectId)) {
    const next = new Set(expandedProjectSet);
    next.add(currentFocus.projectId);
    expandedProjects.set(next);
  }
  if (sidebarEl) {
    requestAnimationFrame(() => {
      const el = sidebarEl?.querySelector<HTMLElement>(`[data-agent-id="${currentFocus.projectId}:${currentFocus.agentKind}"]`);
      if (el) el.focus();
    });
  }
}
```

**Step 4: Verify no type errors**

Run: `npx tsc --noEmit`
Expected: PASS

**Step 5: Commit**

```bash
git add src/lib/Sidebar.svelte
git commit -m "feat: pass expanded/toggle/focus props from Sidebar to AgentTree"
```

---

### Task 4: Make HotkeyManager agent-mode-aware

**Files:**
- Modify: `src/lib/HotkeyManager.svelte:179-197` (SidebarItem + getVisibleItems)
- Modify: `src/lib/HotkeyManager.svelte:199-218` (navigateItem)
- Modify: `src/lib/HotkeyManager.svelte:355-369` (expand-collapse)
- Modify: `src/lib/HotkeyManager.svelte:486-491` (Escape handling)

**Step 1: Extend SidebarItem type and getVisibleItems**

At line 179, change `SidebarItem` to include agents:

```typescript
type SidebarItem =
  | { type: "project"; projectId: string }
  | { type: "session"; sessionId: string; projectId: string }
  | { type: "agent"; agentKind: "auto-worker" | "maintainer"; projectId: string };
```

Replace `getVisibleItems()` (lines 183-197) with mode-aware version:

```typescript
function getVisibleItems(): SidebarItem[] {
  if (currentMode === "agents") {
    const result: SidebarItem[] = [];
    for (const p of projectList) {
      result.push({ type: "project", projectId: p.id });
      if (!expandedSet.has(p.id)) continue;
      result.push({ type: "agent", agentKind: "auto-worker", projectId: p.id });
      result.push({ type: "agent", agentKind: "maintainer", projectId: p.id });
    }
    return result;
  }
  const list = isArchiveView ? archivedProjectList : projectList;
  const result: SidebarItem[] = [];
  for (const p of list) {
    result.push({ type: "project", projectId: p.id });
    if (!expandedSet.has(p.id)) continue;
    const sessions = isArchiveView
      ? p.sessions.filter(s => s.archived && !s.auto_worker_session)
      : p.sessions.filter(s => !s.archived && !s.auto_worker_session);
    for (const s of sessions) {
      result.push({ type: "session", sessionId: s.id, projectId: p.id });
    }
  }
  return result;
}
```

**Step 2: Update navigateItem to handle agent items**

Replace `navigateItem` (lines 199-218):

```typescript
function navigateItem(direction: 1 | -1) {
  const items = getVisibleItems();
  if (items.length === 0) return;
  let idx = -1;
  if (currentFocus?.type === "session") {
    idx = items.findIndex(it => it.type === "session" && it.sessionId === currentFocus.sessionId);
  } else if (currentFocus?.type === "agent") {
    idx = items.findIndex(it => it.type === "agent" && it.projectId === currentFocus.projectId && it.agentKind === currentFocus.agentKind);
  } else if (currentFocus?.type === "project") {
    idx = items.findIndex(it => it.type === "project" && it.projectId === currentFocus.projectId);
  }
  const len = items.length;
  const next = items[((idx + direction) % len + len) % len];
  if (next.type === "session") {
    if (!isArchiveView) {
      activeSessionId.set(next.sessionId);
    }
    focusTarget.set({ type: "session", sessionId: next.sessionId, projectId: next.projectId });
  } else if (next.type === "agent") {
    focusTarget.set({ type: "agent", agentKind: next.agentKind, projectId: next.projectId });
  } else {
    focusTarget.set({ type: "project", projectId: next.projectId });
  }
}
```

**Step 3: Update navigateProject to handle agent focus**

In `navigateProject` (line 223), update the `focusedProjectId` derivation to also handle `"agent"` focus type:

```typescript
const focusedProjectId = currentFocus?.type === "project" || currentFocus?.type === "session" || currentFocus?.type === "agent"
  ? currentFocus.projectId
  : null;
```

**Step 4: Update expand-collapse handler for agent focus**

In the `expand-collapse` case (lines 355-369), add handling for `"agent"` focus type. The agent focus type on `l` doesn't need to do anything special — pressing `l` on an agent is what _sets_ agent focus from navigation. But we need the reverse: when focus is already on an agent in the sidebar, `l` should be a no-op (agent page is already showing). Actually, looking at the pattern: `l` on a session sets active + dispatches focus-terminal. For agents, `l` on an agent sub-item in the sidebar is the equivalent — but since sidebar stays visible, it's already handled by the focus itself. No extra action needed.

Keep the existing project/session handling and don't add agent — the agent page shows reactively when `focusTarget` type is `"agent"`.

**Step 5: Update Escape handling for agent focus**

In the ambient-mode Escape handler (line 486), add agent focus walk-up:

```typescript
} else if (currentFocus?.type === "session") {
  focusTarget.set({ type: "project", projectId: currentFocus.projectId });
  e.stopPropagation();
  e.preventDefault();
  pushKeystroke("Esc");
} else if (currentFocus?.type === "agent") {
  focusTarget.set({ type: "project", projectId: currentFocus.projectId });
  e.stopPropagation();
  e.preventDefault();
  pushKeystroke("Esc");
}
```

**Step 6: Update getFocusedProject to handle agent focus**

In `getFocusedProject` (line 233-238), update to include agent type:

```typescript
function getFocusedProject(): Project | null {
  if (currentFocus?.type === "project" || currentFocus?.type === "session" || currentFocus?.type === "agent") {
    return projectList.find((p) => p.id === currentFocus.projectId) ?? null;
  }
  return null;
}
```

**Step 7: Verify no type errors**

Run: `npx tsc --noEmit`
Expected: PASS

**Step 8: Commit**

```bash
git add src/lib/HotkeyManager.svelte
git commit -m "feat: make HotkeyManager agent-mode-aware for navigation and focus"
```

---

### Task 5: Scope AgentDashboard to focused agent

**Files:**
- Modify: `src/lib/AgentDashboard.svelte` (significant rewrite)

**Step 1: Rewrite AgentDashboard to react to agent focus**

Replace the component. The key changes:
- Derive `focusedAgent` from `focusTarget` (type `"agent"` only)
- When `agentKind === "auto-worker"`: show only auto-worker section
- When `agentKind === "maintainer"`: show only maintainer section
- When no agent focused: show empty state

```svelte
<script lang="ts">
  import { fromStore } from "svelte/store";
  import { invoke } from "@tauri-apps/api/core";
  import { focusTarget, projects, maintainerStatuses, autoWorkerStatuses, type Project, type FocusTarget, type MaintainerReport, type MaintainerStatus, type AutoWorkerStatus } from "./stores";
  import { showToast } from "./toast";

  let report: MaintainerReport | null = $state(null);
  let loading = $state(false);
  let triggerLoading = $state(false);
  let currentProjectId: string | null = $state(null);

  const projectsState = fromStore(projects);
  let projectList: Project[] = $derived(projectsState.current);
  const focusTargetState = fromStore(focusTarget);
  let currentFocus: FocusTarget = $derived(focusTargetState.current);

  let focusedAgent = $derived(
    currentFocus?.type === "agent" ? currentFocus : null
  );

  let project = $derived(
    focusedAgent
      ? projectList.find((p) => p.id === focusedAgent!.projectId) ?? null
      : null
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
    } catch {
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

  let nextRunText = $state("");

  function computeNextRunText(): string {
    if (!project?.maintainer.enabled) return "Disabled";
    if (!report) return "Pending";
    const lastRun = new Date(report.timestamp).getTime();
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

      <div class="section-body">
        {#if loading}
          <p class="muted">Loading...</p>
        {:else if !report}
          <p class="muted">No reports yet</p>
          {#if project.maintainer.enabled}
            <button class="btn" onclick={triggerCheck} disabled={triggerLoading}>
              {triggerLoading ? "Running..." : "(r) Run check now"}
            </button>
          {/if}
        {:else}
          <div class="report-summary" class:passing={report.status === "passing"} class:warnings={report.status === "warnings"} class:failing={report.status === "failing"}>
            <span class="summary-text">{report.summary}</span>
            <span class="timestamp">{new Date(report.timestamp).toLocaleString()}</span>
          </div>

          {#if report.findings.length > 0}
            <div class="findings">
              {#each report.findings as finding}
                <div class="finding">
                  <span class="finding-severity" style="color: {severityColor(finding.severity)}">{finding.severity}</span>
                  <span class="finding-category">{finding.category}</span>
                  <span class="finding-desc">{finding.description}</span>
                  <span class="finding-action">{actionLabel(finding.action_taken)}</span>
                </div>
              {/each}
            </div>
          {/if}

          <div class="report-actions">
            <button class="btn" onclick={triggerCheck} disabled={triggerLoading}>
              {triggerLoading ? "Running..." : "(r) Run again"}
            </button>
          </div>
        {/if}
      </div>
    </section>
  {/if}
</div>
```

Keep all existing styles and add:

```css
.header-subtitle {
  font-size: 12px;
  color: #6c7086;
  margin-left: 8px;
}

.dashboard-header {
  padding: 16px 24px;
  border-bottom: 1px solid #313244;
  display: flex;
  align-items: baseline;
}
```

**Step 2: Verify no type errors**

Run: `npx tsc --noEmit`
Expected: PASS

**Step 3: Commit**

```bash
git add src/lib/AgentDashboard.svelte
git commit -m "feat: scope AgentDashboard to focused agent kind"
```

---

### Task 6: Manual verification

**Step 1: Run the app**

Run: `npm run tauri dev`

**Step 2: Verify sidebar**

1. Switch to Agents workspace (Space → a)
2. Projects should show with ▶/▼ expand buttons
3. Press `l` on a project — should expand showing "Auto-worker" and "Maintainer" sub-items
4. Press `Esc` on expand — should collapse

**Step 3: Verify navigation**

1. `j`/`k` navigates through projects and agent sub-items when expanded
2. `J`/`K` skips agents, navigates only between projects
3. `l` on an agent sub-item — main area shows scoped agent page
4. `Esc` from agent sub-item — focus returns to parent project

**Step 4: Verify dashboard**

1. Focus auto-worker — shows only auto-worker section with project name + "Auto-worker" subtitle
2. Focus maintainer — shows only maintainer section with project name + "Maintainer" subtitle
3. No agent focused — shows "No agent selected" empty state

**Step 5: Commit any fixes if needed**
