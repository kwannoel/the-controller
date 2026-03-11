import { describe, it, expect, beforeEach } from 'vitest';
import { get } from 'svelte/store';
import {
  projects,
  activeSessionId,
  sessionStatuses,
  maintainerStatuses,
  hotkeyAction,
  showKeyHints,
  appConfig,
  onboardingComplete,
  focusTarget,
  sidebarVisible,
  controllerChatVisible,
  workspaceMode,
  workspaceModePickerVisible,
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

  it('focusTarget defaults to null', () => {
    expect(get(focusTarget)).toBeNull();
  });

  it('sidebarVisible defaults to true', () => {
    expect(get(sidebarVisible)).toBe(true);
  });

  it('controllerChatVisible defaults to false', () => {
    expect(get(controllerChatVisible)).toBe(false);
  });

  describe('maintainerStatuses store', () => {
    it('starts as empty map', () => {
      const statuses = get(maintainerStatuses);
      expect(statuses).toBeInstanceOf(Map);
      expect(statuses.size).toBe(0);
    });
  });

  describe('workspace mode store', () => {
    it('defaults to development', () => {
      expect(get(workspaceMode)).toBe('development');
    });

    it('can switch to agents', () => {
      workspaceMode.set('agents');
      expect(get(workspaceMode)).toBe('agents');
      workspaceMode.set('development'); // reset
    });

    it('picker starts hidden', () => {
      expect(get(workspaceModePickerVisible)).toBe(false);
    });
  });
});
