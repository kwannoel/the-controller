import type {
  Agent,
  AgentProfile,
  AgentTurn,
  AgentTurnTrace,
  Channel,
  Chat,
  ChatMetrics,
  ChatMessage,
  ChatTranscriptEntry,
  ChatWorkspaceLink,
  DaemonSession,
  EventRecord,
  SavedAgentProfile,
  SendChatMessageRequest,
} from "./types";

export class DaemonHttpError extends Error {
  name = "DaemonHttpError";
  constructor(public status: number, public body: string, message: string) {
    super(message);
  }
}

export interface CreateSessionRequest {
  agent: Agent;
  cwd: string;
  args?: string[];
  initial_prompt?: string;
}

export interface SendMessageRequest {
  kind: "user_text" | "interrupt" | "tool_approval";
  text?: string;
  call_id?: string;
  approved?: boolean;
  reason?: string;
}

export interface SaveProfileRequest {
  id?: string;
  handle?: string;
  name?: string;
  description?: string | null;
  runtime?: Agent;
  model?: string | null;
  skills?: string[];
  prompt?: string;
  default_workspace_behavior?: string | null;
  outbox_instructions?: string | null;
}

export interface TestProfileInChatRequest {
  chat_id?: string;
  body?: string;
}

export interface CreateChatRequest {
  project_id: string;
  title: string;
}

export interface AddWorkspaceLinkRequest {
  project_id: string;
  workspace_id: string;
  path: string;
  label: string;
  branch?: string | null;
  focused: boolean;
}

export class DaemonClient {
  constructor(private baseUrl = "/api/daemon") {}

  private async call<T>(path: string, init?: RequestInit): Promise<T> {
    const res = await fetch(`${this.baseUrl}${path}`, {
      ...init,
      headers: {
        "Content-Type": "application/json",
        ...(init?.headers ?? {}),
      },
    });
    if (!res.ok) {
      const body = await res.text().catch(() => "");
      throw new DaemonHttpError(res.status, body, `daemon ${res.status} on ${path}: ${body}`);
    }
    if (res.status === 204) return undefined as unknown as T;
    return res.json() as Promise<T>;
  }

  listSessions(): Promise<DaemonSession[]> {
    return this.call("/sessions");
  }
  getSession(id: string): Promise<DaemonSession> {
    return this.call(`/sessions/${id}`);
  }
  createSession(req: CreateSessionRequest): Promise<{ id: string; label: string }> {
    return this.call("/sessions", { method: "POST", body: JSON.stringify(req) });
  }
  deleteSession(id: string): Promise<void> {
    return this.call(`/sessions/${id}`, { method: "DELETE" });
  }
  sendMessage(id: string, req: SendMessageRequest): Promise<{ seq: number }> {
    const body =
      req.kind === "user_text" ? { kind: "user_text", text: req.text } :
      req.kind === "interrupt" ? { kind: "interrupt" } :
      { kind: "tool_approval", call_id: req.call_id, approved: req.approved, reason: req.reason };
    return this.call(`/sessions/${id}/messages`, { method: "POST", body: JSON.stringify(body) });
  }
  readEvents(id: string, since = 0, channels?: Channel[]): Promise<EventRecord[]> {
    const q = new URLSearchParams();
    q.set("since", String(since));
    if (channels && channels.length) q.set("channels", channels.join(","));
    return this.call(`/sessions/${id}/messages?${q.toString()}`);
  }

  wsUrl(id: string, since = 0, channels?: Channel[]): string {
    const base = this.baseUrl.replace(/^http/, "ws");
    const q = new URLSearchParams();
    q.set("since", String(since));
    if (channels && channels.length) q.set("channels", channels.join(","));
    return `${base}/sessions/${id}/stream?${q.toString()}`;
  }

  listProfiles(): Promise<AgentProfile[]> {
    return this.call("/profiles");
  }
  saveProfile(req: SaveProfileRequest): Promise<SavedAgentProfile> {
    if (req.id) {
      return this.call(`/profiles/${req.id}`, { method: "PATCH", body: JSON.stringify(req) });
    }
    return this.call("/profiles", { method: "POST", body: JSON.stringify(req) });
  }
  archiveProfile(id: string): Promise<AgentProfile> {
    return this.call(`/profiles/${id}/archive`, { method: "POST" });
  }
  restoreProfile(id: string): Promise<AgentProfile> {
    return this.call(`/profiles/${id}/restore`, { method: "POST" });
  }
  testProfileInChat(id: string, req: TestProfileInChatRequest = {}): Promise<unknown> {
    return this.call(`/profiles/${id}/test-chat`, { method: "POST", body: JSON.stringify(req) });
  }

  listChats(): Promise<Chat[]> {
    return this.call("/chats");
  }
  createChat(req: CreateChatRequest): Promise<Chat> {
    return this.call("/chats", { method: "POST", body: JSON.stringify(req) });
  }
  deleteChat(id: string): Promise<void> {
    return this.call(`/chats/${id}`, { method: "DELETE" });
  }
  sendChatMessage(id: string, req: SendChatMessageRequest): Promise<{ message: ChatMessage; turns: AgentTurn[] }> {
    return this.call(`/chats/${id}/messages`, { method: "POST", body: JSON.stringify(req) });
  }
  readChatTranscript(id: string): Promise<ChatTranscriptEntry[]> {
    return this.call(`/chats/${id}/transcript`);
  }
  chatStreamUrl(id: string): string {
    const base = this.baseUrl.replace(/^http/, "ws");
    return `${base}/chats/${id}/stream`;
  }
  addWorkspaceLink(id: string, req: AddWorkspaceLinkRequest): Promise<ChatWorkspaceLink> {
    return this.call(`/chats/${id}/workspace-links`, { method: "POST", body: JSON.stringify(req) });
  }
  focusWorkspaceLink(id: string, linkId: string): Promise<ChatWorkspaceLink> {
    return this.call(`/chats/${id}/workspace-links/${linkId}/focus`, { method: "PATCH" });
  }

  getAgentTrace(sessionId: string): Promise<AgentTurnTrace[]> {
    return this.call(`/observability/agents/${sessionId}`);
  }
  getChatMetrics(chatId: string): Promise<ChatMetrics> {
    return this.call(`/observability/chats/${chatId}/metrics`);
  }
}
