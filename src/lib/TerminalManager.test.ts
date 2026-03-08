import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen } from '@testing-library/svelte';
import { projects, activeSessionId, focusTarget, type FocusTarget } from './stores';

// Mock Terminal.svelte to avoid xterm.js dependency
vi.mock('./Terminal.svelte', () => {
  // Return a minimal mock component
  const { mount } = require('svelte');
  return {
    default: function MockTerminal() {},
  };
});

import TerminalManager from './TerminalManager.svelte';

describe('TerminalManager', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    projects.set([]);
    activeSessionId.set(null);
    focusTarget.set(null);
  });

  it('shows empty state when no active session', () => {
    render(TerminalManager);
    expect(screen.getByText('No active session')).toBeInTheDocument();
  });

  it('shows keyboard hints in empty state', () => {
    render(TerminalManager);
    expect(screen.getByText('c')).toBeInTheDocument();
    expect(screen.getByText('n')).toBeInTheDocument();
  });

  it('hides empty state when a session is active', () => {
    projects.set([{
      id: 'proj-1',
      name: 'test',
      repo_path: '/tmp/test',
      created_at: '2026-01-01',
      archived: false,
      sessions: [
        { id: 'sess-1', label: 'session-1', worktree_path: null, worktree_branch: null, archived: false, kind: 'claude', github_issue: null, auto_worker_session: false },
      ],
    }]);
    activeSessionId.set('sess-1');

    render(TerminalManager);
    expect(screen.queryByText('No active session')).not.toBeInTheDocument();
  });

  it('sets focusTarget with projectId when terminal receives focus', async () => {
    projects.set([{
      id: 'proj-1',
      name: 'test',
      repo_path: '/tmp/test',
      created_at: '2026-01-01',
      archived: false,
      sessions: [
        { id: 'sess-1', label: 'session-1', worktree_path: null, worktree_branch: null, archived: false, kind: 'claude', github_issue: null, auto_worker_session: false },
      ],
    }]);
    activeSessionId.set('sess-1');

    const { container } = render(TerminalManager);
    const terminalManager = container.querySelector('.terminal-manager')!;
    terminalManager.dispatchEvent(new FocusEvent('focusin', { bubbles: true }));

    // Wait for Svelte reactivity
    await vi.dynamicImportSettled();

    let focus: FocusTarget = null;
    focusTarget.subscribe((v) => { focus = v; })();
    expect(focus).toEqual({ type: 'terminal', projectId: 'proj-1' });
  });
});
