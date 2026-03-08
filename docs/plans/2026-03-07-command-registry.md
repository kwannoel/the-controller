# Command Registry Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use the-controller-executing-plans to implement this plan task-by-task.

**Goal:** Single source of truth for all keyboard commands — adding a command to the registry automatically updates help, and TypeScript enforces handler coverage.

**Architecture:** A `commands.ts` module defines every command with metadata (key, description, section). `HotkeyHelp.svelte` derives its display from the registry. `HotkeyManager.svelte` switches on `CommandId` instead of raw key strings, with a `never` default for compile-time exhaustiveness.

**Tech Stack:** TypeScript, Svelte 5, Vitest

---

### Task 1: Create the command registry

**Files:**
- Create: `src/lib/commands.ts`

**Step 1: Create `src/lib/commands.ts` with types and full command list**

```ts
export type CommandSection = "Navigation" | "Sessions" | "Projects" | "Panels";

// IDs for commands handled in handleHotkey's switch
export type CommandId =
  | "navigate-next"
  | "navigate-prev"
  | "navigate-project-next"
  | "navigate-project-prev"
  | "jump-mode"
  | "fuzzy-finder"
  | "new-project"
  | "delete"
  | "archive"
  | "toggle-archive-view"
  | "create-session-claude"
  | "create-session-codex"
  | "background-worker-claude"
  | "background-worker-codex"
  | "finish-branch"
  | "toggle-sidebar"
  | "create-issue"
  | "triage-untriaged"
  | "triage-triaged"
  | "expand-collapse"
  | "toggle-maintainer"
  | "trigger-maintainer-check"
  | "toggle-maintainer-panel"
  | "toggle-help";

// IDs for commands handled outside handleHotkey (Cmd+key, Escape)
export type ExternalCommandId =
  | "screenshot"
  | "screenshot-cropped"
  | "keystroke-visualizer"
  | "escape-focus";

export interface CommandDef {
  id: CommandId | ExternalCommandId;
  key: string;
  section: CommandSection;
  description: string;
  helpKey?: string;       // Display override for help (e.g., "j / k")
  hidden?: boolean;       // Don't show in help (paired secondary keys)
  handledExternally?: boolean;  // Handled in onKeydown, not handleHotkey
}

export const commands: CommandDef[] = [
  // ── Navigation ──
  { id: "navigate-next", key: "j", section: "Navigation", description: "Next / previous item (project or session)", helpKey: "j / k" },
  { id: "navigate-prev", key: "k", section: "Navigation", description: "Next / previous item (project or session)", hidden: true },
  { id: "navigate-project-next", key: "J", section: "Navigation", description: "Next / previous project (skip sessions)", helpKey: "J / K" },
  { id: "navigate-project-prev", key: "K", section: "Navigation", description: "Next / previous project (skip sessions)", hidden: true },
  { id: "expand-collapse", key: "l", section: "Navigation", description: "Expand/collapse project or focus terminal", helpKey: "l / Enter" },
  { id: "expand-collapse", key: "Enter", section: "Navigation", description: "Expand/collapse project or focus terminal", hidden: true },
  { id: "jump-mode", key: "g", section: "Navigation", description: "Go to project / session (jump mode)" },
  { id: "fuzzy-finder", key: "f", section: "Navigation", description: "Find project (fuzzy finder)" },
  { id: "escape-focus", key: "Esc", section: "Navigation", description: "Move focus up (terminal → session → project)", handledExternally: true },

  // ── Sessions ──
  { id: "create-session-claude", key: "c", section: "Sessions", description: "Create Claude session with issue" },
  { id: "create-session-codex", key: "x", section: "Sessions", description: "Create Codex session with issue" },
  { id: "background-worker-claude", key: "C", section: "Sessions", description: "Background worker: Claude (autonomous)" },
  { id: "background-worker-codex", key: "X", section: "Sessions", description: "Background worker: Codex (autonomous)" },
  { id: "finish-branch", key: "m", section: "Sessions", description: "Merge session branch (create PR)" },
  { id: "screenshot", key: "⌘S", section: "Sessions", description: "Screenshot app → new session with image", handledExternally: true },

  // ── Projects ──
  { id: "new-project", key: "n", section: "Projects", description: "New project" },
  { id: "delete", key: "d", section: "Projects", description: "Delete focused item (session or project)" },
  { id: "archive", key: "a", section: "Projects", description: "Archive focused item (session or project)" },
  { id: "toggle-archive-view", key: "A", section: "Projects", description: "View archived projects" },
  { id: "create-issue", key: "i", section: "Projects", description: "Create GitHub issue for focused project" },
  { id: "triage-untriaged", key: "t", section: "Projects", description: "Triage issues (untriaged)" },
  { id: "triage-triaged", key: "T", section: "Projects", description: "View triaged issues" },

  // ── Panels ──
  { id: "toggle-sidebar", key: "s", section: "Panels", description: "Toggle sidebar" },
  { id: "toggle-maintainer-panel", key: "b", section: "Panels", description: "Toggle background agent panel" },
  { id: "toggle-maintainer", key: "o", section: "Panels", description: "Toggle maintainer on/off (when panel open)" },
  { id: "trigger-maintainer-check", key: "r", section: "Panels", description: "Run maintainer check now (when panel open)" },
  { id: "toggle-help", key: "?", section: "Panels", description: "Toggle this help" },
  { id: "keystroke-visualizer", key: "⌘K", section: "Panels", description: "Toggle keystroke visualizer", handledExternally: true },
];

// Section order for help display
const SECTION_ORDER: CommandSection[] = ["Navigation", "Sessions", "Projects", "Panels"];

export interface HelpEntry {
  key: string;
  description: string;
}

export interface HelpSection {
  label: string;
  entries: HelpEntry[];
}

export function getHelpSections(): HelpSection[] {
  return SECTION_ORDER.map(section => ({
    label: section,
    entries: commands
      .filter(c => c.section === section && !c.hidden)
      .map(c => ({ key: c.helpKey ?? c.key, description: c.description })),
  }));
}

// Build key→CommandId map for handleHotkey (excludes external commands)
export function buildKeyMap(): Map<string, CommandId> {
  const map = new Map<string, CommandId>();
  for (const cmd of commands) {
    if (cmd.handledExternally) continue;
    map.set(cmd.key, cmd.id as CommandId);
  }
  return map;
}
```

**Step 2: Commit**

```bash
git add src/lib/commands.ts
git commit -m "feat: add command registry with types and help derivation"
```

---

### Task 2: Write tests for the registry

**Files:**
- Create: `src/lib/commands.test.ts`

**Step 1: Write tests**

```ts
import { describe, it, expect } from "vitest";
import { commands, getHelpSections, buildKeyMap, type CommandId } from "./commands";

describe("command registry", () => {
  it("every non-external command has a unique key", () => {
    const internal = commands.filter(c => !c.handledExternally);
    const keys = internal.map(c => c.key);
    const unique = new Set(keys);
    // "l" and "Enter" both map to "expand-collapse" — they have different keys, so unique check passes
    expect(keys.length).toBe(unique.size);
  });

  it("every non-hidden command has a description", () => {
    for (const cmd of commands.filter(c => !c.hidden)) {
      expect(cmd.description.length).toBeGreaterThan(0);
    }
  });

  it("getHelpSections returns all four sections in order", () => {
    const sections = getHelpSections();
    expect(sections.map(s => s.label)).toEqual(["Navigation", "Sessions", "Projects", "Panels"]);
  });

  it("getHelpSections excludes hidden commands", () => {
    const sections = getHelpSections();
    const allEntries = sections.flatMap(s => s.entries);
    // "k", "K", "Enter" are hidden — their descriptions shouldn't appear as standalone entries
    // But their paired primary (j/k, J/K, l/Enter) should appear
    const keys = allEntries.map(e => e.key);
    expect(keys).toContain("j / k");
    expect(keys).not.toContain("k");
    expect(keys).toContain("l / Enter");
    expect(keys).not.toContain("Enter");
  });

  it("getHelpSections includes externally handled commands", () => {
    const sections = getHelpSections();
    const allKeys = sections.flatMap(s => s.entries.map(e => e.key));
    expect(allKeys).toContain("Esc");
    expect(allKeys).toContain("⌘S");
    expect(allKeys).toContain("⌘K");
  });

  it("buildKeyMap excludes external commands", () => {
    const map = buildKeyMap();
    expect(map.has("Esc")).toBe(false);
    expect(map.has("⌘S")).toBe(false);
    expect(map.has("⌘K")).toBe(false);
  });

  it("buildKeyMap includes all internal command keys", () => {
    const map = buildKeyMap();
    expect(map.get("j")).toBe("navigate-next");
    expect(map.get("k")).toBe("navigate-prev");
    expect(map.get("l")).toBe("expand-collapse");
    expect(map.get("Enter")).toBe("expand-collapse");
    expect(map.get("?")).toBe("toggle-help");
  });

  it("help sections match the original hardcoded sections", () => {
    const sections = getHelpSections();
    // Verify exact entry counts match the original HotkeyHelp.svelte
    const nav = sections.find(s => s.label === "Navigation")!;
    expect(nav.entries).toHaveLength(6); // j/k, J/K, l/Enter, g, f, Esc

    const sess = sections.find(s => s.label === "Sessions")!;
    expect(sess.entries).toHaveLength(6); // c, x, C, X, m, ⌘S

    const proj = sections.find(s => s.label === "Projects")!;
    expect(proj.entries).toHaveLength(7); // n, d, a, A, i, t, T

    const panels = sections.find(s => s.label === "Panels")!;
    expect(panels.entries).toHaveLength(6); // s, b, o, r, ?, ⌘K
  });
});
```

**Step 2: Run tests to verify they pass**

Run: `npx vitest run src/lib/commands.test.ts`
Expected: All pass

**Step 3: Commit**

```bash
git add src/lib/commands.test.ts
git commit -m "test: add command registry tests"
```

---

### Task 3: Update HotkeyHelp to use the registry

**Files:**
- Modify: `src/lib/HotkeyHelp.svelte:1-59` (script section)

**Step 1: Replace hardcoded sections with registry import**

Replace the entire `<script>` block. Remove the `Shortcut`, `Section` interfaces and hardcoded `sections` array. Import from registry instead:

```ts
<script lang="ts">
  import { onMount } from "svelte";
  import { getHelpSections } from "./commands";

  interface Props {
    onClose: () => void;
  }

  let { onClose }: Props = $props();

  const sections = getHelpSections();

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === "Escape") {
      e.preventDefault();
      e.stopPropagation();
      onClose();
    }
  }

  onMount(() => {
    window.addEventListener("keydown", handleKeydown, { capture: true });
    return () => {
      window.removeEventListener("keydown", handleKeydown, { capture: true });
    };
  });
</script>
```

The template references `section.shortcuts` — update to `section.entries`:

```svelte
{#each section.entries as { key, description }}
```

Also update the `Section` label reference — `section.label` stays the same since `HelpSection` uses `label`.

**Step 2: Run all tests**

Run: `npx vitest run`
Expected: All pass (existing HotkeyManager tests unchanged)

**Step 3: Commit**

```bash
git add src/lib/HotkeyHelp.svelte
git commit -m "refactor: derive HotkeyHelp sections from command registry"
```

---

### Task 4: Update HotkeyManager to use the registry

**Files:**
- Modify: `src/lib/HotkeyManager.svelte:1-10` (imports)
- Modify: `src/lib/HotkeyManager.svelte:251-354` (handleHotkey function)

**Step 1: Add import and build key map**

Add to imports:

```ts
import { buildKeyMap, type CommandId } from "./commands";
```

Add after the store derivations (around line 47):

```ts
const keyMap = buildKeyMap();
```

**Step 2: Replace handleHotkey switch from key strings to CommandId**

Replace the `handleHotkey` function body:

```ts
function handleHotkey(key: string): boolean {
  const id = keyMap.get(key);
  if (id === undefined) return false;

  switch (id) {
    case "navigate-next":
      navigateItem(1);
      return true;
    case "navigate-prev":
      navigateItem(-1);
      return true;
    case "navigate-project-next":
      navigateProject(1);
      return true;
    case "navigate-project-prev":
      navigateProject(-1);
      return true;
    case "jump-mode":
      enterJumpMode();
      return true;
    case "fuzzy-finder":
      dispatchAction({ type: "open-fuzzy-finder" });
      return true;
    case "new-project":
      dispatchAction({ type: "open-new-project" });
      return true;
    case "delete":
      dispatchDeleteAction();
      return true;
    case "archive":
      dispatchArchiveAction();
      return true;
    case "toggle-archive-view":
      dispatchAction({ type: "toggle-archive-view" });
      return true;
    case "create-session-claude":
      dispatchIssuePicker();
      return true;
    case "create-session-codex":
      dispatchIssuePicker({ kind: "codex" });
      return true;
    case "background-worker-claude":
      dispatchIssuePicker({ background: true });
      return true;
    case "background-worker-codex":
      dispatchIssuePicker({ kind: "codex", background: true });
      return true;
    case "finish-branch":
      if (activeId) {
        const proj = projectList.find((p) => p.sessions.some((s) => s.id === activeId));
        const sess = proj?.sessions.find((s) => s.id === activeId);
        dispatchHotkeyAction({ type: "finish-branch", sessionId: activeId, kind: sess?.kind });
      }
      return true;
    case "toggle-sidebar":
      sidebarVisible.update(v => !v);
      return true;
    case "create-issue":
      dispatchCreateIssue();
      return true;
    case "triage-untriaged":
      dispatchAction({ type: "toggle-triage-panel", category: "untriaged" });
      return true;
    case "triage-triaged":
      dispatchAction({ type: "toggle-triage-panel", category: "triaged" });
      return true;
    case "expand-collapse":
      if (currentFocus?.type === "project") {
        const next = new Set(expandedSet);
        if (next.has(currentFocus.projectId)) {
          next.delete(currentFocus.projectId);
        } else {
          next.add(currentFocus.projectId);
        }
        expandedProjects.set(next);
      } else if (currentFocus?.type === "session") {
        if (!isArchiveView) {
          activeSessionId.set(currentFocus.sessionId);
        }
        dispatchAction({ type: "focus-terminal" });
      }
      return true;
    case "toggle-maintainer":
      if (isMaintainerPanelVisible && getFocusedProject()) {
        dispatchAction({ type: "toggle-maintainer-enabled" });
        return true;
      }
      return false;
    case "trigger-maintainer-check":
      if (isMaintainerPanelVisible) {
        dispatchAction({ type: "trigger-maintainer-check" });
        return true;
      }
      return false;
    case "toggle-maintainer-panel":
      dispatchAction({ type: "toggle-maintainer-panel" });
      return true;
    case "toggle-help":
      dispatchAction({ type: "toggle-help" });
      return true;
    default: {
      const _exhaustive: never = id;
      return false;
    }
  }
}
```

**Step 3: Run all tests**

Run: `npx vitest run`
Expected: All existing HotkeyManager tests pass — behavior is identical, only the dispatch mechanism changed.

**Step 4: Commit**

```bash
git add src/lib/HotkeyManager.svelte
git commit -m "refactor: dispatch hotkeys via command registry with exhaustive switch"
```

---

### Task 5: Final verification

**Step 1: Run full test suite**

Run: `npx vitest run`
Expected: All tests pass

**Step 2: Run TypeScript check**

Run: `npx tsc --noEmit`
Expected: No errors. Adding a new `CommandId` member without a switch case would cause a compile error.

**Step 3: Manual smoke test (optional)**

Run: `npm run tauri dev`
- Press `?` — help dialog should show identical content to before
- Press `j`/`k` — navigation works
- Press `c` — issue picker opens
- Press `s` — sidebar toggles

**Step 4: Commit if any fixups needed, then done**
