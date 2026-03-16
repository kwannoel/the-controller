# UI Consistency Fixes — Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use the-controller-executing-plans to implement this plan task-by-task.

**Goal:** Fix six design inconsistencies found during visual audit: sidebar header labels, empty state text/structure, kbd styles, footer spacer, and focus ring.

**Architecture:** Pure frontend changes across 4 Svelte components. No backend changes. Each task is independent — they can be done in any order.

**Tech Stack:** Svelte 5, CSS

---

### Task 1: Sidebar header — show all mode names

**Files:**
- Modify: `src/lib/Sidebar.svelte:611`

**Step 1: Edit the sidebar header**

In `src/lib/Sidebar.svelte`, replace the ternary chain on line 611:

```svelte
<!-- OLD -->
<h2>{currentMode === "agents" ? "Agents" : currentMode === "notes" ? "Notes" : "Development"}</h2>

<!-- NEW -->
<h2>{{ development: "Development", agents: "Agents", architecture: "Architecture", notes: "Notes", infrastructure: "Infrastructure", voice: "Voice" }[currentMode]}</h2>
```

**Step 2: Commit**

```bash
git add src/lib/Sidebar.svelte
git commit -m "fix: show correct mode name in sidebar header for all workspace modes"
```

---

### Task 2: Empty state text — standardize casing

**Files:**
- Modify: `src/lib/TerminalManager.svelte:64`
- Modify: `src/lib/AgentDashboard.svelte:526`

Only Development and Agents need text changes. Notes and Architecture are already correct.

**Step 1: Fix TerminalManager empty state text**

In `src/lib/TerminalManager.svelte`, change line 64 from:

```svelte
<div class="empty-hint">Press <kbd>c</kbd> to create a session, or <kbd>n</kbd> to add a project</div>
```

to:

```svelte
<div class="empty-hint">press <kbd>c</kbd> to create a session, or <kbd>n</kbd> to add a project</div>
```

**Step 2: Fix AgentDashboard empty state text**

In `src/lib/AgentDashboard.svelte`, change line 526 from:

```svelte
<div class="empty-hint">Navigate to an agent with <kbd>j</kbd> / <kbd>k</kbd> and press <kbd>l</kbd></div>
```

to:

```svelte
<div class="empty-hint">navigate to an agent with <kbd>j</kbd> / <kbd>k</kbd> and press <kbd>l</kbd></div>
```

**Step 3: Commit**

```bash
git add src/lib/TerminalManager.svelte src/lib/AgentDashboard.svelte
git commit -m "fix: standardize empty state hint text to lowercase"
```

---

### Task 3: Infrastructure empty state — two-line pattern + kbd fix

**Files:**
- Modify: `src/lib/InfrastructureDashboard.svelte:33-37` (template)
- Modify: `src/lib/InfrastructureDashboard.svelte:100-103` (styles)

**Step 1: Replace Infrastructure empty state template**

In `src/lib/InfrastructureDashboard.svelte`, replace lines 33-38:

```svelte
<!-- OLD -->
    <div class="empty-state">
      <div class="title">Infrastructure</div>
      <div class="subtitle">No services deployed yet</div>
      <div class="hint">Deploy a project with <kbd>d</kbd> from the infrastructure workspace</div>
    </div>

<!-- NEW -->
    <div class="empty-state">
      <div class="empty-title">No services deployed yet</div>
      <div class="empty-hint">press <kbd>d</kbd> to deploy a project</div>
    </div>
```

**Step 2: Replace Infrastructure empty state styles**

In `src/lib/InfrastructureDashboard.svelte`, replace the `.title`, `.subtitle`, `.hint`, and `kbd` rules (lines 100-103):

```css
/* OLD */
  .title { font-size: 18px; font-weight: 600; margin-bottom: 8px; }
  .subtitle { font-size: 14px; color: var(--text-secondary); margin-bottom: 16px; }
  .hint { font-size: 12px; color: var(--text-tertiary); }
  kbd { background: var(--bg-active); padding: 2px 6px; border-radius: 3px; font-family: monospace; font-size: 11px; }

/* NEW */
  .empty-title { color: var(--text-primary); font-size: 16px; font-weight: 500; margin-bottom: 8px; }
  .empty-hint { color: var(--text-secondary); font-size: 13px; }
  .empty-hint kbd { background: var(--bg-hover); color: var(--text-emphasis); padding: 1px 6px; border-radius: 3px; font-family: var(--font-mono); font-size: 12px; }
```

**Step 3: Commit**

```bash
git add src/lib/InfrastructureDashboard.svelte
git commit -m "fix: standardize Infrastructure empty state to two-line pattern with correct kbd style"
```

---

### Task 4: Footer spacer — remove dead space

**Files:**
- Modify: `src/lib/Sidebar.svelte:668-670` (template)
- Modify: `src/lib/Sidebar.svelte:881-883` (styles)

**Step 1: Remove footer-spacer div**

In `src/lib/Sidebar.svelte`, change lines 667-671 from:

```svelte
  <div class="sidebar-footer">
    <div class="footer-left">
      <div class="footer-spacer"></div>
      <div class="provider-indicator">Provider: {currentSessionProviderLabel}</div>
    </div>
```

to:

```svelte
  <div class="sidebar-footer">
    <div class="footer-left">
      <div class="provider-indicator">Provider: {currentSessionProviderLabel}</div>
    </div>
```

**Step 2: Remove footer-spacer CSS rule**

In `src/lib/Sidebar.svelte`, delete lines 881-883:

```css
  .footer-spacer {
    min-height: 31px;
  }
```

**Step 3: Commit**

```bash
git add src/lib/Sidebar.svelte
git commit -m "fix: remove footer spacer dead space in sidebar"
```

---

### Task 5: Help button focus ring — use app convention

**Files:**
- Modify: `src/lib/Sidebar.svelte` (style block, around line 894)

**Step 1: Add outline: none to .btn-help and add :focus-visible rule**

In `src/lib/Sidebar.svelte`, add `outline: none;` to the existing `.btn-help` rule (after `box-shadow: none;`), and add a new `.btn-help:focus-visible` rule after `.btn-help.active`:

```css
  .btn-help:focus-visible {
    outline: 2px solid var(--focus-ring);
    outline-offset: -2px;
  }
```

**Step 2: Commit**

```bash
git add src/lib/Sidebar.svelte
git commit -m "fix: replace browser-default blue focus ring with app convention on help button"
```

---

### Task 6: Update e2e tests + visual verification

**Files:**
- Modify: `e2e/specs/ui-consistency.spec.ts`

**Step 1: Update e2e test assertions**

The existing `ui-consistency.spec.ts` has assertions that match the old text. Update:

1. The Infrastructure empty state test checks for `"Infrastructure"` as `.title` text — change to check `.empty-title` with text `"No services deployed yet"` and `.empty-hint` with `"press d to deploy a project"`.

2. The Development empty state `kbd` test uses `".terminal-manager .empty-hint"` — the text casing changed from "Press" to "press", but the test only checks kbd bg color so no assertion change needed there.

**Step 2: Run the targeted e2e test**

```bash
./e2e/eval.sh "$WORKTREE_PATH" e2e/specs/ui-consistency.spec.ts
```

Expected: All 19 tests pass.

**Step 3: Run the visual audit test to capture new screenshots**

```bash
./e2e/eval.sh "$WORKTREE_PATH" e2e/specs/visual-audit.spec.ts
```

Expected: Passes. Review screenshots in `e2e/results/visual-audit/` to confirm:
- Sidebar headers say the correct mode name
- Infrastructure shows two-line empty state with visible kbd
- Footer has no dead space above provider indicator
- Help button focus ring is white (if focused)

**Step 4: Commit test updates**

```bash
git add e2e/specs/ui-consistency.spec.ts
git commit -m "test: update e2e assertions for UI consistency fixes"
```
