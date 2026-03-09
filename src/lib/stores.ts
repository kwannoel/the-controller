import { writable } from "svelte/store";

export interface GithubIssue {
  number: number;
  title: string;
  url: string;
  body?: string | null;
  labels: { name: string }[];
}

export interface AssignedIssue {
  number: number;
  title: string;
  url: string;
  assignees: { login: string }[];
  updatedAt: string;
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
  auto_worker_session: boolean;
}

export interface MaintainerConfig {
  enabled: boolean;
  interval_minutes: number;
  github_repo?: string | null;
}

export interface AutoWorkerConfig {
  enabled: boolean;
}

export interface IssueSummary {
  issue_number: number;
  title: string;
  url: string;
  labels: string[];
  action: "filed" | "updated";
}

export interface MaintainerRunLog {
  id: string;
  project_id: string;
  timestamp: string;
  issues_filed: IssueSummary[];
  issues_updated: IssueSummary[];
  issues_unchanged: number;
  summary: string;
}

export type MaintainerStatus = "idle" | "running" | "error";

export interface Project {
  id: string;
  name: string;
  repo_path: string;
  created_at: string;
  archived: boolean;
  sessions: SessionConfig[];
  maintainer: MaintainerConfig;
  auto_worker: AutoWorkerConfig;
}

export interface Config {
  projects_root: string;
}

export type WorkspaceMode = "development" | "agents";
export const workspaceMode = writable<WorkspaceMode>("development");
export const workspaceModePickerVisible = writable<boolean>(false);

export const projects = writable<Project[]>([]);
export const activeSessionId = writable<string | null>(null);
export type SessionStatus = "working" | "idle" | "exited";
export const sessionStatuses = writable<Map<string, SessionStatus>>(new Map());
export const appConfig = writable<Config | null>(null);
export const onboardingComplete = writable<boolean>(false);
export const maintainerStatuses = writable<Map<string, MaintainerStatus>>(new Map());
export const maintainerErrors = writable<Map<string, string>>(new Map());
export type AutoWorkerStatus = {
  status: "idle" | "working";
  message?: string;
  issue_number?: number;
  issue_title?: string;
};
export const autoWorkerStatuses = writable<Map<string, AutoWorkerStatus>>(new Map());

// Hotkey state
export type TriageCategory = "untriaged" | "triaged";

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
  | { type: "finish-branch"; sessionId: string; kind?: string }
  | { type: "screenshot-to-session"; preview?: boolean; cropped?: boolean }
  | { type: "toggle-maintainer-enabled" }
  | { type: "toggle-auto-worker-enabled" }
  | { type: "toggle-triage-panel"; category?: TriageCategory }
  | { type: "toggle-assigned-issues-panel" }
  | { type: "trigger-maintainer-check" }
  | { type: "clear-maintainer-reports" }
  | { type: "agent-panel-navigate"; direction: 1 | -1 }
  | { type: "agent-panel-select" }
  | { type: "agent-panel-escape" }
  | null;

export const hotkeyAction = writable<HotkeyAction>(null);
export const showKeyHints = writable<boolean>(false);
export const archiveView = writable<boolean>(false);
export const archivedProjects = writable<Project[]>([]);
export const sidebarVisible = writable<boolean>(true);

export const expandedProjects = writable<Set<string>>(new Set());

export function dispatchHotkeyAction(action: NonNullable<HotkeyAction>) {
  hotkeyAction.set(action);
  setTimeout(() => hotkeyAction.set(null), 0);
}

export function focusTerminalSoon(delayMs = 50) {
  setTimeout(() => dispatchHotkeyAction({ type: "focus-terminal" }), delayMs);
}

// Focus tracking — granular: which element is focused
export type AgentKind = "auto-worker" | "maintainer";

export type FocusTarget =
  | { type: "terminal"; projectId: string }
  | { type: "session"; sessionId: string; projectId: string }
  | { type: "project"; projectId: string }
  | { type: "agent"; agentKind: AgentKind; projectId: string }
  | { type: "agent-panel"; agentKind: AgentKind; projectId: string }
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
