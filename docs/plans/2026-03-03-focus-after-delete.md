# Focus After Delete Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** After deleting a session or project, move focus to the nearest sibling above in the sidebar.

**Architecture:** Pre-compute the focus target in Sidebar.svelte before invoking the Tauri delete command. After deletion and reload, apply the computed target via `focusTarget.set()`. This mirrors the existing `archiveSession()` pattern.

**Tech Stack:** Svelte 5 (runes), TypeScript, Vitest

---

### Task 1: Add focus redirect to session deletion

**Files:**
- Modify: `src/lib/Sidebar.svelte:311-328` (the `closeSession` function)

**Step 1: Write the failing test**

Create `src/lib/Sidebar.test.ts` with a unit test for the focus computation logic. Since Sidebar.svelte is tightly coupled to Tauri invocations and DOM, extract the focus computation into a pure helper function in `src/lib/focus-helpers.ts` that can be tested independently.

Create `src/lib/focus-helpers.ts`:

```typescript
import type { Project, FocusTarget } from "./stores";

/**
 * Compute the focus target after deleting a session.
 * Prefers the session above; falls back to parent project.
 */
export function focusAfterSessionDelete(
  projectList: Project[],
  projectId: string,
  sessionId: string,
  isArchiveView: boolean,
): FocusTarget {
  const project = projectList.find(p => p.id === projectId);
  if (!project) return null;
  const sessions = isArchiveView
    ? project.sessions.filter(s => s.archived)
    : project.sessions.filter(s => !s.archived);
  const idx = sessions.findIndex(s => s.id === sessionId);
  if (idx > 0) {
    return { type: "session", sessionId: sessions[idx - 1].id, projectId };
  }
  return { type: "project", projectId };
}
```

Create `src/lib/focus-helpers.test.ts`:

```typescript
import { describe, it, expect } from "vitest";
import { focusAfterSessionDelete } from "./focus-helpers";
import type { Project } from "./stores";

function makeProject(id: string, sessionIds: string[]): Project {
  return {
    id,
    name: `project-${id}`,
    repo_path: `/tmp/${id}`,
    created_at: "2026-01-01",
    archived: false,
    sessions: sessionIds.map(sid => ({
      id: sid,
      label: `session-${sid}`,
      worktree_path: null,
      worktree_branch: null,
      archived: false,
      kind: "claude",
    })),
  };
}

describe("focusAfterSessionDelete", () => {
  it("focuses the session above when deleting a non-first session", () => {
    const projects = [makeProject("p1", ["s1", "s2", "s3"])];
    const result = focusAfterSessionDelete(projects, "p1", "s2", false);
    expect(result).toEqual({ type: "session", sessionId: "s1", projectId: "p1" });
  });

  it("focuses the parent project when deleting the first session", () => {
    const projects = [makeProject("p1", ["s1", "s2"])];
    const result = focusAfterSessionDelete(projects, "p1", "s1", false);
    expect(result).toEqual({ type: "project", projectId: "p1" });
  });

  it("focuses the parent project when deleting the only session", () => {
    const projects = [makeProject("p1", ["s1"])];
    const result = focusAfterSessionDelete(projects, "p1", "s1", false);
    expect(result).toEqual({ type: "project", projectId: "p1" });
  });

  it("returns null for unknown project", () => {
    const projects = [makeProject("p1", ["s1"])];
    const result = focusAfterSessionDelete(projects, "unknown", "s1", false);
    expect(result).toBeNull();
  });
});
```

**Step 2: Run test to verify it fails**

Run: `npx vitest run src/lib/focus-helpers.test.ts`
Expected: FAIL — module `./focus-helpers` not found

**Step 3: Write the implementation**

Create `src/lib/focus-helpers.ts` with the `focusAfterSessionDelete` function shown above.

**Step 4: Run test to verify it passes**

Run: `npx vitest run src/lib/focus-helpers.test.ts`
Expected: All 4 tests PASS

**Step 5: Wire into Sidebar.svelte**

In `src/lib/Sidebar.svelte`, modify `closeSession()` to compute and apply focus before deletion:

```typescript
// At top of <script>, add import:
import { focusAfterSessionDelete, focusAfterProjectDelete } from "./focus-helpers";

// Replace closeSession function (lines 311-328):
async function closeSession(projectId: string, sessionId: string, deleteWorktree: boolean) {
  try {
    const list = isArchiveView ? archivedProjectList : projectList;
    const nextFocus = focusAfterSessionDelete(list, projectId, sessionId, isArchiveView);

    await invoke("close_session", { projectId, sessionId, deleteWorktree });
    sessionStatuses.update(m => {
      const next = new Map(m);
      next.delete(sessionId);
      return next;
    });
    activeSessionId.update(current => {
      if (current !== sessionId) return current;
      if (nextFocus?.type === "session") return nextFocus.sessionId;
      return null;
    });
    focusTarget.set(nextFocus);
    await loadProjects();
    if (isArchiveView) await loadArchivedProjects();
  } catch (e) {
    showToast(String(e), "error");
  }
}
```

**Step 6: Run all tests**

Run: `npx vitest run`
Expected: PASS

**Step 7: Commit**

```bash
git add src/lib/focus-helpers.ts src/lib/focus-helpers.test.ts src/lib/Sidebar.svelte
git commit -m "feat: redirect focus after session deletion (#10)"
```

---

### Task 2: Add focus redirect to project deletion

**Files:**
- Modify: `src/lib/focus-helpers.ts` — add `focusAfterProjectDelete`
- Modify: `src/lib/focus-helpers.test.ts` — add tests
- Modify: `src/lib/Sidebar.svelte:600-611` — wire into DeleteProjectModal's `onDeleted`

**Step 1: Write the failing test**

Add to `src/lib/focus-helpers.test.ts`:

```typescript
import { focusAfterSessionDelete, focusAfterProjectDelete } from "./focus-helpers";

describe("focusAfterProjectDelete", () => {
  it("focuses last session of project above when it's expanded and has sessions", () => {
    const projects = [
      makeProject("p1", ["s1", "s2"]),
      makeProject("p2", ["s3"]),
    ];
    const expanded = new Set(["p1"]);
    const result = focusAfterProjectDelete(projects, "p2", expanded, false);
    expect(result).toEqual({ type: "session", sessionId: "s2", projectId: "p1" });
  });

  it("focuses the project above when it's collapsed", () => {
    const projects = [
      makeProject("p1", ["s1"]),
      makeProject("p2", ["s3"]),
    ];
    const expanded = new Set<string>();
    const result = focusAfterProjectDelete(projects, "p2", expanded, false);
    expect(result).toEqual({ type: "project", projectId: "p1" });
  });

  it("focuses the project above when it has no visible sessions", () => {
    const projects = [
      makeProject("p1", []),
      makeProject("p2", ["s3"]),
    ];
    const expanded = new Set(["p1"]);
    const result = focusAfterProjectDelete(projects, "p2", expanded, false);
    expect(result).toEqual({ type: "project", projectId: "p1" });
  });

  it("returns null when deleting the topmost project", () => {
    const projects = [makeProject("p1", ["s1"])];
    const expanded = new Set<string>();
    const result = focusAfterProjectDelete(projects, "p1", expanded, false);
    expect(result).toBeNull();
  });

  it("returns null for unknown project", () => {
    const projects = [makeProject("p1", ["s1"])];
    const expanded = new Set<string>();
    const result = focusAfterProjectDelete(projects, "unknown", expanded, false);
    expect(result).toBeNull();
  });
});
```

**Step 2: Run test to verify it fails**

Run: `npx vitest run src/lib/focus-helpers.test.ts`
Expected: FAIL — `focusAfterProjectDelete` not exported

**Step 3: Write the implementation**

Add to `src/lib/focus-helpers.ts`:

```typescript
/**
 * Compute the focus target after deleting a project.
 * Prefers the last visible session of the project above (if expanded);
 * falls back to the project above; returns null if topmost.
 */
export function focusAfterProjectDelete(
  projectList: Project[],
  projectId: string,
  expandedProjects: Set<string>,
  isArchiveView: boolean,
): FocusTarget {
  const idx = projectList.findIndex(p => p.id === projectId);
  if (idx <= 0) return null;

  const prevProject = projectList[idx - 1];
  if (expandedProjects.has(prevProject.id)) {
    const sessions = isArchiveView
      ? prevProject.sessions.filter(s => s.archived)
      : prevProject.sessions.filter(s => !s.archived);
    if (sessions.length > 0) {
      const lastSession = sessions[sessions.length - 1];
      return { type: "session", sessionId: lastSession.id, projectId: prevProject.id };
    }
  }
  return { type: "project", projectId: prevProject.id };
}
```

**Step 4: Run test to verify it passes**

Run: `npx vitest run src/lib/focus-helpers.test.ts`
Expected: All tests PASS

**Step 5: Wire into Sidebar.svelte**

The `deleteTarget` is set before the modal opens, so we can compute the focus target when the modal's `onDeleted` fires. But at that point the project is already deleted and `projectList` is stale until reload. We need to compute BEFORE calling `loadProjects()`.

Modify the DeleteProjectModal `onDeleted` callback in Sidebar.svelte (lines 600-611):

```svelte
{#if deleteTarget}
  <DeleteProjectModal
    projectId={deleteTarget.id}
    projectName={deleteTarget.name}
    onDeleted={async () => {
      const list = isArchiveView ? archivedProjectList : projectList;
      const nextFocus = focusAfterProjectDelete(list, deleteTarget!.id, expandedProjectSet, isArchiveView);
      activeSessionId.update(current => {
        if (deleteTarget!.sessions.some(s => s.id === current)) return nextFocus?.type === "session" ? nextFocus.sessionId : null;
        return current;
      });
      deleteTarget = null;
      await loadProjects();
      if (isArchiveView) await loadArchivedProjects();
      focusTarget.set(nextFocus);
    }}
    onClose={() => (deleteTarget = null)}
  />
{/if}
```

**Step 6: Run all tests**

Run: `npx vitest run`
Expected: PASS

**Step 7: Commit**

```bash
git add src/lib/focus-helpers.ts src/lib/focus-helpers.test.ts src/lib/Sidebar.svelte
git commit -m "feat: redirect focus after project deletion (#10)"
```
