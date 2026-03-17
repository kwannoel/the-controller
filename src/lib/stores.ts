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

export interface NoteEntry {
  filename: string;
  modified_at: string;
}

export interface ArchitectureRelationship {
  component_id: string;
  summary: string;
}

export interface ArchitectureComponent {
  id: string;
  name: string;
  summary: string;
  contains: string[];
  incoming_relationships: ArchitectureRelationship[];
  outgoing_relationships: ArchitectureRelationship[];
  evidence_paths: string[];
  evidence_snippets: string[];
}

export interface ArchitectureResult {
  title: string;
  mermaid: string;
  components: ArchitectureComponent[];
}

export interface ArchitectureViewState {
  result: ArchitectureResult | null;
  selectedComponentId: string | null;
  isGenerating: boolean;
  error: string | null;
  logs: string[];
}

export function createArchitectureViewState(
  result: ArchitectureResult | null = null,
): ArchitectureViewState {
  return {
    result,
    selectedComponentId: result?.components[0]?.id ?? null,
    isGenerating: false,
    error: null,
    logs: [],
  };
}

export type WorkspaceMode =
  | "development"
  | "agents"
  | "notes"
  | "architecture"
  | "infrastructure"
  | "voice";
export const workspaceMode = writable<WorkspaceMode>("development");
export const workspaceModePickerVisible = writable<boolean>(false);
export type SessionProvider = "claude" | "codex";
export const selectedSessionProvider = writable<SessionProvider>("claude");

export const activeNote = writable<{
  folder: string;
  filename: string;
} | null>(null);
export const noteEntries = writable<Map<string, NoteEntry[]>>(new Map());
export const noteFolders = writable<string[]>([]);
export type NoteViewMode = "edit" | "preview" | "split";
export const noteViewMode = writable<NoteViewMode>("edit");
export const architectureViews = writable<Map<string, ArchitectureViewState>>(
  new Map(),
);
export const projects = writable<Project[]>([]);
export const activeSessionId = writable<string | null>(null);
export type SessionStatus = "working" | "idle" | "exited";
export const sessionStatuses = writable<Map<string, SessionStatus>>(new Map());
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
  | { type: "open-new-project" }
  | { type: "create-session"; projectId?: string; kind?: string }
  | { type: "delete-session"; sessionId?: string; projectId?: string }
  | { type: "focus-terminal" }
  | { type: "toggle-help" }
  | { type: "delete-project"; projectId?: string }
  | { type: "open-issues-modal"; projectId: string; repoPath: string }
  | {
      type: "assign-issue-to-session";
      projectId: string;
      repoPath: string;
      issue: GithubIssue;
    }
  | { type: "merge-session"; sessionId: string; projectId: string }
  | { type: "finish-branch"; sessionId: string; kind?: "claude" | "codex" }
  | { type: "e2e-eval"; sessionId: string; kind?: "claude" | "codex" }
  | { type: "screenshot-to-session"; direct?: boolean; cropped?: boolean }
  | { type: "toggle-maintainer-enabled" }
  | { type: "toggle-auto-worker-enabled" }
  | { type: "trigger-maintainer-check" }
  | { type: "clear-maintainer-reports" }
  | { type: "agent-panel-navigate"; direction: 1 | -1 }
  | { type: "agent-panel-select" }
  | { type: "agent-panel-escape" }
  | { type: "create-note" }
  | { type: "create-folder" }
  | { type: "delete-note"; folder: string; filename: string }
  | { type: "rename-note"; folder: string; filename: string }
  | { type: "duplicate-note"; folder: string; filename: string }
  | { type: "rename-folder"; folder: string }
  | { type: "delete-folder"; folder: string }
  | { type: "toggle-note-preview" }
  | { type: "save-session-prompt"; sessionId: string; projectId: string }
  | { type: "pick-prompt-for-session"; projectId: string }
  | { type: "generate-architecture"; projectId: string; repoPath: string }
  | { type: "stage-session"; sessionId: string; projectId: string }
  | { type: "unstage-session"; projectId: string; sessionId: string }
  | { type: "toggle-maintainer-view" }
  | { type: "open-issue-in-browser" }
  | { type: "deploy-project"; projectId: string; repoPath: string }
  | { type: "rollback-deploy"; projectId: string }
  | { type: "voice-toggle-panel"; panel: "debug" | "transcript" }
  | null;

export const hotkeyAction = writable<HotkeyAction>(null);
export const showKeyHints = writable<boolean>(false);
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
  | { type: "folder"; folder: string }
  | { type: "note"; filename: string; folder: string }
  | { type: "notes-editor"; folder: string; entryKey?: string }
  | null;
export const focusTarget = writable<FocusTarget>(null);
