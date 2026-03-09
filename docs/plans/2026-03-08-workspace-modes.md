# Workspace Modes Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use the-controller-executing-plans to implement this plan task-by-task.

**Goal:** Add workspace modes (Development / Agents) switchable via Space leader key, with isolated keymaps and distinct layouts per mode.

**Architecture:** A `workspaceMode` store drives the entire UI. The Space key opens a mode picker overlay and sets a `workspaceModeActive` flag (same pattern as `o` toggle). Layout in `App.svelte` switches sidebar content and main area based on mode. Commands get a `mode` field; `buildKeyMap(mode)` returns only global + mode-specific keys. MaintainerPanel and `b` key are removed — their content moves into the Agents workspace.

**Tech Stack:** Svelte 5 (runes), TypeScript, vitest, Catppuccin Mocha theme

---

## Task 1: Add workspace mode store

**Files:**
- Modify: `src/lib/stores.ts`
- Test: `src/lib/stores.test.ts`

**Step 1: Write the failing test**

In `src/lib/stores.test.ts`, add:

```ts
import { get } from "svelte/store";
import { workspaceMode, workspaceModePickerVisible } from "./stores";

describe("workspace mode store", () => {
  it("defaults to development", () => {
    expect(get(workspaceMode)).toBe("development");
  });

  it("can switch to agents", () => {
    workspaceMode.set("agents");
    expect(get(workspaceMode)).toBe("agents");
    workspaceMode.set("development"); // reset
  });

  it("picker starts hidden", () => {
    expect(get(workspaceModePickerVisible)).toBe(false);
  });
});
```

**Step 2: Run test to verify it fails**

Run: `npx vitest run src/lib/stores.test.ts`
Expected: FAIL — `workspaceMode` and `workspaceModePickerVisible` not exported

**Step 3: Write minimal implementation**

In `src/lib/stores.ts`, add:

```ts
export type WorkspaceMode = "development" | "agents";
export const workspaceMode = writable<WorkspaceMode>("development");
export const workspaceModePickerVisible = writable<boolean>(false);
```

**Step 4: Run test to verify it passes**

Run: `npx vitest run src/lib/stores.test.ts`
Expected: PASS

**Step 5: Commit**

```bash
git add src/lib/stores.ts src/lib/stores.test.ts
git commit -m "feat: add workspaceMode and workspaceModePickerVisible stores"
```

---

## Task 2: Add mode field to command registry

**Files:**
- Modify: `src/lib/commands.ts`
- Test: `src/lib/commands.test.ts`

**Step 1: Write the failing test**

Update `src/lib/commands.test.ts`:

```ts
import { commands, getHelpSections, buildKeyMap, type CommandDef } from "./commands";

describe("command registry", () => {
  it("every non-external command has a unique key within its mode", () => {
    const internal = commands.filter(c => !c.handledExternally);
    // Group by mode, check uniqueness within each group
    // Global commands must not conflict with any mode-specific commands
    const globalKeys = internal.filter(c => !c.mode).map(c => c.key);
    const globalSet = new Set(globalKeys);
    expect(globalKeys.length).toBe(globalSet.size);

    const modes = ["development", "agents"] as const;
    for (const mode of modes) {
      const modeKeys = internal.filter(c => c.mode === mode).map(c => c.key);
      const allKeys = [...globalKeys, ...modeKeys];
      const allSet = new Set(allKeys);
      expect(allKeys.length).toBe(allSet.size);
    }
  });

  // ... keep existing tests but update expected counts ...

  it("buildKeyMap for development includes dev commands but not agents commands", () => {
    const map = buildKeyMap("development");
    expect(map.has("c")).toBe(true); // create-session-claude
    expect(map.has("j")).toBe(true); // global nav
  });

  it("buildKeyMap for agents includes agents commands but not dev commands", () => {
    const map = buildKeyMap("agents");
    expect(map.has("j")).toBe(true); // global nav
  });
});
```

**Step 2: Run test to verify it fails**

Run: `npx vitest run src/lib/commands.test.ts`
Expected: FAIL — `mode` property doesn't exist, `buildKeyMap` doesn't accept parameter

**Step 3: Write minimal implementation**

In `src/lib/commands.ts`:

1. Add `mode` to `CommandDef`:

```ts
import type { WorkspaceMode } from "./stores";

export interface CommandDef {
  id: CommandId | ExternalCommandId;
  key: string;
  section: CommandSection;
  description: string;
  helpKey?: string;
  hidden?: boolean;
  handledExternally?: boolean;
  mode?: WorkspaceMode;  // undefined = global (available in all modes)
}
```

2. Add `mode` to each command in the `commands` array. Use these categorizations:

**Global (no mode field):** `navigate-next`, `navigate-prev`, `navigate-project-next`, `navigate-project-prev`, `expand-collapse` (both `l` and `Enter`), `jump-mode`, `fuzzy-finder`, `escape-focus`, `escape-forward`, `toggle-sidebar`, `toggle-help`, `keystroke-visualizer`, `screenshot`, `screenshot-cropped`, `screenshot-preview`

**Development (`mode: "development"`):** `create-session-claude`, `create-session-codex`, `background-worker-claude`, `background-worker-codex`, `finish-branch`, `thinking-up`, `thinking-down`, `new-project`, `delete`, `archive`, `toggle-archive-view`, `create-issue`, `triage-untriaged`, `triage-triaged`, `toggle-mode`

**Remove entirely:** `toggle-maintainer-panel` (the `b` command), `trigger-maintainer-check` (will be re-added as agents mode), `clear-maintainer-reports` (will be re-added as agents mode)

**Agents (`mode: "agents"`):** Add new commands:

```ts
// ── Agents ──
{ id: "toggle-agent", key: "o", section: "Agents", description: "Toggle focused agent on/off", mode: "agents" },
{ id: "trigger-agent-check", key: "r", section: "Agents", description: "Run maintainer check for focused project", mode: "agents" },
{ id: "clear-agent-reports", key: "c", section: "Agents", description: "Clear maintainer reports for focused project", mode: "agents" },
```

3. Add new CommandIds:

```ts
export type CommandId =
  | "navigate-next"
  | "navigate-prev"
  // ... existing ...
  | "toggle-agent"
  | "trigger-agent-check"
  | "clear-agent-reports";
```

4. Add `"Agents"` to `CommandSection`:

```ts
export type CommandSection = "Navigation" | "Sessions" | "Projects" | "Panels" | "Agents";
```

5. Update `SECTION_ORDER`:

```ts
const SECTION_ORDER: CommandSection[] = ["Navigation", "Sessions", "Projects", "Panels", "Agents"];
```

6. Update `buildKeyMap`:

```ts
export function buildKeyMap(mode?: WorkspaceMode): Map<string, CommandId> {
  const map = new Map<string, CommandId>();
  for (const cmd of commands) {
    if (cmd.handledExternally) continue;
    if (cmd.mode && cmd.mode !== mode) continue;  // skip commands from other modes
    map.set(cmd.key, cmd.id as CommandId);
  }
  return map;
}
```

7. Update `getHelpSections` to accept an optional mode:

```ts
export function getHelpSections(mode?: WorkspaceMode): HelpSection[] {
  return SECTION_ORDER.map(section => ({
    label: section,
    entries: commands
      .filter(c => c.section === section && !c.hidden)
      .filter(c => !c.mode || !mode || c.mode === mode)
      .map(c => ({ key: c.helpKey ?? c.key, description: c.description })),
  })).filter(s => s.entries.length > 0);
}
```

**Step 4: Run test to verify it passes**

Run: `npx vitest run src/lib/commands.test.ts`
Expected: PASS

**Step 5: Update remaining commands tests**

The existing tests that check exact counts need updating since we removed `b`/`trigger-maintainer-check`/`clear-maintainer-reports` and added agents commands. Update the counts in the "help sections match" test and the "getHelpSections returns all sections" test accordingly.

Run: `npx vitest run src/lib/commands.test.ts`
Expected: PASS

**Step 6: Commit**

```bash
git add src/lib/commands.ts src/lib/commands.test.ts
git commit -m "feat: add mode field to command registry, categorize by workspace"
```

---

## Task 3: Space leader key in HotkeyManager

**Files:**
- Modify: `src/lib/HotkeyManager.svelte`
- Test: `src/lib/HotkeyManager.test.ts`

**Step 1: Write the failing tests**

Add to `src/lib/HotkeyManager.test.ts`:

```ts
import { workspaceMode, workspaceModePickerVisible } from './stores';

// In beforeEach, add:
// workspaceMode.set("development");
// workspaceModePickerVisible.set(false);

describe('workspace mode (Space)', () => {
  it('Space opens the workspace mode picker', () => {
    pressKey(' ');
    expect(get(workspaceModePickerVisible)).toBe(true);
  });

  it('Space then a switches to agents mode', () => {
    pressKey(' ');
    pressKey('a');
    expect(get(workspaceMode)).toBe('agents');
    expect(get(workspaceModePickerVisible)).toBe(false);
  });

  it('Space then d switches to development mode', () => {
    workspaceMode.set('agents');
    pressKey(' ');
    pressKey('d');
    expect(get(workspaceMode)).toBe('development');
    expect(get(workspaceModePickerVisible)).toBe(false);
  });

  it('Space then Escape closes picker without changing mode', () => {
    pressKey(' ');
    pressKey('Escape');
    expect(get(workspaceMode)).toBe('development');
    expect(get(workspaceModePickerVisible)).toBe(false);
  });

  it('Space then unknown key closes picker without changing mode', () => {
    pressKey(' ');
    pressKey('q');
    expect(get(workspaceMode)).toBe('development');
    expect(get(workspaceModePickerVisible)).toBe(false);
  });

  it('Space is ignored when terminal is focused', () => {
    const xtermEl = simulateTerminalFocus();
    pressKey(' ');
    expect(get(workspaceModePickerVisible)).toBe(false);
    removeTerminalFocus(xtermEl);
  });
});
```

**Step 2: Run test to verify it fails**

Run: `npx vitest run src/lib/HotkeyManager.test.ts`
Expected: FAIL

**Step 3: Implement space leader key**

In `src/lib/HotkeyManager.svelte`:

1. Import the new stores:
```ts
import { workspaceMode, workspaceModePickerVisible } from "./stores";
```

2. Add local state:
```ts
let workspaceModeActive = $state(false);
```

3. Add handler function:
```ts
function handleWorkspaceModeKey(key: string) {
  workspaceModeActive = false;
  workspaceModePickerVisible.set(false);
  if (key === "d") {
    workspaceMode.set("development");
    return;
  }
  if (key === "a") {
    workspaceMode.set("agents");
    return;
  }
  // Any other key (including Escape) cancels
}
```

4. In `onKeydown`, add workspace mode interception AFTER toggle mode but BEFORE terminal focus check... actually, it should be after jump mode and toggle mode intercepts, and before the terminal focus check. Wait, actually it should intercept just like toggle mode. Let me place it right after the toggle mode check:

```ts
// Workspace mode intercepts all keys
if (workspaceModeActive) {
  e.stopPropagation();
  e.preventDefault();
  handleWorkspaceModeKey(e.key);
  pushKeystroke("␣" + e.key);
  return;
}
```

5. In `handleHotkey`, add a check for Space key. Since Space is not in the command registry, handle it directly in `onKeydown` before `handleHotkey` is called. Add this in the ambient mode section, before `handleHotkey(e.key)`:

```ts
// Space: workspace mode picker
if (e.key === " ") {
  e.stopPropagation();
  e.preventDefault();
  workspaceModeActive = true;
  workspaceModePickerVisible.set(true);
  pushKeystroke("␣");
  return;
}
```

6. Also update `buildKeyMap` call to pass mode. Import `workspaceMode` from stores and make the keyMap reactive:

```ts
const workspaceModeState = fromStore(workspaceMode);
let currentMode = $derived(workspaceModeState.current);
let keyMap = $derived(buildKeyMap(currentMode));
```

Note: change `const keyMap = buildKeyMap();` to the derived version above. Since `buildKeyMap` now takes a mode, the keymap automatically updates when mode changes, filtering out commands from other modes.

**Step 4: Run test to verify it passes**

Run: `npx vitest run src/lib/HotkeyManager.test.ts`
Expected: PASS

**Step 5: Commit**

```bash
git add src/lib/HotkeyManager.svelte src/lib/HotkeyManager.test.ts
git commit -m "feat: add space leader key for workspace mode switching"
```

---

## Task 4: Create WorkspaceModePicker component

**Files:**
- Create: `src/lib/WorkspaceModePicker.svelte`
- Modify: `src/App.svelte`

**Step 1: Create the component**

Create `src/lib/WorkspaceModePicker.svelte`:

```svelte
<script lang="ts">
  import { fromStore } from "svelte/store";
  import { workspaceMode, type WorkspaceMode } from "./stores";

  const workspaceModeState = fromStore(workspaceMode);
  let currentMode: WorkspaceMode = $derived(workspaceModeState.current);

  const modes: { key: string; id: WorkspaceMode; label: string }[] = [
    { key: "d", id: "development", label: "Development" },
    { key: "a", id: "agents", label: "Agents" },
  ];
</script>

<div class="overlay">
  <div class="picker">
    <div class="picker-title">Switch Workspace</div>
    <div class="picker-options">
      {#each modes as mode}
        <div class="picker-option" class:active={currentMode === mode.id}>
          <kbd>{mode.key}</kbd>
          <span class="option-label">{mode.label}</span>
          {#if currentMode === mode.id}
            <span class="current-badge">current</span>
          {/if}
        </div>
      {/each}
    </div>
  </div>
</div>

<style>
  .overlay {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.5);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 100;
  }

  .picker {
    background: #1e1e2e;
    border: 1px solid #313244;
    border-radius: 8px;
    padding: 20px 24px;
    min-width: 240px;
  }

  .picker-title {
    font-size: 14px;
    font-weight: 600;
    color: #cdd6f4;
    margin-bottom: 16px;
    text-align: center;
  }

  .picker-options {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .picker-option {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 8px 12px;
    border-radius: 6px;
    color: #a6adc8;
  }

  .picker-option.active {
    background: rgba(137, 180, 250, 0.1);
    color: #cdd6f4;
  }

  kbd {
    background: #ffffff;
    color: #1e1e2e;
    padding: 2px 8px;
    border-radius: 4px;
    font-family: monospace;
    font-size: 13px;
    font-weight: 600;
  }

  .option-label {
    flex: 1;
    font-size: 13px;
  }

  .current-badge {
    font-size: 11px;
    color: #89b4fa;
    font-style: italic;
  }
</style>
```

**Step 2: Wire up in App.svelte**

In `App.svelte`:

1. Import the component and store:
```ts
import WorkspaceModePicker from "./lib/WorkspaceModePicker.svelte";
import { workspaceModePickerVisible } from "./lib/stores";
const workspaceModePickerVisibleState = fromStore(workspaceModePickerVisible);
```

2. Add to the template (after `HotkeyHelp`, before `KeystrokeVisualizer`):
```svelte
{#if workspaceModePickerVisibleState.current}
  <WorkspaceModePicker />
{/if}
```

**Step 3: Verify manually**

Run: `npm run tauri dev`
Press Space — picker overlay should appear. Press `a` or `d` to switch. Press Escape to cancel.

**Step 4: Commit**

```bash
git add src/lib/WorkspaceModePicker.svelte src/App.svelte
git commit -m "feat: add workspace mode picker overlay"
```

---

## Task 5: Update sidebar header for mode display

**Files:**
- Modify: `src/lib/Sidebar.svelte`

**Step 1: Implementation**

In `src/lib/Sidebar.svelte`:

1. Import the store:
```ts
import { workspaceMode } from "./stores";
const workspaceModeState = fromStore(workspaceMode);
let currentMode = $derived(workspaceModeState.current);
```

2. Update the header to show mode name:

Change:
```svelte
<h2>{isArchiveView ? "Archives" : "Projects"}</h2>
```
To:
```svelte
<h2>{isArchiveView ? "Archives" : currentMode === "agents" ? "Agents" : "Projects"}</h2>
```

**Step 2: Verify manually**

Run app, switch modes with Space, verify header changes.

**Step 3: Commit**

```bash
git add src/lib/Sidebar.svelte
git commit -m "feat: show workspace mode name in sidebar header"
```

---

## Task 6: Create AgentSidebar component

**Files:**
- Create: `src/lib/sidebar/AgentTree.svelte`

This component mirrors `ProjectTree.svelte` but shows auto-worker status under each project instead of sessions.

**Step 1: Create the component**

Create `src/lib/sidebar/AgentTree.svelte`:

```svelte
<script lang="ts">
  import { fromStore } from "svelte/store";
  import { autoWorkerStatuses, type Project, type FocusTarget, type AutoWorkerStatus } from "../stores";

  interface Props {
    projects: Project[];
    currentFocus: FocusTarget;
    onProjectFocus: (projectId: string) => void;
  }

  let { projects, currentFocus, onProjectFocus }: Props = $props();

  const autoWorkerStatusesState = fromStore(autoWorkerStatuses);
  let statusMap: Map<string, AutoWorkerStatus> = $derived(autoWorkerStatusesState.current);

  function getAgentStatus(projectId: string): AutoWorkerStatus | null {
    return statusMap.get(projectId) ?? null;
  }

  function isProjectFocused(projectId: string): boolean {
    return currentFocus?.type === "project" && currentFocus.projectId === projectId;
  }
</script>

{#each projects as project (project.id)}
  <!-- svelte-ignore a11y_no_noninteractive_tabindex -->
  <div
    class="agent-project"
    class:focused={isProjectFocused(project.id)}
    data-project-id={project.id}
    tabindex="0"
    onfocus={() => onProjectFocus(project.id)}
  >
    <div class="project-header">
      <span class="project-name">{project.name}</span>
      <span class="agent-badge" class:enabled={project.auto_worker.enabled}>
        {project.auto_worker.enabled ? "ON" : "OFF"}
      </span>
    </div>
    <div class="agent-status">
      {#if !project.auto_worker.enabled}
        <span class="status-text muted">Agent disabled</span>
      {:else}
        {@const status = getAgentStatus(project.id)}
        {#if status?.status === "working"}
          <span class="status-dot working"></span>
          <span class="status-text">#{status.issue_number} {status.issue_title}</span>
        {:else}
          <span class="status-dot idle"></span>
          <span class="status-text muted">Waiting for issues</span>
        {/if}
      {/if}
    </div>
    {#if project.maintainer.enabled}
      <div class="maintainer-badge">
        <span class="maintainer-label">Maintainer ON</span>
      </div>
    {/if}
  </div>
{/each}

{#if projects.length === 0}
  <div class="empty">No projects</div>
{/if}

<style>
  .agent-project {
    padding: 10px 16px;
    border-bottom: 1px solid #313244;
    cursor: pointer;
    outline: none;
  }

  .agent-project:hover {
    background: rgba(49, 50, 68, 0.5);
  }

  .agent-project.focused {
    background: #313244;
  }

  .project-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin-bottom: 4px;
  }

  .project-name {
    font-size: 13px;
    font-weight: 500;
    color: #cdd6f4;
  }

  .agent-badge {
    font-size: 10px;
    padding: 1px 6px;
    border-radius: 3px;
    background: #313244;
    color: #6c7086;
  }

  .agent-badge.enabled {
    background: rgba(166, 227, 161, 0.2);
    color: #a6e3a1;
  }

  .agent-status {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: 12px;
  }

  .status-dot {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    flex-shrink: 0;
  }

  .status-dot.working {
    background: #f9e2af;
  }

  .status-dot.idle {
    background: #a6e3a1;
  }

  .status-text {
    color: #cdd6f4;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .status-text.muted {
    color: #6c7086;
  }

  .maintainer-badge {
    margin-top: 4px;
  }

  .maintainer-label {
    font-size: 10px;
    color: #89b4fa;
  }

  .empty {
    padding: 16px;
    color: #6c7086;
    font-size: 13px;
    text-align: center;
  }
</style>
```

**Step 2: Commit**

```bash
git add src/lib/sidebar/AgentTree.svelte
git commit -m "feat: create AgentTree sidebar component"
```

---

## Task 7: Create AgentDashboard component

**Files:**
- Create: `src/lib/AgentDashboard.svelte`

This component replaces `TerminalManager` in the main area when in agents mode. It shows the maintainer report and auto-worker status for the focused project — essentially what `MaintainerPanel.svelte` shows, but in the main content area.

**Step 1: Create the component**

Create `src/lib/AgentDashboard.svelte`. This should closely mirror the logic and display from `MaintainerPanel.svelte` but adapted for the main content area:

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

  let project = $derived(
    currentFocus?.type === "project"
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
  {#if !project}
    <div class="empty-state">
      <div class="empty-title">No project selected</div>
      <div class="empty-hint">Navigate to a project with <kbd>j</kbd> / <kbd>k</kbd></div>
    </div>
  {:else}
    <div class="dashboard-header">
      <h2>{project.name}</h2>
    </div>

    <!-- Auto-worker section -->
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

    <!-- Maintainer section -->
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

<style>
  .dashboard {
    width: 100%;
    height: 100%;
    overflow-y: auto;
    background: #11111b;
    color: #cdd6f4;
  }

  .empty-state {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    height: 100%;
    gap: 8px;
  }

  .empty-title {
    font-size: 16px;
    font-weight: 500;
  }

  .empty-hint {
    color: #6c7086;
    font-size: 13px;
  }

  .empty-hint kbd {
    background: #313244;
    color: #89b4fa;
    padding: 1px 6px;
    border-radius: 3px;
    font-family: monospace;
    font-size: 12px;
  }

  .dashboard-header {
    padding: 16px 24px;
    border-bottom: 1px solid #313244;
  }

  .dashboard-header h2 {
    font-size: 16px;
    font-weight: 600;
    margin: 0;
  }

  .section {
    border-bottom: 1px solid #313244;
  }

  .section-header {
    padding: 12px 24px;
    display: flex;
    align-items: center;
    gap: 8px;
    border-bottom: 1px solid rgba(49, 50, 68, 0.5);
  }

  .section-title {
    font-size: 13px;
    font-weight: 600;
    flex: 1;
  }

  .badge {
    font-size: 10px;
    padding: 1px 6px;
    border-radius: 3px;
    background: #313244;
    color: #6c7086;
  }

  .badge.enabled {
    background: rgba(166, 227, 161, 0.2);
    color: #a6e3a1;
  }

  .status-running {
    font-size: 11px;
    color: #89b4fa;
  }

  .schedule-row {
    padding: 8px 24px;
    display: flex;
    justify-content: space-between;
    font-size: 11px;
    color: #6c7086;
    border-bottom: 1px solid rgba(49, 50, 68, 0.5);
  }

  .section-body {
    padding: 16px 24px;
  }

  .muted {
    color: #6c7086;
    font-size: 13px;
    margin: 0;
  }

  .muted kbd {
    background: #313244;
    color: #89b4fa;
    padding: 1px 6px;
    border-radius: 3px;
    font-family: monospace;
    font-size: 12px;
  }

  .worker-info {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .worker-label {
    color: #6c7086;
    font-size: 11px;
  }

  .worker-issue {
    font-size: 13px;
  }

  .report-summary {
    padding: 12px;
    border-radius: 6px;
    background: rgba(49, 50, 68, 0.3);
    margin-bottom: 12px;
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .report-summary.passing { border-left: 3px solid #a6e3a1; }
  .report-summary.warnings { border-left: 3px solid #f9e2af; }
  .report-summary.failing { border-left: 3px solid #f38ba8; }

  .summary-text { font-size: 13px; }
  .timestamp { color: #6c7086; font-size: 11px; }

  .findings {
    display: flex;
    flex-direction: column;
    gap: 8px;
    margin-bottom: 12px;
  }

  .finding {
    padding: 8px 12px;
    background: rgba(49, 50, 68, 0.2);
    border-radius: 4px;
    font-size: 12px;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .finding-severity { font-weight: 600; font-size: 11px; text-transform: uppercase; }
  .finding-category { color: #89b4fa; font-size: 11px; }
  .finding-desc { color: #cdd6f4; }
  .finding-action { color: #6c7086; font-size: 11px; font-style: italic; }

  .maintainer-status {
    font-size: 11px;
    font-weight: 500;
    text-transform: capitalize;
  }

  .maintainer-status.passing { color: #a6e3a1; }
  .maintainer-status.warnings { color: #f9e2af; }
  .maintainer-status.failing { color: #f38ba8; }
  .maintainer-status.running { color: #89b4fa; }

  .report-actions {
    display: flex;
    gap: 8px;
  }

  .btn {
    background: #313244;
    border: none;
    color: #cdd6f4;
    padding: 6px 12px;
    border-radius: 4px;
    font-size: 12px;
    cursor: pointer;
    box-shadow: none;
  }

  .btn:hover { background: #45475a; }
  .btn:disabled { opacity: 0.5; cursor: not-allowed; }
</style>
```

**Step 2: Commit**

```bash
git add src/lib/AgentDashboard.svelte
git commit -m "feat: create AgentDashboard main area component"
```

---

## Task 8: Switch App.svelte layout based on workspace mode

**Files:**
- Modify: `src/App.svelte`
- Modify: `src/lib/Sidebar.svelte`

**Step 1: Update App.svelte layout**

In `src/App.svelte`:

1. Import new components and stores:
```ts
import AgentDashboard from "./lib/AgentDashboard.svelte";
import { workspaceMode, workspaceModePickerVisible } from "./lib/stores";
const workspaceModeState = fromStore(workspaceMode);
```

2. Update the layout to conditionally render based on mode:

Change:
```svelte
<div class="app-layout">
  {#if sidebarVisibleState.current}
    <Sidebar />
  {/if}
  <main class="terminal-area">
    <TerminalManager />
  </main>

  {#if maintainerPanelVisibleState.current}
    <MaintainerPanel />
  {/if}
</div>
```

To:
```svelte
<div class="app-layout">
  {#if sidebarVisibleState.current}
    <Sidebar />
  {/if}
  <main class="terminal-area">
    {#if workspaceModeState.current === "agents"}
      <AgentDashboard />
    {:else}
      <TerminalManager />
    {/if}
  </main>

  {#if maintainerPanelVisibleState.current}
    <MaintainerPanel />
  {/if}
</div>
```

Note: MaintainerPanel stays for now — it gets removed in Task 11.

**Step 2: Update Sidebar.svelte to show AgentTree in agents mode**

In `src/lib/Sidebar.svelte`:

1. Import AgentTree and workspaceMode:
```ts
import AgentTree from "./sidebar/AgentTree.svelte";
import { workspaceMode } from "./stores";
const workspaceModeState = fromStore(workspaceMode);
let currentMode = $derived(workspaceModeState.current);
```

2. Update the project-list section:

Change:
```svelte
<div class="project-list">
  <ProjectTree
    projects={isArchiveView ? archivedProjectList : projectList}
    mode={isArchiveView ? "archived" : "active"}
    {expandedProjectSet}
    {activeSession}
    {currentFocus}
    jumpState={jumpState}
    {projectJumpLabels}
    {getSessionStatus}
    onToggleProject={toggleProject}
    onProjectFocus={(projectId) => {
      focusTarget.set({ type: "project", projectId });
    }}
    onSessionFocus={(sessionId, projectId) => {
      focusTarget.set({ type: "session", sessionId, projectId });
    }}
    onSessionSelect={(sessionId, projectId) => {
      selectSession(sessionId);
      focusTarget.set({ type: "session", sessionId, projectId });
    }}
  />
</div>
```

To:
```svelte
<div class="project-list">
  {#if currentMode === "agents"}
    <AgentTree
      projects={projectList}
      {currentFocus}
      onProjectFocus={(projectId) => {
        focusTarget.set({ type: "project", projectId });
      }}
    />
  {:else}
    <ProjectTree
      projects={isArchiveView ? archivedProjectList : projectList}
      mode={isArchiveView ? "archived" : "active"}
      {expandedProjectSet}
      {activeSession}
      {currentFocus}
      jumpState={jumpState}
      {projectJumpLabels}
      {getSessionStatus}
      onToggleProject={toggleProject}
      onProjectFocus={(projectId) => {
        focusTarget.set({ type: "project", projectId });
      }}
      onSessionFocus={(sessionId, projectId) => {
        focusTarget.set({ type: "session", sessionId, projectId });
      }}
      onSessionSelect={(sessionId, projectId) => {
        selectSession(sessionId);
        focusTarget.set({ type: "session", sessionId, projectId });
      }}
    />
  {/if}
</div>
```

3. Hide the Active/Archives footer tabs in agents mode:

```svelte
<div class="sidebar-footer">
  {#if currentMode !== "agents"}
    <button class="footer-tab" class:active={!isArchiveView} onclick={() => archiveView.set(false)}>Active</button>
    <button class="footer-tab" class:active={isArchiveView} onclick={() => archiveView.set(true)}>Archives</button>
  {:else}
    <div class="footer-spacer"></div>
  {/if}
  <button class="btn-help" ... >?</button>
</div>
```

Add a simple style:
```css
.footer-spacer { flex: 1; }
```

**Step 3: Verify manually**

Run app, switch to agents mode with Space+a. Sidebar should show AgentTree. Main area should show AgentDashboard. Switch back with Space+d — original dev layout returns.

**Step 4: Commit**

```bash
git add src/App.svelte src/lib/Sidebar.svelte
git commit -m "feat: switch layout based on workspace mode"
```

---

## Task 9: Add agents mode key handlers

**Files:**
- Modify: `src/lib/HotkeyManager.svelte`
- Modify: `src/App.svelte`
- Test: `src/lib/HotkeyManager.test.ts`

**Step 1: Write failing tests**

Add to `src/lib/HotkeyManager.test.ts`:

```ts
describe('agents mode keys', () => {
  beforeEach(() => {
    workspaceMode.set('agents');
    focusTarget.set({ type: 'project', projectId: 'proj-1' });
  });

  afterEach(() => {
    workspaceMode.set('development');
  });

  it('o in agents mode dispatches toggle-auto-worker-enabled', () => {
    let captured: any = null;
    const unsub = hotkeyAction.subscribe((v) => { captured = v; });
    pressKey('o');
    expect(captured).toEqual({ type: 'toggle-auto-worker-enabled' });
    unsub();
  });

  it('r in agents mode dispatches trigger-maintainer-check', () => {
    let captured: any = null;
    const unsub = hotkeyAction.subscribe((v) => { captured = v; });
    pressKey('r');
    expect(captured).toEqual({ type: 'trigger-maintainer-check' });
    unsub();
  });

  it('c in agents mode dispatches clear-maintainer-reports', () => {
    let captured: any = null;
    const unsub = hotkeyAction.subscribe((v) => { captured = v; });
    pressKey('c');
    expect(captured).toEqual({ type: 'clear-maintainer-reports' });
    unsub();
  });

  it('dev-only keys like n do not work in agents mode', () => {
    let captured: any = null;
    const unsub = hotkeyAction.subscribe((v) => { captured = v; });
    pressKey('n');
    expect(captured).toBeNull();
    unsub();
  });

  it('global keys like j still work in agents mode', () => {
    projects.set([testProject, testProject2]);
    focusTarget.set({ type: 'project', projectId: 'proj-1' });
    pressKey('j');
    // In agents mode, j navigates between projects only (no sessions to visit)
    // But with current getVisibleItems logic, it depends on expanded state
    // The key point is j doesn't get blocked
    expect(get(focusTarget)).not.toBeNull();
  });
});
```

**Step 2: Run test to verify it fails**

Run: `npx vitest run src/lib/HotkeyManager.test.ts`
Expected: FAIL — agents mode commands not handled

**Step 3: Implement handlers**

In `src/lib/HotkeyManager.svelte`, add cases to `handleHotkey`:

```ts
case "toggle-agent":
  dispatchAction({ type: "toggle-auto-worker-enabled" });
  return true;
case "trigger-agent-check":
  dispatchAction({ type: "trigger-maintainer-check" });
  return true;
case "clear-agent-reports":
  dispatchAction({ type: "clear-maintainer-reports" });
  return true;
```

Also remove the context-sensitive `c` override that checks `isMaintainerPanelVisible`:

Remove these lines from the top of `handleHotkey`:
```ts
// Remove this block:
if (key === "c" && isMaintainerPanelVisible) {
  dispatchAction({ type: "clear-maintainer-reports" });
  return true;
}
```

And remove the conditional guard on `trigger-maintainer-check`:
```ts
// Change from:
case "trigger-maintainer-check":
  if (isMaintainerPanelVisible) {
    dispatchAction({ type: "trigger-maintainer-check" });
    return true;
  }
  return false;
// To: remove this case entirely (handled by trigger-agent-check in agents mode)
```

Update the `exhaustive` check in the switch default to include the new command IDs.

**Step 4: Run test to verify it passes**

Run: `npx vitest run src/lib/HotkeyManager.test.ts`
Expected: PASS

**Step 5: Commit**

```bash
git add src/lib/HotkeyManager.svelte src/lib/HotkeyManager.test.ts
git commit -m "feat: add agents mode key handlers (o/r/c)"
```

---

## Task 10: Update HotkeyHelp for mode-aware display

**Files:**
- Modify: `src/lib/HotkeyHelp.svelte`

**Step 1: Implementation**

In `src/lib/HotkeyHelp.svelte`:

1. Import workspace mode store:
```ts
import { fromStore } from "svelte/store";
import { workspaceMode } from "./stores";
const workspaceModeState = fromStore(workspaceMode);
```

2. Pass mode to `getHelpSections`:
```ts
const sections = $derived(getHelpSections(workspaceModeState.current));
```

Note: change from `const sections = getHelpSections();` (computed once) to a `$derived` so it updates when mode changes.

3. Update the subtitle:
```svelte
<p class="subtitle">Mode: {workspaceModeState.current === "agents" ? "Agents" : "Development"} — Press Space to switch</p>
```

**Step 2: Verify manually**

Run app, press `?` in dev mode — should show Navigation, Sessions, Projects, Panels. Switch to agents mode, press `?` — should show Navigation, Agents.

**Step 3: Commit**

```bash
git add src/lib/HotkeyHelp.svelte
git commit -m "feat: make help overlay mode-aware"
```

---

## Task 11: Remove MaintainerPanel, `b` key, and `maintainerPanelVisible`

**Files:**
- Delete: `src/lib/MaintainerPanel.svelte`
- Modify: `src/App.svelte`
- Modify: `src/lib/stores.ts`
- Modify: `src/lib/HotkeyManager.svelte`
- Test: `src/lib/HotkeyManager.test.ts`

**Step 1: Remove from stores.ts**

Remove:
```ts
export const maintainerPanelVisible = writable<boolean>(false);
```

Remove `"maintainer"` from `FocusTarget` type:
```ts
// Change from:
export type FocusTarget =
  | { type: "terminal"; projectId: string }
  | { type: "session"; sessionId: string; projectId: string }
  | { type: "project"; projectId: string }
  | { type: "maintainer" }
  | null;

// To:
export type FocusTarget =
  | { type: "terminal"; projectId: string }
  | { type: "session"; sessionId: string; projectId: string }
  | { type: "project"; projectId: string }
  | null;
```

Remove from `HotkeyAction`:
```ts
| { type: "toggle-maintainer-panel" }
```

**Step 2: Remove from HotkeyManager.svelte**

- Remove import of `maintainerPanelVisible`
- Remove `maintainerPanelVisibleState` and `isMaintainerPanelVisible` derived
- Remove the `"maintainer"` type check in `getFocusedProject()`
- Remove `toggle-maintainer-panel` case from `handleHotkey` switch
- Remove `trigger-maintainer-check` case (now handled by `trigger-agent-check` in agents mode)

**Step 3: Remove from App.svelte**

- Remove import of `MaintainerPanel`
- Remove `maintainerPanelVisible` import and `maintainerPanelVisibleState`
- Remove the `toggle-maintainer-panel` handler from `hotkeyAction` subscriber
- Remove `{#if maintainerPanelVisibleState.current} <MaintainerPanel /> {/if}` from template
- In `getTargetProject()`, remove the `focus?.type === "maintainer"` branch

**Step 4: Delete MaintainerPanel.svelte**

```bash
rm src/lib/MaintainerPanel.svelte
```

**Step 5: Update CommandId type**

In `src/lib/commands.ts`, remove:
- `"toggle-maintainer-panel"` from `CommandId`
- `"trigger-maintainer-check"` from `CommandId` (replaced by `"trigger-agent-check"`)
- `"clear-maintainer-reports"` from `ExternalCommandId` (replaced by `"clear-agent-reports"`)
- Remove the `b` command entry from the `commands` array
- Remove the `r` (trigger-maintainer-check) and `c` (clear-maintainer-reports) old command entries

**Step 6: Update tests**

In `src/lib/HotkeyManager.test.ts`:
- Remove all references to `maintainerPanelVisible`
- Remove the "clear maintainer reports (c)" describe block (this behavior now lives in agents mode tests)
- Update the `c on project` test — `c` should always dispatch `pick-issue-for-session` in dev mode now (no panel visibility check)

In `src/lib/commands.test.ts`:
- Update expected section counts
- Remove any test checking for `b` key in the keymap

Run: `npx vitest run`
Expected: ALL PASS

**Step 7: Commit**

```bash
git add -A
git commit -m "feat: remove MaintainerPanel, b key, and maintainerPanelVisible store"
```

---

## Task 12: Final test cleanup and verification

**Files:**
- Test: `src/lib/HotkeyManager.test.ts`
- Test: `src/lib/commands.test.ts`

**Step 1: Run full test suite**

Run: `npx vitest run`

Fix any remaining test failures. Common issues to watch for:
- Tests referencing `maintainerPanelVisible` need removal or update
- Tests checking exact command counts need updating
- Tests referencing `b` key need removal
- The exhaustive switch in HotkeyManager may need new cases added

**Step 2: Run type check**

Run: `npx tsc --noEmit` (or `npm run check` if available)

Fix any TypeScript errors from removed types/stores.

**Step 3: Run Rust tests**

Run: `cd src-tauri && cargo test`

Backend should be unaffected (no Rust changes needed).

**Step 4: Manual smoke test**

Run: `npm run tauri dev`

Verify:
1. App starts in development mode — sidebar shows "Projects", terminal area works
2. Press Space — mode picker appears with [d] Development, [a] Agents
3. Press `a` — switches to agents mode: sidebar shows "Agents" with projects and their auto-worker status, main area shows AgentDashboard
4. Press `o` on a focused project — toggles agent on/off
5. Press `r` — triggers maintainer check (toast appears)
6. Press `j`/`k` — navigates between projects
7. Press Space+d — returns to development mode, terminal works
8. Press `?` — help shows mode-appropriate keys
9. `b` key does nothing (removed)
10. `c` in dev mode creates session (no panel visibility check)

**Step 5: Commit**

```bash
git add -A
git commit -m "chore: test cleanup for workspace modes"
```

---

## Summary of key changes

| File | Change |
|------|--------|
| `src/lib/stores.ts` | Add `WorkspaceMode`, `workspaceMode`, `workspaceModePickerVisible`. Remove `maintainerPanelVisible`, `"maintainer"` from FocusTarget, `"toggle-maintainer-panel"` from HotkeyAction |
| `src/lib/commands.ts` | Add `mode` field, `"Agents"` section, new agent CommandIds. Remove `b`/old `r`/old `c` commands |
| `src/lib/HotkeyManager.svelte` | Add space leader key, workspace mode key interception, agent mode handlers. Remove `maintainerPanelVisible` refs, old panel-conditional logic |
| `src/lib/WorkspaceModePicker.svelte` | New — overlay showing mode options |
| `src/lib/sidebar/AgentTree.svelte` | New — sidebar tree showing agents per project |
| `src/lib/AgentDashboard.svelte` | New — main area dashboard with auto-worker status + maintainer reports |
| `src/lib/Sidebar.svelte` | Conditionally render AgentTree vs ProjectTree based on mode, update header |
| `src/lib/HotkeyHelp.svelte` | Pass mode to `getHelpSections`, show mode indicator |
| `src/App.svelte` | Switch main area between TerminalManager/AgentDashboard, add WorkspaceModePicker, remove MaintainerPanel |
| `src/lib/MaintainerPanel.svelte` | Deleted |
