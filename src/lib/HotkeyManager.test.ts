import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { render, cleanup } from '@testing-library/svelte';
import { get } from 'svelte/store';
import { invoke } from '@tauri-apps/api/core';
import { projects, activeSessionId, hotkeyAction, focusTarget, jumpMode, sidebarVisible, expandedProjects, maintainerPanelVisible, sessionThinkingLevels, sessionStatuses } from './stores';
import HotkeyManager from './HotkeyManager.svelte';

const testProject = {
  id: 'proj-1',
  name: 'test-project',
  repo_path: '/tmp/test',
  created_at: '2026-01-01',
  archived: false,
  maintainer: { enabled: false, interval_minutes: 60 },
  auto_worker: { enabled: false },
  sessions: [
    { id: 'sess-1', label: 'session-1', worktree_path: null, worktree_branch: null, archived: false, kind: 'claude', github_issue: null, auto_worker_session: false },
    { id: 'sess-2', label: 'session-2', worktree_path: null, worktree_branch: null, archived: false, kind: 'claude', github_issue: null, auto_worker_session: false },
  ],
};

const testProject2 = {
  id: 'proj-2',
  name: 'other-project',
  repo_path: '/tmp/other',
  created_at: '2026-01-01',
  archived: false,
  maintainer: { enabled: false, interval_minutes: 60 },
  auto_worker: { enabled: false },
  sessions: [
    { id: 'sess-3', label: 'session-1', worktree_path: null, worktree_branch: null, archived: false, kind: 'claude', github_issue: null, auto_worker_session: false },
    { id: 'sess-4', label: 'session-2', worktree_path: null, worktree_branch: null, archived: false, kind: 'claude', github_issue: null, auto_worker_session: false },
  ],
};

function pressKey(key: string) {
  window.dispatchEvent(new KeyboardEvent('keydown', { key, bubbles: true }));
}

/** Create a fake xterm element and focus it to simulate terminal focus. */
function simulateTerminalFocus(): HTMLElement {
  const xterm = document.createElement('div');
  xterm.className = 'xterm';
  const textarea = document.createElement('textarea');
  xterm.appendChild(textarea);
  document.body.appendChild(xterm);
  textarea.focus();
  return xterm;
}

function removeTerminalFocus(xtermEl: HTMLElement) {
  (document.activeElement as HTMLElement)?.blur();
  xtermEl.remove();
}

describe('HotkeyManager', () => {
  beforeEach(() => {
    projects.set([testProject]);
    activeSessionId.set('sess-1');
    hotkeyAction.set(null);
    focusTarget.set(null);
    jumpMode.set(null);
    sidebarVisible.set(true);
    maintainerPanelVisible.set(false);
    expandedProjects.set(new Set(['proj-1', 'proj-2']));
    sessionThinkingLevels.set(new Map());
    sessionStatuses.set(new Map());
    vi.clearAllMocks();
    render(HotkeyManager);
  });

  afterEach(() => {
    cleanup();
  });

  // ── Ambient mode (no terminal focused) ──

  describe('ambient mode', () => {
    it('f dispatches open-fuzzy-finder action', () => {
      let captured: any = null;
      const unsub = hotkeyAction.subscribe((v) => { captured = v; });
      pressKey('f');
      expect(captured).toEqual({ type: 'open-fuzzy-finder' });
      unsub();
    });

    it('n dispatches open-new-project action', () => {
      let captured: any = null;
      const unsub = hotkeyAction.subscribe((v) => { captured = v; });
      pressKey('n');
      expect(captured).toEqual({ type: 'open-new-project' });
      unsub();
    });

    it('? dispatches toggle-help action', () => {
      let captured: any = null;
      const unsub = hotkeyAction.subscribe((v) => { captured = v; });
      pressKey('?');
      expect(captured).toEqual({ type: 'toggle-help' });
      unsub();
    });

    it('d dispatches delete-project when no focus', () => {
      let captured: any = null;
      const unsub = hotkeyAction.subscribe((v) => { captured = v; });
      pressKey('d');
      expect(captured).toEqual({ type: 'delete-project' });
      unsub();
    });

    it('d dispatches delete-session when session focused', () => {
      focusTarget.set({ type: 'session', sessionId: 'sess-1', projectId: 'proj-1' });
      let captured: any = null;
      const unsub = hotkeyAction.subscribe((v) => { captured = v; });
      pressKey('d');
      expect(captured).toEqual({ type: 'delete-session', sessionId: 'sess-1', projectId: 'proj-1' });
      unsub();
    });

    it('d dispatches delete-project with projectId when project focused', () => {
      focusTarget.set({ type: 'project', projectId: 'proj-1' });
      let captured: any = null;
      const unsub = hotkeyAction.subscribe((v) => { captured = v; });
      pressKey('d');
      expect(captured).toEqual({ type: 'delete-project', projectId: 'proj-1' });
      unsub();
    });

    it('a dispatches archive-project when no focus', () => {
      let captured: any = null;
      const unsub = hotkeyAction.subscribe((v) => { captured = v; });
      pressKey('a');
      expect(captured).toEqual({ type: 'archive-project' });
      unsub();
    });

    it('a dispatches archive-session when session focused', () => {
      focusTarget.set({ type: 'session', sessionId: 'sess-1', projectId: 'proj-1' });
      let captured: any = null;
      const unsub = hotkeyAction.subscribe((v) => { captured = v; });
      pressKey('a');
      expect(captured).toEqual({ type: 'archive-session', sessionId: 'sess-1', projectId: 'proj-1' });
      unsub();
    });

    it('m dispatches finish-branch action instead of writing directly', () => {
      let captured: any = null;
      const unsub = hotkeyAction.subscribe((v) => { captured = v; });
      pressKey('m');
      expect(captured).toEqual({ type: 'finish-branch', sessionId: 'sess-1', kind: 'claude' });
      expect(invoke).not.toHaveBeenCalled();
      unsub();
    });

    it('m dispatches finish-branch with codex kind', () => {
      projects.set([
        {
          ...testProject,
          sessions: [
            { ...testProject.sessions[0], kind: 'codex' },
          ],
        },
      ]);
      activeSessionId.set('sess-1');

      let captured: any = null;
      const unsub = hotkeyAction.subscribe((v) => { captured = v; });
      pressKey('m');
      expect(captured).toEqual({ type: 'finish-branch', sessionId: 'sess-1', kind: 'codex' });
      expect(invoke).not.toHaveBeenCalled();
      unsub();
    });

    it('modifier keys alone do not dispatch', () => {
      const initial = get(activeSessionId);
      pressKey('Shift');
      pressKey('Control');
      pressKey('Alt');
      pressKey('Meta');
      expect(get(activeSessionId)).toBe(initial);
      expect(get(hotkeyAction)).toBeNull();
    });

    it('Escape with no focus does nothing', () => {
      pressKey('Escape');
      expect(get(focusTarget)).toBeNull();
      expect(get(hotkeyAction)).toBeNull();
    });

    it('Escape with session focus moves to project focus', () => {
      focusTarget.set({ type: 'session', sessionId: 'sess-1', projectId: 'proj-1' });
      pressKey('Escape');
      expect(get(focusTarget)).toEqual({ type: 'project', projectId: 'proj-1' });
    });

    it('Escape with project focus stays on project', () => {
      focusTarget.set({ type: 'project', projectId: 'proj-1' });
      pressKey('Escape');
      expect(get(focusTarget)).toEqual({ type: 'project', projectId: 'proj-1' });
    });

    it('s toggles sidebarVisible', () => {
      expect(get(sidebarVisible)).toBe(true);
      pressKey('s');
      expect(get(sidebarVisible)).toBe(false);
      pressKey('s');
      expect(get(sidebarVisible)).toBe(true);
    });

    it('unrecognized keys do not change state', () => {
      const initial = get(activeSessionId);
      pressKey('w');
      pressKey('y');
      expect(get(activeSessionId)).toBe(initial);
      expect(get(hotkeyAction)).toBeNull();
    });

  });

  // ── j/k session navigation ──

  describe('j/k item navigation', () => {
    // Flat order for testProject: proj-1, sess-1, sess-2

    it('j from project moves to first session', () => {
      focusTarget.set({ type: 'project', projectId: 'proj-1' });
      pressKey('j');
      expect(get(focusTarget)).toEqual({ type: 'session', sessionId: 'sess-1', projectId: 'proj-1' });
      expect(get(activeSessionId)).toBe('sess-1');
    });

    it('j from session moves to next session', () => {
      focusTarget.set({ type: 'session', sessionId: 'sess-1', projectId: 'proj-1' });
      pressKey('j');
      expect(get(focusTarget)).toEqual({ type: 'session', sessionId: 'sess-2', projectId: 'proj-1' });
      expect(get(activeSessionId)).toBe('sess-2');
    });

    it('k from session moves to project header', () => {
      focusTarget.set({ type: 'session', sessionId: 'sess-1', projectId: 'proj-1' });
      pressKey('k');
      expect(get(focusTarget)).toEqual({ type: 'project', projectId: 'proj-1' });
    });

    it('j wraps from last item to first project', () => {
      focusTarget.set({ type: 'session', sessionId: 'sess-2', projectId: 'proj-1' });
      pressKey('j');
      expect(get(focusTarget)).toEqual({ type: 'project', projectId: 'proj-1' });
    });

    it('k wraps from first project to last session', () => {
      focusTarget.set({ type: 'project', projectId: 'proj-1' });
      pressKey('k');
      expect(get(focusTarget)).toEqual({ type: 'session', sessionId: 'sess-2', projectId: 'proj-1' });
    });

    it('j crosses project boundary via project header', () => {
      // Flat order: proj-1, sess-1, sess-2, proj-2, sess-3, sess-4
      projects.set([testProject, testProject2]);
      focusTarget.set({ type: 'session', sessionId: 'sess-2', projectId: 'proj-1' });
      pressKey('j');
      expect(get(focusTarget)).toEqual({ type: 'project', projectId: 'proj-2' });
    });

    it('k crosses project boundary via last session of prev project', () => {
      projects.set([testProject, testProject2]);
      focusTarget.set({ type: 'project', projectId: 'proj-2' });
      pressKey('k');
      expect(get(focusTarget)).toEqual({ type: 'session', sessionId: 'sess-2', projectId: 'proj-1' });
    });

    it('j with no focus goes to first project', () => {
      focusTarget.set(null);
      pressKey('j');
      expect(get(focusTarget)).toEqual({ type: 'project', projectId: 'proj-1' });
    });

    it('j with empty projects does nothing', () => {
      projects.set([]);
      pressKey('j');
      expect(get(focusTarget)).toBeNull();
    });

    it('j on project with no sessions skips to next project', () => {
      // Flat order: proj-1 (no sessions), proj-2, sess-3, sess-4
      projects.set([{ ...testProject, sessions: [] }, testProject2]);
      focusTarget.set({ type: 'project', projectId: 'proj-1' });
      pressKey('j');
      expect(get(focusTarget)).toEqual({ type: 'project', projectId: 'proj-2' });
    });
  });

  // ── J/K project navigation ──

  describe('J/K project navigation', () => {
    it('J moves to next project', () => {
      projects.set([testProject, testProject2]);
      focusTarget.set({ type: 'project', projectId: 'proj-1' });
      pressKey('J');
      expect(get(focusTarget)).toEqual({ type: 'project', projectId: 'proj-2' });
    });

    it('K moves to prev project', () => {
      projects.set([testProject, testProject2]);
      focusTarget.set({ type: 'project', projectId: 'proj-2' });
      pressKey('K');
      expect(get(focusTarget)).toEqual({ type: 'project', projectId: 'proj-1' });
    });

    it('J wraps from last to first project', () => {
      projects.set([testProject, testProject2]);
      focusTarget.set({ type: 'project', projectId: 'proj-2' });
      pressKey('J');
      expect(get(focusTarget)).toEqual({ type: 'project', projectId: 'proj-1' });
    });

    it('K wraps from first to last project', () => {
      projects.set([testProject, testProject2]);
      focusTarget.set({ type: 'project', projectId: 'proj-1' });
      pressKey('K');
      expect(get(focusTarget)).toEqual({ type: 'project', projectId: 'proj-2' });
    });

    it('J from session focus moves to next project', () => {
      projects.set([testProject, testProject2]);
      focusTarget.set({ type: 'session', sessionId: 'sess-1', projectId: 'proj-1' });
      pressKey('J');
      expect(get(focusTarget)).toEqual({ type: 'project', projectId: 'proj-2' });
    });

    it('J with no focus goes to first project', () => {
      focusTarget.set(null);
      pressKey('J');
      expect(get(focusTarget)).toEqual({ type: 'project', projectId: 'proj-1' });
    });
  });

  // ── Jump mode (g) ──

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
        maintainer: { enabled: false, interval_minutes: 60 },
        auto_worker: { enabled: false },
        sessions: [
          { id: `sess-${i}`, label: 'session-1', worktree_path: null, worktree_branch: null, archived: false, kind: 'claude', github_issue: null, auto_worker_session: false },
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
        maintainer: { enabled: false, interval_minutes: 60 },
        auto_worker: { enabled: false },
        sessions: [
          { id: `sess-${i}`, label: 'session-1', worktree_path: null, worktree_branch: null, archived: false, kind: 'claude', github_issue: null, auto_worker_session: false },
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

  // ── Terminal escape (terminal focused) ──

  describe('terminal escape', () => {
    let xtermEl: HTMLElement;

    beforeEach(() => {
      xtermEl = simulateTerminalFocus();
    });

    afterEach(() => {
      removeTerminalFocus(xtermEl);
    });

    it('keys are ignored when terminal focused', () => {
      const initial = get(activeSessionId);
      pressKey('g');
      pressKey('c');
      pressKey('f');
      expect(get(activeSessionId)).toBe(initial);
      expect(get(hotkeyAction)).toBeNull();
      expect(get(jumpMode)).toBeNull();
    });

    it('Escape sets focusTarget to active session', () => {
      pressKey('Escape');
      expect(get(focusTarget)).toEqual({
        type: 'session',
        sessionId: 'sess-1',
        projectId: 'proj-1',
      });
    });

    it('Escape then ambient hotkey works', () => {
      pressKey('Escape');

      removeTerminalFocus(xtermEl);
      xtermEl = document.createElement('div');

      let captured: any = null;
      const unsub = hotkeyAction.subscribe((v) => { captured = v; });
      pressKey('f');
      expect(captured).toEqual({ type: 'open-fuzzy-finder' });
      unsub();
    });

    it('Escape then g enters jump mode', () => {
      pressKey('Escape');

      removeTerminalFocus(xtermEl);
      xtermEl = document.createElement('div');

      pressKey('g');
      expect(get(jumpMode)).toEqual({ phase: 'project' });
    });

    it('double Escape forwards Escape to PTY', () => {
      const now = Date.now();
      vi.spyOn(Date, 'now').mockReturnValue(now);

      pressKey('Escape');

      vi.spyOn(Date, 'now').mockReturnValue(now + 50);
      pressKey('Escape');

      expect(invoke).toHaveBeenCalledWith('write_to_pty', {
        sessionId: 'sess-1',
        data: '\x1b',
      });

      vi.restoreAllMocks();
    });

    it('slow second Escape does not forward to PTY', () => {
      const now = Date.now();
      vi.spyOn(Date, 'now').mockReturnValue(now);

      pressKey('Escape');

      vi.spyOn(Date, 'now').mockReturnValue(now + 500);
      pressKey('Escape');

      expect(invoke).not.toHaveBeenCalledWith('write_to_pty', expect.anything());

      vi.restoreAllMocks();
    });
  });

  // ── Collapse/Expand ──

  describe('collapse/expand', () => {
    it('j skips sessions of collapsed project', () => {
      projects.set([testProject, testProject2]);
      expandedProjects.set(new Set(['proj-2'])); // proj-1 collapsed
      focusTarget.set({ type: 'project', projectId: 'proj-1' });
      pressKey('j');
      // Should skip sess-1, sess-2 and go to proj-2
      expect(get(focusTarget)).toEqual({ type: 'project', projectId: 'proj-2' });
    });

    it('k skips sessions of collapsed project', () => {
      projects.set([testProject, testProject2]);
      expandedProjects.set(new Set(['proj-1'])); // proj-2 collapsed
      focusTarget.set({ type: 'project', projectId: 'proj-2' });
      pressKey('k');
      // Should skip sess-3, sess-4 and go to sess-2 (last session of expanded proj-1)
      expect(get(focusTarget)).toEqual({ type: 'session', sessionId: 'sess-2', projectId: 'proj-1' });
    });

    it('j navigates only projects when all collapsed', () => {
      projects.set([testProject, testProject2]);
      expandedProjects.set(new Set()); // all collapsed
      focusTarget.set({ type: 'project', projectId: 'proj-1' });
      pressKey('j');
      expect(get(focusTarget)).toEqual({ type: 'project', projectId: 'proj-2' });
    });

    it('Enter on project toggles expand', () => {
      expandedProjects.set(new Set());
      focusTarget.set({ type: 'project', projectId: 'proj-1' });
      pressKey('Enter');
      expect(get(expandedProjects).has('proj-1')).toBe(true);
      pressKey('Enter');
      expect(get(expandedProjects).has('proj-1')).toBe(false);
    });

    it('Enter on session dispatches focus-terminal', () => {
      focusTarget.set({ type: 'session', sessionId: 'sess-1', projectId: 'proj-1' });
      let captured: any = null;
      const unsub = hotkeyAction.subscribe((v) => { captured = v; });
      pressKey('Enter');
      expect(captured).toEqual({ type: 'focus-terminal' });
      expect(get(activeSessionId)).toBe('sess-1');
      unsub();
    });

    it('Enter with no focus does nothing harmful', () => {
      focusTarget.set(null);
      pressKey('Enter');
      expect(get(hotkeyAction)).toBeNull();
    });

    it('c on project dispatches pick-issue-for-session', () => {
      focusTarget.set({ type: 'project', projectId: 'proj-1' });
      let captured: any = null;
      const unsub = hotkeyAction.subscribe((v) => { captured = v; });
      pressKey('c');
      expect(captured).toEqual({ type: 'pick-issue-for-session', projectId: 'proj-1', repoPath: '/tmp/test' });
      unsub();
    });

    it('c on session dispatches pick-issue-for-session for that project', () => {
      focusTarget.set({ type: 'session', sessionId: 'sess-1', projectId: 'proj-1' });
      let captured: any = null;
      const unsub = hotkeyAction.subscribe((v) => { captured = v; });
      pressKey('c');
      expect(captured).toEqual({ type: 'pick-issue-for-session', projectId: 'proj-1', repoPath: '/tmp/test' });
      unsub();
    });

    it('x on project dispatches pick-issue-for-session with kind codex', () => {
      focusTarget.set({ type: 'project', projectId: 'proj-1' });
      let captured: any = null;
      const unsub = hotkeyAction.subscribe((v) => { captured = v; });
      pressKey('x');
      expect(captured).toEqual({ type: 'pick-issue-for-session', projectId: 'proj-1', repoPath: '/tmp/test', kind: 'codex' });
      unsub();
    });

    it('x on session dispatches pick-issue-for-session with kind codex for that project', () => {
      focusTarget.set({ type: 'session', sessionId: 'sess-1', projectId: 'proj-1' });
      let captured: any = null;
      const unsub = hotkeyAction.subscribe((v) => { captured = v; });
      pressKey('x');
      expect(captured).toEqual({ type: 'pick-issue-for-session', projectId: 'proj-1', repoPath: '/tmp/test', kind: 'codex' });
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
  });

  // ── Toggle mode (o) ──

  describe('toggle mode (o)', () => {
    it('o then m dispatches toggle-maintainer-enabled', () => {
      let captured: any = null;
      const unsub = hotkeyAction.subscribe((v) => { captured = v; });
      pressKey('o');
      pressKey('m');
      expect(captured).toEqual({ type: 'toggle-maintainer-enabled' });
      unsub();
    });

    it('o then w dispatches toggle-auto-worker-enabled', () => {
      let captured: any = null;
      const unsub = hotkeyAction.subscribe((v) => { captured = v; });
      pressKey('o');
      pressKey('w');
      expect(captured).toEqual({ type: 'toggle-auto-worker-enabled' });
      unsub();
    });

    it('o then Escape cancels toggle mode', () => {
      let captured: any = null;
      const unsub = hotkeyAction.subscribe((v) => { captured = v; });
      pressKey('o');
      pressKey('Escape');
      expect(captured).toBeNull();
      unsub();
    });

    it('o then unrecognized key cancels toggle mode', () => {
      let captured: any = null;
      const unsub = hotkeyAction.subscribe((v) => { captured = v; });
      pressKey('o');
      pressKey('q');
      expect(captured).toBeNull();
      unsub();
    });

    it('o then m dispatches toggle-maintainer-enabled when focus is maintainer type', () => {
      maintainerPanelVisible.set(true);
      focusTarget.set({ type: 'maintainer' });
      let captured: any = null;
      const unsub = hotkeyAction.subscribe((v) => { captured = v; });
      pressKey('o');
      pressKey('m');
      expect(captured).toEqual({ type: 'toggle-maintainer-enabled' });
      unsub();
    });
  });

  // ── Thinking level cycle (e/q) ──

  describe('thinking level cycle', () => {
    it('e increases thinking level from default (medium) to high', () => {
      pressKey('e');
      expect(get(sessionThinkingLevels).get('sess-1')).toBe('high');
    });

    it('q decreases thinking level from default (medium) to low', () => {
      pressKey('q');
      expect(get(sessionThinkingLevels).get('sess-1')).toBe('low');
    });

    it('e cycles up through all levels and wraps: medium → high → max → low → medium', () => {
      pressKey('e'); // medium → high
      expect(get(sessionThinkingLevels).get('sess-1')).toBe('high');
      pressKey('e'); // high → max
      expect(get(sessionThinkingLevels).get('sess-1')).toBe('max');
      pressKey('e'); // max → low (wrap)
      expect(get(sessionThinkingLevels).get('sess-1')).toBe('low');
      pressKey('e'); // low → medium
      expect(get(sessionThinkingLevels).get('sess-1')).toBe('medium');
    });

    it('q cycles down through all levels and wraps: medium → low → max → high → medium', () => {
      pressKey('q'); // medium → low
      expect(get(sessionThinkingLevels).get('sess-1')).toBe('low');
      pressKey('q'); // low → max (wrap)
      expect(get(sessionThinkingLevels).get('sess-1')).toBe('max');
      pressKey('q'); // max → high
      expect(get(sessionThinkingLevels).get('sess-1')).toBe('high');
      pressKey('q'); // high → medium
      expect(get(sessionThinkingLevels).get('sess-1')).toBe('medium');
    });

    it('e writes /think command to PTY when session is idle', () => {
      sessionStatuses.set(new Map([['sess-1', 'idle']]));
      pressKey('e');
      expect(invoke).toHaveBeenCalledWith('write_to_pty', {
        sessionId: 'sess-1',
        data: '/think high\n',
      });
    });

    it('q writes /think command to PTY when session is idle', () => {
      sessionStatuses.set(new Map([['sess-1', 'idle']]));
      pressKey('q');
      expect(invoke).toHaveBeenCalledWith('write_to_pty', {
        sessionId: 'sess-1',
        data: '/think low\n',
      });
    });

    it('e does not write to PTY when session is working', () => {
      sessionStatuses.set(new Map([['sess-1', 'working']]));
      pressKey('e');
      expect(get(sessionThinkingLevels).get('sess-1')).toBe('high');
      expect(invoke).not.toHaveBeenCalled();
    });

    it('e does not write to PTY when session has no status', () => {
      pressKey('e');
      expect(get(sessionThinkingLevels).get('sess-1')).toBe('high');
      expect(invoke).not.toHaveBeenCalled();
    });
  });

  // ── Clear maintainer reports (c) ──

  describe('clear maintainer reports (c)', () => {
    it('c dispatches clear-maintainer-reports when panel visible', () => {
      maintainerPanelVisible.set(true);
      focusTarget.set({ type: 'project', projectId: 'proj-1' });
      let captured: any = null;
      const unsub = hotkeyAction.subscribe((v) => { captured = v; });
      pressKey('c');
      expect(captured).toEqual({ type: 'clear-maintainer-reports' });
      unsub();
    });

    it('c dispatches clear-maintainer-reports when focus is maintainer type', () => {
      maintainerPanelVisible.set(true);
      focusTarget.set({ type: 'maintainer' });
      let captured: any = null;
      const unsub = hotkeyAction.subscribe((v) => { captured = v; });
      pressKey('c');
      expect(captured).toEqual({ type: 'clear-maintainer-reports' });
      unsub();
    });

    it('c dispatches pick-issue-for-session when panel hidden', () => {
      maintainerPanelVisible.set(false);
      focusTarget.set({ type: 'project', projectId: 'proj-1' });
      let captured: any = null;
      const unsub = hotkeyAction.subscribe((v) => { captured = v; });
      pressKey('c');
      expect(captured).toEqual({ type: 'pick-issue-for-session', projectId: 'proj-1', repoPath: '/tmp/test' });
      unsub();
    });
  });

  // ── Input field passthrough ──

  describe('input field passthrough', () => {
    it('hotkeys are ignored when an input element is focused', () => {
      const input = document.createElement('input');
      document.body.appendChild(input);
      input.focus();

      const initial = get(activeSessionId);
      pressKey('g');
      pressKey('c');
      pressKey('f');
      expect(get(activeSessionId)).toBe(initial);
      expect(get(hotkeyAction)).toBeNull();

      input.blur();
      input.remove();
    });

    it('hotkeys are ignored when a textarea is focused', () => {
      const textarea = document.createElement('textarea');
      document.body.appendChild(textarea);
      textarea.focus();

      pressKey('g');
      expect(get(activeSessionId)).toBe('sess-1');
      expect(get(hotkeyAction)).toBeNull();

      textarea.blur();
      textarea.remove();
    });

    it('Escape does nothing when input is focused', () => {
      const input = document.createElement('input');
      document.body.appendChild(input);
      input.focus();

      pressKey('Escape');
      expect(get(focusTarget)).toBeNull();
      expect(get(hotkeyAction)).toBeNull();

      input.blur();
      input.remove();
    });

    it('hotkeys are ignored when a dialog is open', () => {
      const dialog = document.createElement('div');
      dialog.setAttribute('role', 'dialog');
      document.body.appendChild(dialog);

      try {
        focusTarget.set({ type: 'project', projectId: 'proj-1' });
        pressKey('j');
        expect(get(focusTarget)).toEqual({ type: 'project', projectId: 'proj-1' });
        expect(get(hotkeyAction)).toBeNull();
      } finally {
        dialog.remove();
      }
    });
  });
});
