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
  | { type: "create-session" }
  | { type: "close-session" }
  | { type: "next-project" }
  | { type: "prev-project" }
  | { type: "focus-sidebar" }
  | { type: "focus-terminal" }
  | { type: "toggle-help" }
  | null;

export const leaderActive = writable<boolean>(false);
export const hotkeyAction = writable<HotkeyAction>(null);
