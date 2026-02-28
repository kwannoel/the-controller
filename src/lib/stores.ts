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
