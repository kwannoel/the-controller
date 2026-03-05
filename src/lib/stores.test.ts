import { describe, it, expect, beforeEach } from 'vitest';
import { get } from 'svelte/store';
import {
  projects,
  activeSessionId,
  sessionStatuses,
  hotkeyAction,
  showKeyHints,
  appConfig,
  onboardingComplete,
  jumpMode,
  archiveView,
  focusTarget,
  sidebarVisible,
  generateJumpLabels,
  JUMP_KEYS,
} from './stores';

describe('stores', () => {
  beforeEach(() => {
    projects.set([]);
    activeSessionId.set(null);
    sessionStatuses.set(new Map());
    hotkeyAction.set(null);
    showKeyHints.set(false);
    appConfig.set(null);
    onboardingComplete.set(false);
  });

  it('projects starts empty', () => {
    expect(get(projects)).toEqual([]);
  });

  it('activeSessionId starts null', () => {
    expect(get(activeSessionId)).toBeNull();
  });

  it('sessionStatuses can track working/idle/exited', () => {
    sessionStatuses.update((m) => {
      const next = new Map(m);
      next.set('sess-1', 'working');
      next.set('sess-2', 'idle');
      next.set('sess-3', 'exited');
      return next;
    });
    const statuses = get(sessionStatuses);
    expect(statuses.get('sess-1')).toBe('working');
    expect(statuses.get('sess-2')).toBe('idle');
    expect(statuses.get('sess-3')).toBe('exited');
    expect(statuses.size).toBe(3);
  });

  it('hotkeyAction dispatch and reset', () => {
    hotkeyAction.set({ type: 'open-fuzzy-finder' });
    expect(get(hotkeyAction)).toEqual({ type: 'open-fuzzy-finder' });

    hotkeyAction.set(null);
    expect(get(hotkeyAction)).toBeNull();
  });

  it('showKeyHints toggles', () => {
    expect(get(showKeyHints)).toBe(false);
    showKeyHints.update((v) => !v);
    expect(get(showKeyHints)).toBe(true);
    showKeyHints.update((v) => !v);
    expect(get(showKeyHints)).toBe(false);
  });

  it('appConfig defaults to null', () => {
    expect(get(appConfig)).toBeNull();
  });

  it('onboardingComplete defaults to false', () => {
    expect(get(onboardingComplete)).toBe(false);
  });

  it('jumpMode defaults to null', () => {
    expect(get(jumpMode)).toBeNull();
  });

  it('archiveView defaults to false', () => {
    expect(get(archiveView)).toBe(false);
  });

  it('focusTarget defaults to null', () => {
    expect(get(focusTarget)).toBeNull();
  });

  it('sidebarVisible defaults to true', () => {
    expect(get(sidebarVisible)).toBe(true);
  });

  describe('generateJumpLabels', () => {
    it('returns empty array for 0 items', () => {
      expect(generateJumpLabels(0)).toEqual([]);
    });

    it('returns single-char labels for ≤6 items', () => {
      expect(generateJumpLabels(3)).toEqual(['z', 'x', 'c']);
      expect(generateJumpLabels(6)).toEqual(['z', 'x', 'c', 'b', 'n', 'm']);
    });

    it('returns two-char labels for >6 items', () => {
      const labels = generateJumpLabels(7);
      expect(labels.length).toBe(7);
      expect(labels[0]).toBe('zz');
      expect(labels[1]).toBe('zx');
      expect(labels[6]).toBe('xz');
    });

    it('generates enough labels for large counts', () => {
      const labels = generateJumpLabels(36);
      expect(labels.length).toBe(36);
      // All unique
      expect(new Set(labels).size).toBe(36);
      // All two chars
      expect(labels.every(l => l.length === 2)).toBe(true);
    });
  });
});
