import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { render, cleanup } from '@testing-library/svelte';
import { get } from 'svelte/store';
import { command } from '$lib/backend';
import { projects, activeSessionId, hotkeyAction, focusTarget, sidebarVisible, expandedProjects, workspaceMode, workspaceModePickerVisible, selectedSessionProvider, type Project, type SessionConfig } from './stores';
import { showToast } from './toast';
import HotkeyManager from './HotkeyManager.svelte';
import { daemonStore } from './daemon/store.svelte';
import type { DaemonSession } from './daemon/types';

vi.mock('./toast', () => ({
  showToast: vi.fn(),
}));

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
    staged_sessions: [],
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

    it('c on project dispatches create-session for the selected provider', () => {
      focusTarget.set({ type: 'project', projectId: 'proj-1' });
      let captured: any = null;
      const unsub = hotkeyAction.subscribe((v) => { captured = v; });
      pressKey('c');
      expect(captured).toEqual({ type: 'create-session', projectId: 'proj-1', kind: 'claude' });
      unsub();
    });

    it('c on session dispatches create-session for that project', () => {
      focusTarget.set({ type: 'session', sessionId: 'sess-1', projectId: 'proj-1' });
      let captured: any = null;
      const unsub = hotkeyAction.subscribe((v) => { captured = v; });
      pressKey('c');
      expect(captured).toEqual({ type: 'create-session', projectId: 'proj-1', kind: 'claude' });
      unsub();
    });

    it('c uses codex after provider toggle', () => {
      focusTarget.set({ type: 'project', projectId: 'proj-1' });
      selectedSessionProvider.set('codex');
      let captured: any = null;
      const unsub = hotkeyAction.subscribe((v) => { captured = v; });
      pressKey('c');
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
    it('c dispatches create-session when project focused', () => {
      focusTarget.set({ type: 'project', projectId: 'proj-1' });
      let captured: any = null;
      const unsub = hotkeyAction.subscribe((v) => { captured = v; });
      pressKey('c');
      expect(captured).toEqual({ type: 'create-session', projectId: 'proj-1', kind: 'claude' });
      unsub();
    });

    it('Cmd+T toggles the selected provider', () => {
      expect(get(selectedSessionProvider)).toBe('claude');
      pressMetaKey('t');
      expect(get(selectedSessionProvider)).toBe('codex');
      pressMetaKey('t');
      expect(get(selectedSessionProvider)).toBe('claude');
    });

    it('c with no projects shows "no projects" toast', () => {
      projects.set([]);
      focusTarget.set(null);
      pressKey('c');
      expect(showToast).toHaveBeenCalledWith(
        "No projects yet — press 'f' to find a directory or 'n' to create a new project",
        'error',
      );
      expect(get(hotkeyAction)).toBeNull();
    });

    it('c with projects but no focus shows "select a project" toast', () => {
      projects.set([testProject]);
      focusTarget.set(null);
      pressKey('c');
      expect(showToast).toHaveBeenCalledWith(
        "Select a project first (j/k to navigate, or 'f' to find a directory)",
        'error',
      );
      expect(get(hotkeyAction)).toBeNull();
    });

    it('held key repeat does not fire hotkeys', () => {
      focusTarget.set({ type: 'project', projectId: 'proj-1' });
      window.dispatchEvent(new KeyboardEvent('keydown', { key: 'c', repeat: true, bubbles: true }));
      expect(get(hotkeyAction)).toBeNull();
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

  // ── Chat mode navigation ──

  describe('chat mode navigation', () => {
    function makeDaemonSession(id: string, cwd: string, label = id): DaemonSession {
      return {
        id,
        label,
        agent: 'claude',
        cwd,
        args: [],
        status: 'running',
        native_session_id: null,
        pid: null,
        created_at: 0,
        updated_at: 0,
        ended_at: null,
        end_reason: null,
      };
    }

    beforeEach(() => {
      workspaceMode.set('chat');
      projects.set([testProject, testProject2]);
      expandedProjects.set(new Set(['proj-1', 'proj-2']));
      daemonStore.sessions.clear();
      daemonStore.activeSessionId = null;
      // proj-1 has two chat sessions, proj-2 has one
      daemonStore.sessions.set('c1', makeDaemonSession('c1', '/tmp/test', 'chat-1'));
      daemonStore.sessions.set('c2', makeDaemonSession('c2', '/tmp/test', 'chat-2'));
      daemonStore.sessions.set('c3', makeDaemonSession('c3', '/tmp/other', 'chat-3'));
    });

    afterEach(() => {
      workspaceMode.set('development');
      daemonStore.sessions.clear();
      daemonStore.activeSessionId = null;
    });

    it('j from project moves to first chat session of that project', () => {
      focusTarget.set({ type: 'project', projectId: 'proj-1' });
      pressKey('j');
      expect(get(focusTarget)).toEqual({ type: 'session', sessionId: 'c1', projectId: 'proj-1' });
      expect(daemonStore.activeSessionId).toBe('c1');
    });

    it('j from chat session moves to next chat session', () => {
      focusTarget.set({ type: 'session', sessionId: 'c1', projectId: 'proj-1' });
      pressKey('j');
      expect(get(focusTarget)).toEqual({ type: 'session', sessionId: 'c2', projectId: 'proj-1' });
      expect(daemonStore.activeSessionId).toBe('c2');
    });

    it('j crosses project boundary from last chat of proj-1 to proj-2 header', () => {
      focusTarget.set({ type: 'session', sessionId: 'c2', projectId: 'proj-1' });
      pressKey('j');
      expect(get(focusTarget)).toEqual({ type: 'project', projectId: 'proj-2' });
    });

    it('k from chat session moves to previous chat session or project header', () => {
      focusTarget.set({ type: 'session', sessionId: 'c2', projectId: 'proj-1' });
      pressKey('k');
      expect(get(focusTarget)).toEqual({ type: 'session', sessionId: 'c1', projectId: 'proj-1' });
      pressKey('k');
      expect(get(focusTarget)).toEqual({ type: 'project', projectId: 'proj-1' });
    });

    it('j does not walk into PTY sessions in chat mode', () => {
      // testProject has PTY sessions sess-1 and sess-2 but chat mode must ignore them.
      focusTarget.set({ type: 'project', projectId: 'proj-1' });
      pressKey('j'); // -> c1
      pressKey('j'); // -> c2
      pressKey('j'); // -> proj-2 (skipping any PTY sessions)
      expect(get(focusTarget)).toEqual({ type: 'project', projectId: 'proj-2' });
    });

    it('Enter on chat session sets daemonStore.activeSessionId', () => {
      focusTarget.set({ type: 'session', sessionId: 'c2', projectId: 'proj-1' });
      daemonStore.activeSessionId = null;
      let captured: any = null;
      const unsub = hotkeyAction.subscribe((v) => { captured = v; });
      pressKey('Enter');
      expect(daemonStore.activeSessionId).toBe('c2');
      // Should NOT dispatch focus-terminal (PTY behavior) in chat mode
      expect(captured).toBeNull();
      unsub();
    });

    it('Enter on project still toggles expand in chat mode', () => {
      expandedProjects.set(new Set());
      focusTarget.set({ type: 'project', projectId: 'proj-1' });
      pressKey('Enter');
      expect(get(expandedProjects).has('proj-1')).toBe(true);
    });

    it('j skips chat sessions of collapsed project', () => {
      expandedProjects.set(new Set(['proj-2'])); // proj-1 collapsed
      focusTarget.set({ type: 'project', projectId: 'proj-1' });
      pressKey('j');
      expect(get(focusTarget)).toEqual({ type: 'project', projectId: 'proj-2' });
    });
  });
});
