import { writable } from "svelte/store";

export interface GithubIssue {
  number: number;
  title: string;
  url: string;
  labels: { name: string }[];
}

export interface DirEntry {
  name: string;
  path: string;
}

export interface SessionConfig {
  id: string;
  label: string;
  worktree_path: string | null;
  worktree_branch: string | null;
  archived: boolean;
  kind: string;
  github_issue: GithubIssue | null;
  initial_prompt: string | null;
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
export type SessionStatus = "working" | "idle" | "exited";
export const sessionStatuses = writable<Map<string, SessionStatus>>(new Map());
export const appConfig = writable<Config | null>(null);
export const onboardingComplete = writable<boolean>(false);

// Hotkey state
export type HotkeyAction =
  | { type: "open-fuzzy-finder" }
  | { type: "open-new-project" }
  | { type: "create-session"; projectId?: string; kind?: string }
  | { type: "delete-session"; sessionId?: string; projectId?: string }
  | { type: "focus-terminal" }
  | { type: "toggle-help" }
  | { type: "delete-project"; projectId?: string }
  | { type: "archive-project"; projectId?: string }
  | { type: "archive-session"; sessionId: string; projectId: string }
  | { type: "unarchive-session"; sessionId: string; projectId: string }
  | { type: "unarchive-project"; projectId: string }
  | { type: "toggle-archive-view" }
  | { type: "create-issue"; projectId: string; repoPath: string }
  | { type: "pick-issue-for-session"; projectId: string; repoPath: string; kind?: string; background?: boolean }
  | { type: "merge-session"; sessionId: string; projectId: string }
  | { type: "screenshot-to-session" }
  | null;

export const hotkeyAction = writable<HotkeyAction>(null);
export const showKeyHints = writable<boolean>(false);
export const archiveView = writable<boolean>(false);
export const archivedProjects = writable<Project[]>([]);
export const sidebarVisible = writable<boolean>(true);
export const taskPanelVisible = writable<boolean>(false);
export const expandedProjects = writable<Set<string>>(new Set());

// Focus tracking — granular: which element is focused
export type FocusTarget =
  | { type: "terminal"; projectId: string }
  | { type: "session"; sessionId: string; projectId: string }
  | { type: "project"; projectId: string }
  | null;
export const focusTarget = writable<FocusTarget>(null);

// Jump navigation
export type JumpPhase =
  | { phase: "project" }
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
