export type Agent = "claude" | "codex";
export type SessionStatus = "starting" | "running" | "interrupted" | "ended" | "failed";
export type Channel = "inbox" | "outbox" | "system";
export type RouteTokenKind = "reusable" | "shadow";

export interface DaemonSession {
  id: string;
  label: string;
  agent: Agent;
  agent_profile_id?: string | null;
  session_kind?: "raw" | "reusable" | "shadow";
  owner_chat_id?: string | null;
  profile_version_id?: string | null;
  launch_context_snapshot?: string | null;
  cwd: string;
  args: string[];
  status: SessionStatus;
  native_session_id: string | null;
  pid: number | null;
  created_at: number;
  updated_at: number;
  ended_at: number | null;
  end_reason: string | null;
}

export interface EventRecord {
  session_id: string;
  seq: number;
  channel: Channel;
  kind: string;
  payload: unknown;
  created_at: number;
  applied_at: number | null;
  chat_id?: string | null;
  chat_seq?: number | null;
  turn_id?: string | null;
}

export interface AgentProfile {
  id: string;
  handle: string;
  name: string;
  description: string;
  runtime: Agent;
  skills: string[];
  prompt: string;
  archived_at: number | null;
  avatar_asset_path: string | null;
  avatar_status: string;
  avatar_error: string | null;
  active_version_id: string | null;
  created_at: number;
  updated_at: number;
}

export interface AgentProfileVersion {
  id: string;
  profile_id: string;
  runtime: Agent;
  model: string | null;
  prompt: string;
  skills: string[];
  default_workspace_behavior: string;
  outbox_instructions: string;
  validation_result: unknown;
  created_at: number;
}

export interface SavedAgentProfile {
  profile: AgentProfile;
  version: AgentProfileVersion;
}

export interface Chat {
  id: string;
  project_id: string;
  title: string;
  created_at: number;
  updated_at: number;
  deleted_at: number | null;
}

export interface RouteToken {
  kind: RouteTokenKind;
  handle: string;
  start: number;
  end: number;
}

export interface ChatMessage {
  id: string;
  chat_id: string;
  idempotency_id: string | null;
  body: string;
  token_spans: unknown;
  created_at: number;
}

export interface ChatAgentLink {
  id: string;
  chat_id: string;
  session_id: string;
  profile_id: string;
  profile_version_id: string;
  route_type: string;
  focused: boolean;
  token_source: string | null;
  created_at: number;
}

export interface ChatWorkspaceLink {
  id: string;
  chat_id: string;
  project_id: string;
  workspace_id: string;
  path: string;
  label: string;
  branch: string | null;
  focused: boolean;
  created_at: number;
  updated_at: number;
}

export interface SendChatMessageRequest {
  idempotency_id?: string;
  body: string;
  tokens?: RouteToken[];
}

export interface AgentTurn {
  id: string;
  session_id: string;
  chat_id: string;
  chat_message_id: string;
  inbox_seq: number;
  status: string;
  received_at: number;
  activity_started_at: number | null;
  ended_at: number | null;
}

export interface TurnMetrics {
  turn_id: string;
  input_tokens: number | null;
  output_tokens: number | null;
  cache_read_tokens: number | null;
  cache_write_tokens: number | null;
  tool_call_count: number;
  outbox_write_count: number;
  error_count: number;
  updated_at: number;
}

export interface AgentTurnTrace {
  turn: AgentTurn;
  metrics: TurnMetrics | null;
  events: EventRecord[];
}

export interface ChatMetrics {
  chat_id: string;
  turn_count: number;
  input_tokens: number;
  output_tokens: number;
  cache_read_tokens: number;
  cache_write_tokens: number;
  tool_call_count: number;
  outbox_write_count: number;
  error_count: number;
  total_elapsed_ms?: number | null;
  average_turn_ms?: number | null;
  slowest_turn_ms?: number | null;
  updated_at: number;
  agents?: ChatAgentMetrics[];
  turns?: ChatTurnMetrics[];
  workspace_links?: ChatWorkspaceLink[];
}

export interface ChatAgentMetrics {
  session_id: string;
  profile_id: string;
  profile_version_id: string;
  route_type: string;
  focused: boolean;
  token_source: string;
  status: SessionStatus | null;
  turn_count: number;
  input_tokens: number | null;
  output_tokens: number | null;
  cache_read_tokens: number | null;
  cache_write_tokens: number | null;
  total_tokens: number | null;
  tool_call_count: number | null;
  outbox_write_count: number | null;
  error_count: number | null;
  total_elapsed_ms: number | null;
  average_turn_ms: number | null;
  slowest_turn_ms: number | null;
  current_turn_id: string | null;
  updated_at: number;
}

export interface ChatTurnMetrics {
  turn_id: string;
  session_id: string;
  chat_id: string;
  chat_message_id: string;
  status: string;
  received_at: number;
  activity_started_at: number | null;
  ended_at: number | null;
  activity_latency_ms: number | null;
  duration_ms: number | null;
  input_tokens: number | null;
  output_tokens: number | null;
  cache_read_tokens: number | null;
  cache_write_tokens: number | null;
  total_tokens: number | null;
  tool_call_count: number | null;
  outbox_write_count: number | null;
  error_count: number | null;
  updated_at: number;
}

export type ChatTranscriptEntry =
  | { type: "user_message"; message: ChatMessage }
  | { type: "outbox_event"; event: EventRecord };

export type OutboxEvent =
  | { kind: "agent_text"; payload: { message_id: string; block_id: string; text: string; role?: string } }
  | { kind: "agent_text_delta"; payload: { message_id: string; block_id: string; delta: string; role?: string } }
  | { kind: "agent_thinking"; payload: { message_id: string; block_id: string; text: string } }
  | { kind: "tool_call"; payload: { call_id: string; tool: string; input: unknown } }
  | { kind: "tool_result"; payload: { call_id: string; output: unknown; is_error: boolean } }
  | { kind: "token_usage"; payload: { input: number; output: number; cache_read: number; cache_write: number } }
  | { kind: "error"; payload: { code: string; message: string; detail?: unknown } };

export type InboxEvent =
  | { kind: "user_text"; payload: { text: string } }
  | { kind: "interrupt"; payload: {} }
  | { kind: "tool_approval"; payload: { call_id: string; approved: boolean; reason?: string } };

export type StatusState = "starting" | "idle" | "working" | "waiting_for_tool_approval" | "failed";

export type SystemEvent =
  | { kind: "session_started"; payload: { agent: Agent; cwd: string; args: string[] } }
  | { kind: "session_ended"; payload: { end_reason: string; exit_code?: number; signal?: string } }
  | { kind: "session_interrupted"; payload: { reason: string } }
  | { kind: "session_resumed"; payload: { native_session_id: string } }
  | { kind: "agent_crashed"; payload: { exit_code?: number; signal?: string; last_stderr_tail?: string } }
  | { kind: "status_changed"; payload: { state: StatusState; idle_ms?: number } };
