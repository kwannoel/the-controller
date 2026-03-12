import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { render, cleanup } from '@testing-library/svelte';
import { get } from 'svelte/store';
import { command } from '$lib/backend';
import { projects, activeSessionId, hotkeyAction, focusTarget, sidebarVisible, expandedProjects, workspaceMode, workspaceModePickerVisible, selectedSessionProvider, activeNote, noteEntries, type Project, type SessionConfig } from './stores';
import HotkeyManager from './HotkeyManager.svelte';

function makeSession(id: string, label: string, kind = 'claude'): SessionConfig {
  return {
    id,
    label,
    worktree_path: null,
    worktree_branch: null,
    archived: false,
    kind,
    github_issue: null,
    initial_prompt: null,
    auto_worker_session: false,
  };
}

function makeProject(id: string, name: string, repoPath: string, sessions: SessionConfig[]): Project {
  return {
    id,
    name,
    repo_path: repoPath,
    created_at: '2026-01-01',
    archived: false,
    maintainer: { enabled: false, interval_minutes: 60 },
    auto_worker: { enabled: false },
    sessions,
    prompts: [],
    staged_session: null,
  };
}

const testProject = makeProject(
  'proj-1',
  'test-project',
  '/tmp/test',
  [
    makeSession('sess-1', 'session-1'),
    makeSession('sess-2', 'session-2'),
  ],
);

const testProject2 = makeProject(
  'proj-2',
  'other-project',
  '/tmp/other',
  [
    makeSession('sess-3', 'session-1'),
    makeSession('sess-4', 'session-2'),
  ],
);

function pressKey(key: string) {
  window.dispatchEvent(new KeyboardEvent('keydown', { key, bubbles: true }));
}

function pressMetaKey(key: string) {
  window.dispatchEvent(new KeyboardEvent('keydown', { key, metaKey: true, bubbles: true }));
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
    sidebarVisible.set(true);
    expandedProjects.set(new Set(['proj-1', 'proj-2']));
    workspaceMode.set("development");
    workspaceModePickerVisible.set(false);
    selectedSessionProvider.set("claude");
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

    it('a does not dispatch any archive action', () => {
      focusTarget.set({ type: 'session', sessionId: 'sess-1', projectId: 'proj-1' });
      let captured: any = null;
      const unsub = hotkeyAction.subscribe((v) => { captured = v; });
      pressKey('a');
      expect(captured).toBeNull();
      unsub();
    });

    it('A does not dispatch archive-view actions', () => {
      let captured: any = null;
      const unsub = hotkeyAction.subscribe((v) => { captured = v; });
      pressKey('A');
      expect(captured).toBeNull();
      unsub();
    });

    it('m dispatches finish-branch action instead of writing directly', () => {
      let captured: any = null;
      const unsub = hotkeyAction.subscribe((v) => { captured = v; });
      pressKey('m');
      expect(captured).toEqual({ type: 'finish-branch', sessionId: 'sess-1', kind: 'claude' });
      expect(command).not.toHaveBeenCalled();
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
      expect(command).not.toHaveBeenCalled();
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

    it('double Escape forwards Escape to PTY', () => {
      const now = Date.now();
      vi.spyOn(Date, 'now').mockReturnValue(now);

      pressKey('Escape');

      vi.spyOn(Date, 'now').mockReturnValue(now + 50);
      pressKey('Escape');

      expect(command).toHaveBeenCalledWith('write_to_pty', {
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

      expect(command).not.toHaveBeenCalledWith('write_to_pty', expect.anything());

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

    it('c on project dispatches pick-issue-for-session for the selected provider', () => {
      focusTarget.set({ type: 'project', projectId: 'proj-1' });
      let captured: any = null;
      const unsub = hotkeyAction.subscribe((v) => { captured = v; });
      pressKey('c');
      expect(captured).toEqual({ type: 'pick-issue-for-session', projectId: 'proj-1', repoPath: '/tmp/test', kind: 'claude' });
      unsub();
    });

    it('c on session dispatches pick-issue-for-session for that project', () => {
      focusTarget.set({ type: 'session', sessionId: 'sess-1', projectId: 'proj-1' });
      let captured: any = null;
      const unsub = hotkeyAction.subscribe((v) => { captured = v; });
      pressKey('c');
      expect(captured).toEqual({ type: 'pick-issue-for-session', projectId: 'proj-1', repoPath: '/tmp/test', kind: 'claude' });
      unsub();
    });

    it('c uses codex after provider toggle', () => {
      focusTarget.set({ type: 'project', projectId: 'proj-1' });
      selectedSessionProvider.set('codex');
      let captured: any = null;
      const unsub = hotkeyAction.subscribe((v) => { captured = v; });
      pressKey('c');
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

    it('X with no focus does nothing', () => {
      focusTarget.set(null);
      let captured: any = null;
      const unsub = hotkeyAction.subscribe((v) => { captured = v; });
      pressKey('X');
      expect(captured).toBeNull();
      unsub();
    });

    it('C with no focus does nothing', () => {
      focusTarget.set(null);
      let captured: any = null;
      const unsub = hotkeyAction.subscribe((v) => { captured = v; });
      pressKey('C');
      expect(captured).toBeNull();
      unsub();
    });
  });

  // ── Toggle mode (o) ──

  // ── c key in development mode ──

  describe('c key in development mode', () => {
    it('c dispatches pick-issue-for-session when project focused', () => {
      focusTarget.set({ type: 'project', projectId: 'proj-1' });
      let captured: any = null;
      const unsub = hotkeyAction.subscribe((v) => { captured = v; });
      pressKey('c');
      expect(captured).toEqual({ type: 'pick-issue-for-session', projectId: 'proj-1', repoPath: '/tmp/test', kind: 'claude' });
      unsub();
    });

    it('Cmd+T toggles the selected provider', () => {
      expect(get(selectedSessionProvider)).toBe('claude');
      pressMetaKey('t');
      expect(get(selectedSessionProvider)).toBe('codex');
      pressMetaKey('t');
      expect(get(selectedSessionProvider)).toBe('claude');
    });

    it('Cmd+T does not toggle while typing in an input', () => {
      const input = document.createElement('input');
      document.body.appendChild(input);
      input.focus();

      pressMetaKey('t');
      expect(get(selectedSessionProvider)).toBe('claude');

      input.remove();
    });
  });

  // ── Agents mode keys ──

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

    it('dev-only keys like n do not fire in agents mode', () => {
      let captured: any = null;
      const unsub = hotkeyAction.subscribe((v) => { captured = v; });
      pressKey('n');
      expect(captured).toBeNull();
      unsub();
    });

    it('global keys like j still work in agents mode', () => {
      pressKey('j');
      expect(get(focusTarget)).not.toBeNull();
    });

    it('r dispatches trigger-maintainer-check when focus is agent-panel', () => {
      focusTarget.set({ type: 'agent-panel', agentKind: 'maintainer', projectId: 'proj-1' });
      let captured: any = null;
      const unsub = hotkeyAction.subscribe((v) => { captured = v; });
      pressKey('r');
      expect(captured).toEqual({ type: 'trigger-maintainer-check' });
      unsub();
    });

    it('c dispatches clear-maintainer-reports when focus is agent-panel', () => {
      focusTarget.set({ type: 'agent-panel', agentKind: 'maintainer', projectId: 'proj-1' });
      let captured: any = null;
      const unsub = hotkeyAction.subscribe((v) => { captured = v; });
      pressKey('c');
      expect(captured).toEqual({ type: 'clear-maintainer-reports' });
      unsub();
    });
  });

  // ── Workspace mode (Space) ──

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

    it('Space then r switches to architecture mode and focuses the project', () => {
      focusTarget.set({ type: 'session', sessionId: 'sess-1', projectId: 'proj-1' });

      pressKey(' ');
      pressKey('r');

      expect(get(workspaceMode)).toBe('architecture');
      expect(get(workspaceModePickerVisible)).toBe(false);
      expect(get(focusTarget)).toEqual({ type: 'project', projectId: 'proj-1' });
    });

    it('Space then i switches to infrastructure mode', () => {
      focusTarget.set({ type: 'session', sessionId: 'sess-1', projectId: 'proj-1' });

      pressKey(' ');
      pressKey('i');

      expect(get(workspaceMode)).toBe('infrastructure');
      expect(get(workspaceModePickerVisible)).toBe(false);
      expect(get(focusTarget)).toEqual({ type: 'project', projectId: 'proj-1' });
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

  // ── Notes mode keys ──

  describe('notes mode keys', () => {
    beforeEach(() => {
      workspaceMode.set('notes');
      noteEntries.set(new Map([['proj-1', [{ filename: 'todo.md', modified_at: '2026-01-01' }]]]));
    });

    afterEach(() => {
      workspaceMode.set('development');
    });

    it('o on a focused note opens the note editor with entryKey', () => {
      focusTarget.set({ type: 'note', filename: 'todo.md', folder: 'proj-1' });
      pressKey('o');
      expect(get(activeNote)).toEqual({ folder: 'proj-1', filename: 'todo.md' });
      expect(get(focusTarget)).toEqual({ type: 'notes-editor', folder: 'proj-1', entryKey: 'o' });
    });

    it('i on a focused note opens the note editor with entryKey', () => {
      focusTarget.set({ type: 'note', filename: 'todo.md', folder: 'proj-1' });
      pressKey('i');
      expect(get(activeNote)).toEqual({ folder: 'proj-1', filename: 'todo.md' });
      expect(get(focusTarget)).toEqual({ type: 'notes-editor', folder: 'proj-1', entryKey: 'i' });
    });

    it('a on a focused note opens the note editor with entryKey', () => {
      focusTarget.set({ type: 'note', filename: 'todo.md', folder: 'proj-1' });
      pressKey('a');
      expect(get(activeNote)).toEqual({ folder: 'proj-1', filename: 'todo.md' });
      expect(get(focusTarget)).toEqual({ type: 'notes-editor', folder: 'proj-1', entryKey: 'a' });
    });
  });

  describe('architecture mode', () => {
    it('r dispatches generate-architecture for the focused project', () => {
      workspaceMode.set('architecture');
      focusTarget.set({ type: 'project', projectId: 'proj-1' });

      let captured: any = null;
      const unsub = hotkeyAction.subscribe((v) => { captured = v; });

      pressKey('r');

      expect(captured).toEqual({
        type: 'generate-architecture',
        projectId: 'proj-1',
        repoPath: '/tmp/test',
      });
      unsub();
    });
  });

  // ── Infrastructure mode navigation ──

  describe('infrastructure mode navigation', () => {
    beforeEach(() => {
      workspaceMode.set('infrastructure');
      projects.set([testProject, testProject2]);
    });

    afterEach(() => {
      workspaceMode.set('development');
    });

    it('j navigates between projects only (no sessions)', () => {
      focusTarget.set({ type: 'project', projectId: 'proj-1' });
      pressKey('j');
      // Should go directly to proj-2, skipping sess-1 and sess-2
      expect(get(focusTarget)).toEqual({ type: 'project', projectId: 'proj-2' });
    });

    it('k navigates between projects only (no sessions)', () => {
      focusTarget.set({ type: 'project', projectId: 'proj-2' });
      pressKey('k');
      // Should go directly to proj-1, skipping sessions
      expect(get(focusTarget)).toEqual({ type: 'project', projectId: 'proj-1' });
    });

    it('j wraps from last project to first project', () => {
      focusTarget.set({ type: 'project', projectId: 'proj-2' });
      pressKey('j');
      expect(get(focusTarget)).toEqual({ type: 'project', projectId: 'proj-1' });
    });

    it('k wraps from first project to last project', () => {
      focusTarget.set({ type: 'project', projectId: 'proj-1' });
      pressKey('k');
      expect(get(focusTarget)).toEqual({ type: 'project', projectId: 'proj-2' });
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

    it('hotkeys are ignored when a contenteditable element is focused', () => {
      const editor = document.createElement('div');
      editor.contentEditable = 'true';
      document.body.appendChild(editor);
      editor.focus();

      pressKey('g');
      expect(get(activeSessionId)).toBe('sess-1');
      expect(get(hotkeyAction)).toBeNull();

      editor.blur();
      editor.remove();
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
