import type { DaemonSession, EventRecord, Agent, Channel } from "./types";

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
}
