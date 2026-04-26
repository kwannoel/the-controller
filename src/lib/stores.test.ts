import { describe, it, expect, beforeEach } from 'vitest';
import { get } from 'svelte/store';
import {
  projects,
  maintainerStatuses,
  hotkeyAction,
  showKeyHints,
  appConfig,
  onboardingComplete,
  focusTarget,
  sidebarVisible,
  workspaceMode,
  workspaceModePickerVisible,
} from './stores';
import type { WorkspaceMode } from './stores';

describe('stores', () => {
  beforeEach(() => {
    projects.set([]);
    hotkeyAction.set(null);
    showKeyHints.set(false);
    appConfig.set(null);
    onboardingComplete.set(false);
  });

  it('projects starts empty', () => {
    expect(get(projects)).toEqual([]);
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

  describe('maintainerStatuses store', () => {
    it('starts as empty map', () => {
      const statuses = get(maintainerStatuses);
      expect(statuses).toBeInstanceOf(Map);
      expect(statuses.size).toBe(0);
    });
  });

  describe('workspace mode store', () => {
    it('defaults to chat', () => {
      expect(get(workspaceMode)).toBe('chat');
    });

    it('can switch to agents', () => {
      workspaceMode.set('agents');
      expect(get(workspaceMode)).toBe('agents');
      workspaceMode.set('chat'); // reset
    });

    it('can switch to kanban', () => {
      workspaceMode.set('kanban');
      expect(get(workspaceMode)).toBe('kanban');
      workspaceMode.set('chat'); // reset
    });

    it('picker starts hidden', () => {
      expect(get(workspaceModePickerVisible)).toBe(false);
    });
  });
});

describe("WorkspaceMode", () => {
  it("accepts the three remaining workspace modes", () => {
    const modes: WorkspaceMode[] = ["agents", "kanban", "chat"];
    expect(modes).toEqual(["agents", "kanban", "chat"]);
  });
});
