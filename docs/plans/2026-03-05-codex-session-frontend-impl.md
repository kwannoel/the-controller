# Codex Session Frontend + Jump Mode Simplification Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add 'x' keybinding to create codex sessions, and simplify jump mode to project-level only.

**Architecture:** Jump mode loses its session phase — pressing `g` then a label just focuses a project. The 'x' key in normal hotkey mode creates a codex session by passing `kind: "codex"` through the existing `create-session` action to the Tauri backend (which already supports the `kind` parameter).

**Tech Stack:** Svelte 5, TypeScript, Vitest

---

### Task 1: Simplify `JumpPhase` type and add `kind` to TS types

**Files:**
- Modify: `src/lib/stores.ts:3-9` (add `kind` to `SessionConfig`)
- Modify: `src/lib/stores.ts:35` (add `kind` to `create-session` action)
- Modify: `src/lib/stores.ts:66-69` (simplify `JumpPhase`)

**Step 1: Update types**

In `src/lib/stores.ts`:

Add `kind` to `SessionConfig`:
```typescript
export interface SessionConfig {
  id: string;
  label: string;
  worktree_path: string | null;
  worktree_branch: string | null;
  archived: boolean;
  kind: string;
}
```

Add `kind` to the `create-session` action variant:
```typescript
| { type: "create-session"; projectId?: string; kind?: string }
```

Simplify `JumpPhase` to remove session variant:
```typescript
export type JumpPhase =
  | { phase: "project" }
  | null;
```

**Step 2: Run type check to verify compilation**

Run: `cd /Users/noel/.the-controller/worktrees/the-controller/session-2 && npx svelte-check 2>&1 | tail -20`
Expected: Type errors in files that reference `jumpState.phase === 'session'` or `jumpState.projectId` — this is expected and will be fixed in subsequent tasks.

**Step 3: Commit**

```bash
git add src/lib/stores.ts
git commit -m "feat: add kind to TS types, simplify JumpPhase to project-only"
```

---

### Task 2: Simplify jump mode in HotkeyManager — remove session phase

**Files:**
- Modify: `src/lib/HotkeyManager.svelte:44-228`

**Step 1: Write failing tests**

In `src/lib/HotkeyManager.test.ts`, replace the entire `describe('jump mode', ...)` block (lines 324-548) with:

```typescript
  describe('jump mode', () => {
    it('g enters jump mode (project phase)', () => {
      pressKey('g');
      expect(get(jumpMode)).toEqual({ phase: 'project' });
    });

    it('g then z focuses first project and exits jump mode', () => {
      pressKey('g');
      pressKey('z');
      expect(get(focusTarget)).toEqual({ type: 'project', projectId: 'proj-1' });
      expect(get(jumpMode)).toBeNull();
    });

    it('g then x focuses second project and exits jump mode', () => {
      projects.set([testProject, testProject2]);
      pressKey('g');
      pressKey('x');
      expect(get(focusTarget)).toEqual({ type: 'project', projectId: 'proj-2' });
      expect(get(jumpMode)).toBeNull();
    });

    it('g then Escape cancels jump mode', () => {
      pressKey('g');
      expect(get(jumpMode)).toEqual({ phase: 'project' });
      pressKey('Escape');
      expect(get(jumpMode)).toBeNull();
    });

    it('g then unrecognized key cancels jump mode', () => {
      pressKey('g');
      expect(get(jumpMode)).toEqual({ phase: 'project' });
      pressKey('q');
      expect(get(jumpMode)).toBeNull();
    });

    it('two-char labels work for >6 projects', () => {
      const manyProjects = Array.from({ length: 7 }, (_, i) => ({
        id: `proj-${i}`,
        name: `project-${i}`,
        repo_path: `/tmp/p${i}`,
        created_at: '2026-01-01',
        archived: false,
        sessions: [
          { id: `sess-${i}`, label: 'session-1', worktree_path: null, worktree_branch: null, archived: false, kind: 'claude' },
        ],
      }));
      projects.set(manyProjects);

      pressKey('g');
      expect(get(jumpMode)).toEqual({ phase: 'project' });

      // 'z' is a prefix of 'zz', 'zx', etc — should stay in jump mode
      pressKey('z');
      expect(get(jumpMode)).toEqual({ phase: 'project' });

      // 'zz' matches first project
      pressKey('z');
      expect(get(focusTarget)).toEqual({ type: 'project', projectId: 'proj-0' });
      expect(get(jumpMode)).toBeNull();
    });

    it('two-char label second key selects correct project', () => {
      const manyProjects = Array.from({ length: 7 }, (_, i) => ({
        id: `proj-${i}`,
        name: `project-${i}`,
        repo_path: `/tmp/p${i}`,
        created_at: '2026-01-01',
        archived: false,
        sessions: [
          { id: `sess-${i}`, label: 'session-1', worktree_path: null, worktree_branch: null, archived: false, kind: 'claude' },
        ],
      }));
      projects.set(manyProjects);

      pressKey('g');
      pressKey('z');
      pressKey('x');
      expect(get(focusTarget)).toEqual({ type: 'project', projectId: 'proj-1' });
      expect(get(jumpMode)).toBeNull();
    });

    it('g with no projects does nothing', () => {
      projects.set([]);
      pressKey('g');
      expect(get(jumpMode)).toBeNull();
    });
  });
```

**Step 2: Run tests to verify they fail**

Run: `cd /Users/noel/.the-controller/worktrees/the-controller/session-2 && npx vitest run src/lib/HotkeyManager.test.ts 2>&1 | tail -30`
Expected: FAIL — jump mode still has session phase behavior

**Step 3: Simplify HotkeyManager jump mode logic**

In `src/lib/HotkeyManager.svelte`:

Remove these state variables (lines 46-48):
```typescript
// DELETE these lines:
  let jumpPhase: "project" | "session" = $state("project");
  let jumpProjectId: string | null = $state(null);
```

Update `enterJumpMode` (line 134-144):
```typescript
  function enterJumpMode() {
    clearDwellTimer();
    const list = getJumpProjects();
    if (list.length === 0) return;
    jumpActive = true;
    jumpBuffer = "";
    jumpLabels = generateJumpLabels(list.length);
    jumpMode.set({ phase: "project" });
  }
```

Update `exitJumpMode` (line 146-153):
```typescript
  function exitJumpMode() {
    jumpActive = false;
    jumpBuffer = "";
    jumpLabels = [];
    jumpMode.set(null);
  }
```

Replace `handleJumpKey` (line 155-228) with:
```typescript
  function handleJumpKey(key: string) {
    if (key === "Escape") {
      exitJumpMode();
      return;
    }

    if (!JUMP_KEYS.includes(key)) {
      exitJumpMode();
      return;
    }

    jumpBuffer += key;

    // Check for exact match
    const matchIndex = jumpLabels.indexOf(jumpBuffer);
    if (matchIndex !== -1) {
      const list = getJumpProjects();
      const project = list[matchIndex];
      if (project) {
        focusTarget.set({ type: "project", projectId: project.id });
      }
      exitJumpMode();
      return;
    }

    // Check if buffer is a valid prefix of any label
    const isPrefix = jumpLabels.some((l) => l.startsWith(jumpBuffer));
    if (!isPrefix) {
      exitJumpMode();
    }
  }
```

**Step 4: Run tests to verify they pass**

Run: `cd /Users/noel/.the-controller/worktrees/the-controller/session-2 && npx vitest run src/lib/HotkeyManager.test.ts 2>&1 | tail -30`
Expected: ALL PASS

**Step 5: Commit**

```bash
git add src/lib/HotkeyManager.svelte src/lib/HotkeyManager.test.ts
git commit -m "refactor: simplify jump mode to project-level only"
```

---

### Task 3: Remove session jump labels and rendering from Sidebar

**Files:**
- Modify: `src/lib/Sidebar.svelte:114-132` (remove `sessionJumpLabels` derived, remove auto-expand effect)
- Modify: `src/lib/Sidebar.svelte:505-510` (remove session jump label rendering in archive view)
- Modify: `src/lib/Sidebar.svelte:560-573` (remove session jump label rendering + "New session" option in active view)

**Step 1: Remove session jump infrastructure from Sidebar**

In `src/lib/Sidebar.svelte`:

Delete the `sessionJumpLabels` derived (lines 114-125):
```typescript
// DELETE this entire block:
  let sessionJumpLabels = $derived.by(() => {
    const js = jumpState;
    if (!js || js.phase !== 'session') return [];
    ...
  });
```

Delete the auto-expand effect (lines 127-133):
```typescript
// DELETE this entire block:
  // Auto-expand project when entering session jump phase
  $effect(() => {
    if (jumpState?.phase === 'session' && !expandedProjectSet.has(jumpState.projectId)) {
      ...
    }
  });
```

Remove the `JumpPhase` import from the stores import line (line 4) — it's no longer needed in Sidebar since we only check `jumpState?.phase === 'project'` which doesn't need the type.

In the archive view template, remove session jump label rendering (around line 505-509):
```svelte
<!-- DELETE these lines inside the archived session item: -->
{#if jumpState?.phase === 'session' && jumpState.projectId === project.id && sessionJumpLabels[sessionIdx]}
  <kbd class="jump-label">{sessionJumpLabels[sessionIdx]}</kbd>
{/if}
```

In the active view template, remove session jump label rendering (around line 560-564):
```svelte
<!-- DELETE these lines inside the active session item: -->
{#if jumpState?.phase === 'session' && jumpState.projectId === project.id && sessionJumpLabels[sessionIdx]}
  <kbd class="jump-label">{sessionJumpLabels[sessionIdx]}</kbd>
{/if}
```

Also remove the "New session" jump option block (around line 567-573):
```svelte
<!-- DELETE this entire block: -->
{#if jumpState?.phase === 'session' && jumpState.projectId === project.id}
  <div class="session-item create-option">
    <span class="status-dot">+</span>
    <span class="session-label">New session</span>
    <kbd class="jump-label">{sessionJumpLabels[activeSessions.length]}</kbd>
  </div>
{/if}
```

**Step 2: Run type check**

Run: `cd /Users/noel/.the-controller/worktrees/the-controller/session-2 && npx svelte-check 2>&1 | tail -20`
Expected: No errors

**Step 3: Commit**

```bash
git add src/lib/Sidebar.svelte
git commit -m "refactor: remove session-phase jump labels from Sidebar"
```

---

### Task 4: Add 'x' keybinding and pass `kind` through Sidebar

**Files:**
- Modify: `src/lib/HotkeyManager.svelte:341-347` (add `case "x":`)
- Modify: `src/lib/Sidebar.svelte:147-154` (pass `action.kind` to createSession)
- Modify: `src/lib/Sidebar.svelte:325-341` (accept `kind` param, pass to invoke)

**Step 1: Write failing tests**

In `src/lib/HotkeyManager.test.ts`, add these tests inside the `describe('collapse/expand', ...)` block (after the `c on session` test, around line 707):

```typescript
    it('x on project dispatches create-session with kind codex', () => {
      focusTarget.set({ type: 'project', projectId: 'proj-1' });
      let captured: any = null;
      const unsub = hotkeyAction.subscribe((v) => { captured = v; });
      pressKey('x');
      expect(captured).toEqual({ type: 'create-session', projectId: 'proj-1', kind: 'codex' });
      unsub();
    });

    it('x on session dispatches create-session with kind codex for that project', () => {
      focusTarget.set({ type: 'session', sessionId: 'sess-1', projectId: 'proj-1' });
      let captured: any = null;
      const unsub = hotkeyAction.subscribe((v) => { captured = v; });
      pressKey('x');
      expect(captured).toEqual({ type: 'create-session', projectId: 'proj-1', kind: 'codex' });
      unsub();
    });

    it('x with no focus does nothing', () => {
      focusTarget.set(null);
      let captured: any = null;
      const unsub = hotkeyAction.subscribe((v) => { captured = v; });
      pressKey('x');
      expect(captured).toBeNull();
      unsub();
    });
```

**Step 2: Run tests to verify they fail**

Run: `cd /Users/noel/.the-controller/worktrees/the-controller/session-2 && npx vitest run src/lib/HotkeyManager.test.ts 2>&1 | tail -20`
Expected: FAIL — 'x' key is not handled in `handleHotkey`

**Step 3: Add 'x' handler in HotkeyManager**

In `src/lib/HotkeyManager.svelte`, add after the `case "c":` block (after line 347):

```typescript
      case "x":
        if (currentFocus?.type === "project") {
          dispatchAction({ type: "create-session", projectId: currentFocus.projectId, kind: "codex" });
        } else if (currentFocus?.type === "session") {
          dispatchAction({ type: "create-session", projectId: currentFocus.projectId, kind: "codex" });
        }
        return true;
```

**Step 4: Run tests to verify they pass**

Run: `cd /Users/noel/.the-controller/worktrees/the-controller/session-2 && npx vitest run src/lib/HotkeyManager.test.ts 2>&1 | tail -20`
Expected: ALL PASS

**Step 5: Pass `kind` through Sidebar**

In `src/lib/Sidebar.svelte`, update the `create-session` handler (around line 147-154):
```typescript
        case "create-session": {
          const project = action.projectId
            ? projectList.find((p) => p.id === action.projectId)
            : (projectList.find((p) =>
                p.sessions.some((s) => s.id === activeSession),
              ) ?? projectList[0]);
          if (project) createSession(project.id, action.kind);
          break;
        }
```

Update the `createSession` function signature and invoke call (around line 325):
```typescript
  async function createSession(projectId: string, kind?: string) {
    try {
      const sessionId: string = await invoke("create_session", {
        projectId,
        kind: kind ?? "claude",
      });
```

**Step 6: Run type check and all tests**

Run: `cd /Users/noel/.the-controller/worktrees/the-controller/session-2 && npx svelte-check 2>&1 | tail -10 && npx vitest run 2>&1 | tail -20`
Expected: No type errors, ALL PASS

**Step 7: Commit**

```bash
git add src/lib/HotkeyManager.svelte src/lib/HotkeyManager.test.ts src/lib/Sidebar.svelte
git commit -m "feat: add 'x' keybinding for codex sessions, pass kind through Sidebar"
```

---

### Task 5: Update test fixtures to include `kind` field

**Files:**
- Modify: `src/lib/HotkeyManager.test.ts:8-30` (add `kind` to test session objects)

**Step 1: Add `kind` to test fixtures**

In `src/lib/HotkeyManager.test.ts`, update `testProject` and `testProject2` session objects to include `kind: 'claude'`:

```typescript
const testProject = {
  id: 'proj-1',
  name: 'test-project',
  repo_path: '/tmp/test',
  created_at: '2026-01-01',
  archived: false,
  sessions: [
    { id: 'sess-1', label: 'session-1', worktree_path: null, worktree_branch: null, archived: false, kind: 'claude' },
    { id: 'sess-2', label: 'session-2', worktree_path: null, worktree_branch: null, archived: false, kind: 'claude' },
  ],
};

const testProject2 = {
  id: 'proj-2',
  name: 'other-project',
  repo_path: '/tmp/other',
  created_at: '2026-01-01',
  archived: false,
  sessions: [
    { id: 'sess-3', label: 'session-1', worktree_path: null, worktree_branch: null, archived: false, kind: 'claude' },
    { id: 'sess-4', label: 'session-2', worktree_path: null, worktree_branch: null, archived: false, kind: 'claude' },
  ],
};
```

**Step 2: Run all tests**

Run: `cd /Users/noel/.the-controller/worktrees/the-controller/session-2 && npx vitest run 2>&1 | tail -20`
Expected: ALL PASS

**Step 3: Also run backend tests for completeness**

Run: `cd /Users/noel/.the-controller/worktrees/the-controller/session-2/src-tauri && cargo test 2>&1 | tail -20`
Expected: ALL PASS

**Step 4: Commit**

```bash
git add src/lib/HotkeyManager.test.ts
git commit -m "test: add kind field to test fixtures"
```
