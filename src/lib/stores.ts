import { writable } from "svelte/store";

export interface SessionConfig {
  id: string;
  label: string;
  worktree_path: string | null;
  worktree_branch: string | null;
}

export interface Project {
  id: string;
  name: string;
  repo_path: string;
  created_at: string;
  archived: boolean;
  sessions: SessionConfig[];
}

export interface Config {
  projects_root: string;
}

export const projects = writable<Project[]>([]);
export const activeSessionId = writable<string | null>(null);
export const sessionStatuses = writable<Map<string, "running" | "idle">>(new Map());
export const appConfig = writable<Config | null>(null);
export const onboardingComplete = writable<boolean>(false);

// Hotkey state
export type HotkeyAction =
  | { type: "open-fuzzy-finder" }
  | { type: "open-new-project" }
  | { type: "create-session"; projectId?: string }
  | { type: "close-session" }
  | { type: "focus-sidebar" }
  | { type: "focus-terminal" }
  | { type: "toggle-help" }
  | { type: "delete-project" }
  | { type: "toggle-archive-view" }
  | null;

export const leaderActive = writable<boolean>(false);
export const hotkeyAction = writable<HotkeyAction>(null);
export const showKeyHints = writable<boolean>(false);
export const archiveView = writable<boolean>(false);
export const sidebarVisible = writable<boolean>(true);

// Focus tracking
export type FocusedPanel = "sidebar" | "terminal" | null;
export const focusedPanel = writable<FocusedPanel>(null);

// Jump navigation
export type JumpPhase =
  | { phase: "project" }
  | { phase: "session"; projectId: string }
  | null;

export const jumpMode = writable<JumpPhase>(null);

export const JUMP_KEYS = ["z", "x", "c", "b", "n", "m"];

export function generateJumpLabels(count: number): string[] {
  if (count <= 0) return [];
  if (count <= JUMP_KEYS.length) {
    return JUMP_KEYS.slice(0, count);
  }
  const labels: string[] = [];
  for (const a of JUMP_KEYS) {
    for (const b of JUMP_KEYS) {
      labels.push(a + b);
      if (labels.length >= count) return labels;
    }
  }
  return labels;
}
