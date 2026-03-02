import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { render, cleanup } from '@testing-library/svelte';
import { get } from 'svelte/store';
import { invoke } from '@tauri-apps/api/core';
import { projects, activeSessionId, hotkeyAction, leaderActive, jumpMode, sidebarVisible } from './stores';
import HotkeyManager from './HotkeyManager.svelte';

const testProject = {
  id: 'proj-1',
  name: 'test-project',
  repo_path: '/tmp/test',
  created_at: '2026-01-01',
  archived: false,
  sessions: [
    { id: 'sess-1', label: 'session-1', worktree_path: null, worktree_branch: null },
    { id: 'sess-2', label: 'session-2', worktree_path: null, worktree_branch: null },
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
    leaderActive.set(false);
    jumpMode.set(null);
    sidebarVisible.set(true);
    vi.clearAllMocks();
    render(HotkeyManager);
  });

  afterEach(() => {
    cleanup();
  });

  // ── Ambient mode (no terminal focused) ──

  describe('ambient mode', () => {
    it('1 switches to first session', () => {
      activeSessionId.set('sess-2');
      pressKey('1');
      expect(get(activeSessionId)).toBe('sess-1');
    });

    it('2 switches to second session', () => {
      pressKey('2');
      expect(get(activeSessionId)).toBe('sess-2');
    });

    it('c dispatches create-session action', () => {
      let captured: any = null;
      const unsub = hotkeyAction.subscribe((v) => { captured = v; });
      pressKey('c');
      expect(captured).toEqual({ type: 'create-session' });
      unsub();
    });

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

    it('x dispatches close-session action', () => {
      let captured: any = null;
      const unsub = hotkeyAction.subscribe((v) => { captured = v; });
      pressKey('x');
      expect(captured).toEqual({ type: 'close-session' });
      unsub();
    });

    it('? dispatches toggle-help action', () => {
      let captured: any = null;
      const unsub = hotkeyAction.subscribe((v) => { captured = v; });
      pressKey('?');
      expect(captured).toEqual({ type: 'toggle-help' });
      unsub();
    });

    it('h dispatches focus-sidebar action', () => {
      let captured: any = null;
      const unsub = hotkeyAction.subscribe((v) => { captured = v; });
      pressKey('h');
      expect(captured).toEqual({ type: 'focus-sidebar' });
      unsub();
    });

    it('l dispatches focus-terminal action', () => {
      let captured: any = null;
      const unsub = hotkeyAction.subscribe((v) => { captured = v; });
      pressKey('l');
      expect(captured).toEqual({ type: 'focus-terminal' });
      unsub();
    });

    it('d dispatches delete-project action', () => {
      let captured: any = null;
      const unsub = hotkeyAction.subscribe((v) => { captured = v; });
      pressKey('d');
      expect(captured).toEqual({ type: 'delete-project' });
      unsub();
    });

    it('a dispatches toggle-archive-view action', () => {
      let captured: any = null;
      const unsub = hotkeyAction.subscribe((v) => { captured = v; });
      pressKey('a');
      expect(captured).toEqual({ type: 'toggle-archive-view' });
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

    it('Escape in ambient mode toggles help bar', () => {
      pressKey('Escape');
      expect(get(leaderActive)).toBe(true);

      pressKey('Escape');
      expect(get(leaderActive)).toBe(false);
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
      pressKey('q');
      pressKey('w');
      pressKey('g');
      expect(get(activeSessionId)).toBe(initial);
      expect(get(hotkeyAction)).toBeNull();
    });

    it('pressing 9 with only 2 sessions does nothing', () => {
      pressKey('9');
      expect(get(activeSessionId)).toBe('sess-1');
    });
  });

  // ── Jump mode ──

  describe('jump mode', () => {
    it('j enters jump mode (project phase)', () => {
      pressKey('j');
      expect(get(jumpMode)).toEqual({ phase: 'project' });
    });

    it('j then z on single-session project enters session phase', () => {
      projects.set([{
        id: 'proj-1',
        name: 'solo-project',
        repo_path: '/tmp/solo',
        created_at: '2026-01-01',
        archived: false,
        sessions: [
          { id: 'sess-only', label: 'session-1', worktree_path: null, worktree_branch: null },
        ],
      }]);
      activeSessionId.set(null);

      pressKey('j');
      pressKey('z');
      // Now enters session phase instead of auto-selecting
      expect(get(jumpMode)).toEqual({ phase: 'session', projectId: 'proj-1' });
      expect(get(activeSessionId)).toBeNull();
    });

    it('j then z then z selects session from single-session project', () => {
      projects.set([{
        id: 'proj-1',
        name: 'solo-project',
        repo_path: '/tmp/solo',
        created_at: '2026-01-01',
        archived: false,
        sessions: [
          { id: 'sess-only', label: 'session-1', worktree_path: null, worktree_branch: null },
        ],
      }]);
      activeSessionId.set(null);

      pressKey('j');
      pressKey('z'); // project → session phase (2 labels: z=session, x=create)
      pressKey('z'); // select existing session
      expect(get(activeSessionId)).toBe('sess-only');
      expect(get(jumpMode)).toBeNull();
    });

    it('last jump label in session phase dispatches create-session with projectId', () => {
      projects.set([{
        id: 'proj-1',
        name: 'solo-project',
        repo_path: '/tmp/solo',
        created_at: '2026-01-01',
        archived: false,
        sessions: [
          { id: 'sess-only', label: 'session-1', worktree_path: null, worktree_branch: null },
        ],
      }]);

      let captured: any = null;
      const unsub = hotkeyAction.subscribe((v) => { captured = v; });

      pressKey('j');
      pressKey('z'); // session phase (2 labels: z=session, x=create)
      pressKey('x'); // create new session
      expect(captured).toEqual({ type: 'create-session', projectId: 'proj-1' });
      expect(get(jumpMode)).toBeNull();
      unsub();
    });

    it('j on zero-session project enters session phase with create option', () => {
      projects.set([{
        id: 'proj-empty',
        name: 'empty-project',
        repo_path: '/tmp/empty',
        created_at: '2026-01-01',
        archived: false,
        sessions: [],
      }]);

      pressKey('j');
      pressKey('z');
      expect(get(jumpMode)).toEqual({ phase: 'session', projectId: 'proj-empty' });
    });

    it('j then z on multi-session project enters session phase', () => {
      // testProject has 2 sessions
      pressKey('j');
      pressKey('z');
      expect(get(jumpMode)).toEqual({ phase: 'session', projectId: 'proj-1' });
      // Not selected yet — need to pick a session
      expect(get(activeSessionId)).toBe('sess-1'); // unchanged
    });

    it('j then z then z selects first session of first project', () => {
      pressKey('j');
      pressKey('z'); // select proj-1 (2 sessions) → session phase
      pressKey('z'); // select sess-1
      expect(get(activeSessionId)).toBe('sess-1');
      expect(get(jumpMode)).toBeNull();
    });

    it('j then z then x selects second session of first project', () => {
      pressKey('j');
      pressKey('z'); // select proj-1 → session phase
      pressKey('x'); // select sess-2
      expect(get(activeSessionId)).toBe('sess-2');
      expect(get(jumpMode)).toBeNull();
    });

    it('j then x on second project enters session phase', () => {
      projects.set([
        testProject,
        {
          id: 'proj-2',
          name: 'other-project',
          repo_path: '/tmp/other',
          created_at: '2026-01-01',
          archived: false,
          sessions: [
            { id: 'sess-3', label: 'session-1', worktree_path: null, worktree_branch: null },
          ],
        },
      ]);

      pressKey('j');
      pressKey('x'); // second project → session phase
      expect(get(jumpMode)).toEqual({ phase: 'session', projectId: 'proj-2' });
    });

    it('j then Escape cancels jump mode', () => {
      pressKey('j');
      expect(get(jumpMode)).toEqual({ phase: 'project' });

      pressKey('Escape');
      expect(get(jumpMode)).toBeNull();
    });

    it('j then unrecognized key cancels jump mode', () => {
      pressKey('j');
      expect(get(jumpMode)).toEqual({ phase: 'project' });

      pressKey('q');
      expect(get(jumpMode)).toBeNull();
    });

    it('Escape cancels session phase', () => {
      pressKey('j');
      pressKey('z'); // project phase → session phase
      expect(get(jumpMode)).toEqual({ phase: 'session', projectId: 'proj-1' });

      pressKey('Escape');
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
          { id: `sess-${i}`, label: 'session-1', worktree_path: null, worktree_branch: null },
        ],
      }));
      projects.set(manyProjects);

      pressKey('j');
      expect(get(jumpMode)).toEqual({ phase: 'project' });

      // First label should be "zz" (two-char since >6)
      pressKey('z'); // first char of "zz" — prefix match, still in jump mode
      expect(get(jumpMode)).toEqual({ phase: 'project' });

      pressKey('z'); // "zz" matches first project → enters session phase
      expect(get(jumpMode)).toEqual({ phase: 'session', projectId: 'proj-0' });
    });

    it('two-char label second key selects correct project', () => {
      const manyProjects = Array.from({ length: 7 }, (_, i) => ({
        id: `proj-${i}`,
        name: `project-${i}`,
        repo_path: `/tmp/p${i}`,
        created_at: '2026-01-01',
        archived: false,
        sessions: [
          { id: `sess-${i}`, label: 'session-1', worktree_path: null, worktree_branch: null },
        ],
      }));
      projects.set(manyProjects);

      pressKey('j');
      pressKey('z'); // prefix
      pressKey('x'); // "zx" = second project → session phase
      expect(get(jumpMode)).toEqual({ phase: 'session', projectId: 'proj-1' });
    });

    it('j with no projects does nothing', () => {
      projects.set([]);
      pressKey('j');
      expect(get(jumpMode)).toBeNull();
    });
  });

  // ── Explicit leader mode (terminal focused) ──

  describe('explicit leader mode (terminal focused)', () => {
    let xtermEl: HTMLElement;

    beforeEach(() => {
      xtermEl = simulateTerminalFocus();
    });

    afterEach(() => {
      removeTerminalFocus(xtermEl);
    });

    it('keys are ignored when terminal focused without Escape prefix', () => {
      const initial = get(activeSessionId);
      pressKey('j');
      pressKey('c');
      pressKey('f');
      expect(get(activeSessionId)).toBe(initial);
      expect(get(hotkeyAction)).toBeNull();
      expect(get(jumpMode)).toBeNull();
    });

    it('Escape enters explicit leader mode', () => {
      pressKey('Escape');
      expect(get(leaderActive)).toBe(true);
    });

    it('Escape then j enters jump mode and exits leader', () => {
      pressKey('Escape');
      expect(get(leaderActive)).toBe(true);

      removeTerminalFocus(xtermEl);
      xtermEl = document.createElement('div');

      pressKey('j');
      expect(get(jumpMode)).toEqual({ phase: 'project' });
      expect(get(leaderActive)).toBe(false);
    });

    it('Escape then c dispatches create-session and exits leader', () => {
      pressKey('Escape');

      removeTerminalFocus(xtermEl);
      xtermEl = document.createElement('div');

      let captured: any = null;
      const unsub = hotkeyAction.subscribe((v) => { captured = v; });
      pressKey('c');
      expect(captured).toEqual({ type: 'create-session' });
      expect(get(leaderActive)).toBe(false);
      unsub();
    });

    it('Escape then f dispatches open-fuzzy-finder and exits leader', () => {
      pressKey('Escape');

      removeTerminalFocus(xtermEl);
      xtermEl = document.createElement('div');

      let captured: any = null;
      const unsub = hotkeyAction.subscribe((v) => { captured = v; });
      pressKey('f');
      expect(captured).toEqual({ type: 'open-fuzzy-finder' });
      expect(get(leaderActive)).toBe(false);
      unsub();
    });

    it('Escape then Escape cancels leader mode', () => {
      pressKey('Escape');
      expect(get(leaderActive)).toBe(true);

      removeTerminalFocus(xtermEl);
      xtermEl = document.createElement('div');

      pressKey('Escape');
      expect(get(leaderActive)).toBe(false);
    });

    it('rapid triple-Escape forwards Escape to PTY', () => {
      const now = Date.now();
      vi.spyOn(Date, 'now').mockReturnValue(now);

      pressKey('Escape');
      expect(get(leaderActive)).toBe(true);

      vi.spyOn(Date, 'now').mockReturnValue(now + 50);
      pressKey('Escape');
      expect(get(leaderActive)).toBe(false);

      vi.spyOn(Date, 'now').mockReturnValue(now + 100);
      pressKey('Escape');

      expect(invoke).toHaveBeenCalledWith('write_to_pty', {
        sessionId: 'sess-1',
        data: '\x1b',
      });

      vi.restoreAllMocks();
    });

    it('Escape does not forward if too slow after leader exit', () => {
      const now = Date.now();
      vi.spyOn(Date, 'now').mockReturnValue(now);

      pressKey('Escape');

      vi.spyOn(Date, 'now').mockReturnValue(now + 50);
      pressKey('Escape');

      vi.spyOn(Date, 'now').mockReturnValue(now + 500);
      pressKey('Escape');

      expect(invoke).not.toHaveBeenCalledWith('write_to_pty', expect.anything());
      expect(get(leaderActive)).toBe(true);

      vi.restoreAllMocks();
    });
  });

  // ── Input field passthrough ──

  describe('input field passthrough', () => {
    it('hotkeys are ignored when an input element is focused', () => {
      const input = document.createElement('input');
      document.body.appendChild(input);
      input.focus();

      const initial = get(activeSessionId);
      pressKey('j');
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

      pressKey('j');
      expect(get(activeSessionId)).toBe('sess-1');
      expect(get(hotkeyAction)).toBeNull();

      textarea.blur();
      textarea.remove();
    });

    it('Escape still propagates when input is focused', () => {
      const input = document.createElement('input');
      document.body.appendChild(input);
      input.focus();

      pressKey('Escape');
      expect(get(leaderActive)).toBe(false);

      input.blur();
      input.remove();
    });
  });
});
