import { writable } from "svelte/store";

export interface GithubIssue {
  number: number;
  title: string;
  url: string;
  body?: string | null;
  labels: { name: string }[];
  assignees?: { login: string; avatarUrl?: string }[];
  milestone?: { title: string } | null;
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

export interface SavedPrompt {
  id: string;
  name: string;
  text: string;
  created_at: string;
  source_session_label: string;
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
  issues_skipped: number;
  summary: string;
}

export interface MaintainerIssue {
  number: number;
  title: string;
  state: string;
  url: string;
  labels: { name: string }[];
  createdAt: string;
  closedAt: string | null;
}

export interface MaintainerIssueDetail {
  number: number;
  title: string;
  state: string;
  body: string;
  url: string;
  labels: { name: string }[];
  createdAt: string;
  closedAt: string | null;
}

export type MaintainerStatus = "idle" | "running" | "error";

export interface StagedSession {
  session_id: string;
  pid: number;
  port: number;
}

export interface Project {
  id: string;
  name: string;
  repo_path: string;
  created_at: string;
  archived: boolean;
  sessions: SessionConfig[];
  maintainer: MaintainerConfig;
  auto_worker: AutoWorkerConfig;
  prompts: SavedPrompt[];
  staged_sessions: StagedSession[];
}

export interface CorruptProjectEntry {
  project_dir: string;
  project_file: string;
  error: string;
}

export interface ProjectInventory {
  projects: Project[];
  corrupt_entries: CorruptProjectEntry[];
}

export interface Config {
  projects_root: string;
}

export type WorkspaceMode =
  | "agents"
  | "kanban"
  | "chat"
  | "agent-create"
  | "agent-observe";
export const workspaceMode = writable<WorkspaceMode>("chat");
export const workspaceModePickerVisible = writable<boolean>(false);

export const projects = writable<Project[]>([]);
export const appConfig = writable<Config | null>(null);
export const onboardingComplete = writable<boolean>(false);
export const maintainerStatuses = writable<Map<string, MaintainerStatus>>(
  new Map(),
);
export const maintainerErrors = writable<Map<string, string>>(new Map());
export type AutoWorkerStatus = {
  status: "idle" | "working";
  message?: string;
  issue_number?: number;
  issue_title?: string;
};
export const autoWorkerStatuses = writable<Map<string, AutoWorkerStatus>>(
  new Map(),
);

export interface WorkerReport {
  issue_number: number;
  title: string;
  comment_body: string;
  updated_at: string;
}

export interface AutoWorkerQueueIssue {
  number: number;
  title: string;
  url: string;
  body?: string | null;
  labels: string[];
  is_active: boolean;
}

// Hotkey state
export type HotkeyAction =
  | { type: "open-fuzzy-finder" }
  | { type: "toggle-help" }
  | { type: "toggle-maintainer-enabled" }
  | { type: "toggle-auto-worker-enabled" }
  | { type: "trigger-maintainer-check" }
  | { type: "clear-maintainer-reports" }
  | { type: "agent-panel-navigate"; direction: 1 | -1 }
  | { type: "agent-panel-select" }
  | { type: "agent-panel-escape" }
  | { type: "toggle-maintainer-view" }
  | { type: "open-issue-in-browser" }
  | { type: "focus-chat-input" }
  | null;

export const hotkeyAction = writable<HotkeyAction>(null);
export const showKeyHints = writable<boolean>(false);
export const sidebarVisible = writable<boolean>(true);

export const expandedProjects = writable<Set<string>>(new Set());

export function dispatchHotkeyAction(action: NonNullable<HotkeyAction>) {
  hotkeyAction.set(action);
  setTimeout(() => hotkeyAction.set(null), 0);
}

// Focus tracking — granular: which element is focused
export type AgentKind = "auto-worker" | "maintainer";

export type FocusTarget =
  | { type: "session"; sessionId: string; projectId: string }
  | { type: "project"; projectId: string }
  | { type: "agent"; agentKind: AgentKind; projectId: string }
  | { type: "agent-panel"; agentKind: AgentKind; projectId: string }
  | null;
export const focusTarget = writable<FocusTarget>(null);
