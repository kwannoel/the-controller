export type Agent = "claude" | "codex";
export type SessionStatus = "starting" | "running" | "interrupted" | "ended" | "failed";
export type Channel = "inbox" | "outbox" | "system";

export interface DaemonSession {
  id: string;
  label: string;
  agent: Agent;
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
}

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
