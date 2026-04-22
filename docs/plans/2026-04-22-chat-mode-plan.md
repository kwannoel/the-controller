# Chat Mode Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use the-controller-executing-plans to implement this plan task-by-task.

**Goal:** Add a new `chat` workspace mode to the Tauri app that renders `the-controller-daemon` sessions as structured messages (user / agent / tool call / tool approval), alongside the existing `development` and `agents` modes.

**Architecture:** The Svelte frontend talks directly to the daemon over HTTP + WebSocket (`127.0.0.1:4867`, bearer token from `~/.the-controller/daemon.token`). A single Tauri command `read_daemon_token` reads the token file. All daemon logic lives in `src/lib/daemon/` (types, client, WS stream, store). Chat UI lives in `src/lib/chat/`. The daemon is assumed to be running externally; if unreachable, a full-panel empty state is shown. Both Claude and Codex agents are supported.

**Tech Stack:** Svelte 5 (runes), TypeScript, Tauri v2, Rust, Vitest, @testing-library/svelte, Playwright (existing e2e harness). Design doc: `docs/plans/2026-04-22-chat-mode-design.md` (commit `1e3d570`). Daemon reference: `~/projects/the-controller-daemon/` (README.md + `src/model.rs` + `docs/plans/2026-04-20-daemon-chat-architecture-design.md`).

**Process rules:** Every task follows the CLAUDE.md **Definition / Constraints / Validation** structure — stated inline per task. Commit after every green task. Use `pnpm test -- --run <file>` for single-file unit tests (no watch mode).

---

## Task 1: Extend `WorkspaceMode` type with `"chat"`

**Definition:** Add `"chat"` to the `WorkspaceMode` union so the rest of the codebase can reference it.
**Constraints:** No default mode change — `development` stays the default. No new store.
**Validation:** Existing tests still pass; a new test for the type's constituent strings passes.

**Files:**
- Modify: `src/lib/stores.ts:131-134`
- Test: `src/lib/stores.test.ts` (append)

**Step 1: Write the failing test**

```ts
// in src/lib/stores.test.ts
import { describe, it, expect } from "vitest";
import type { WorkspaceMode } from "./stores";

describe("WorkspaceMode", () => {
  it("accepts 'chat' as a valid value", () => {
    const m: WorkspaceMode = "chat";
    expect(m).toBe("chat");
  });
});
```

**Step 2: Run test — expect fail**

Run: `pnpm test -- --run src/lib/stores.test.ts`
Expected: TS compile error `Type '"chat"' is not assignable to type 'WorkspaceMode'`.

**Step 3: Implement**

Modify `src/lib/stores.ts:131-133`:
```ts
export type WorkspaceMode =
  | "development"
  | "agents"
  | "chat";
```

**Step 4: Run — expect pass**

Run: `pnpm test -- --run src/lib/stores.test.ts`
Expected: PASS.

**Step 5: Commit**

```bash
git add src/lib/stores.ts src/lib/stores.test.ts
git commit -m "feat(chat): add 'chat' to WorkspaceMode union"
```

---

## Task 2: Register `chat` in the workspace mode picker

**Definition:** The mode picker lists selectable modes. Add an entry with hotkey `c`.
**Constraints:** Keep the shape of existing entries. Label: "Chat". Position after "Agents".
**Validation:** Rendered component contains the new entry; existing snapshots / assertions unchanged.

**Files:**
- Modify: `src/lib/WorkspaceModePicker.svelte:8-11`
- Test: `src/lib/__tests__/WorkspaceModePicker.test.ts` (create if absent)

**Step 1: Write failing test**

```ts
// src/lib/__tests__/WorkspaceModePicker.test.ts
import { describe, it, expect } from "vitest";
import { render } from "@testing-library/svelte";
import WorkspaceModePicker from "../WorkspaceModePicker.svelte";

describe("WorkspaceModePicker", () => {
  it("lists chat with hotkey c", () => {
    const { getByText } = render(WorkspaceModePicker);
    expect(getByText("Chat")).toBeTruthy();
    // kbd adjacent to "Chat" shows "c"
    const chat = getByText("Chat");
    const row = chat.closest(".picker-option")!;
    expect(row.querySelector("kbd")!.textContent).toBe("c");
  });
});
```

**Step 2: Run — expect fail**

Run: `pnpm test -- --run src/lib/__tests__/WorkspaceModePicker.test.ts`
Expected: FAIL — "Chat" not found.

**Step 3: Implement**

Modify `src/lib/WorkspaceModePicker.svelte:8-11`:
```ts
  const modes: { key: string; id: WorkspaceMode; label: string }[] = [
    { key: "d", id: "development", label: "Development" },
    { key: "a", id: "agents", label: "Agents" },
    { key: "c", id: "chat", label: "Chat" },
  ];
```

**Step 4: Run — expect pass**

Run: `pnpm test -- --run src/lib/__tests__/WorkspaceModePicker.test.ts`
Expected: PASS.

**Step 5: Commit**

```bash
git add src/lib/WorkspaceModePicker.svelte src/lib/__tests__/WorkspaceModePicker.test.ts
git commit -m "feat(chat): register chat in workspace mode picker"
```

---

## Task 3: Handle `c` key in `HotkeyManager.handleWorkspaceModeKey`

**Definition:** `Space` then `c` must switch the workspace to `chat`.
**Constraints:** Mirror the existing `d` / `a` branches exactly, including `focusForModeSwitch`.
**Validation:** A unit test for the handler (extracted or via HotkeyManager.test.ts) covers the new branch.

**Files:**
- Modify: `src/lib/HotkeyManager.svelte:87-103`
- Modify: `src/lib/focus-helpers.ts:53-80` (add a `chat` branch)
- Test: `src/lib/focus-helpers.test.ts` (append)

**Step 1: Write failing test**

Append to `src/lib/focus-helpers.test.ts`:
```ts
describe("focusForModeSwitch — chat", () => {
  it("preserves session focus when switching to chat", () => {
    const projects = [{ id: "p1", sessions: [{ id: "s1", auto_worker_session: false }] }] as any;
    const result = focusForModeSwitch(
      { type: "session", sessionId: "s1", projectId: "p1" },
      "chat",
      "s1",
      projects,
    );
    // v1: preserve current focus; ChatWorkspace decides what to do with it
    expect(result).toEqual({ type: "session", sessionId: "s1", projectId: "p1" });
  });
});
```

**Step 2: Run — expect fail**

Run: `pnpm test -- --run src/lib/focus-helpers.test.ts`
Expected: TS error if the `chat` branch doesn't satisfy the type, or test failure.

**Step 3: Implement**

(a) Modify `src/lib/HotkeyManager.svelte` inside `handleWorkspaceModeKey` (around line 100) — add:
```ts
    if (key === "c") {
      workspaceMode.set("chat");
      const newFocus = focusForModeSwitch(currentFocus, "chat", activeId, projectList);
      if (newFocus !== currentFocus) focusTarget.set(newFocus);
      return;
    }
```

(b) `focus-helpers.ts` — no structural change needed; the function already falls through and returns `current`. But for type soundness, ensure the `WorkspaceMode` switch is exhaustive if a switch statement is ever used.

**Step 4: Run — expect pass**

Run: `pnpm test -- --run src/lib/focus-helpers.test.ts`
Expected: PASS.

**Step 5: Commit**

```bash
git add src/lib/HotkeyManager.svelte src/lib/focus-helpers.ts src/lib/focus-helpers.test.ts
git commit -m "feat(chat): handle Space+c to switch to chat mode"
```

---

## Task 4: Add `read_daemon_token` Tauri command

**Definition:** A Rust-side command reads `~/.the-controller/daemon.token` and returns its contents. Returns a structured error if the file is missing or unreadable.
**Constraints:** No caching — token file is small and rarely read. Path comes from `$TCD_STATE_DIR` env var if set (matches daemon default), else `~/.the-controller/`. Trim whitespace on return.
**Validation:** Rust unit test in `commands.rs` covers present / absent / unreadable.

**Files:**
- Create: `src-tauri/src/commands/daemon.rs`
- Modify: `src-tauri/src/commands.rs` (add `mod daemon;`)
- Modify: `src-tauri/src/lib.rs:61-118` (register command)

**Step 1: Write failing test**

Create `src-tauri/src/commands/daemon.rs` with only a test for now:
```rust
use std::path::PathBuf;

pub(crate) fn daemon_token_path() -> PathBuf {
    if let Ok(dir) = std::env::var("TCD_STATE_DIR") {
        return PathBuf::from(dir).join("daemon.token");
    }
    let home = std::env::var("HOME").unwrap_or_default();
    PathBuf::from(home).join(".the-controller").join("daemon.token")
}

pub(crate) fn read_token_from(path: &std::path::Path) -> Result<String, String> {
    let bytes = std::fs::read(path)
        .map_err(|e| format!("read daemon token at {}: {}", path.display(), e))?;
    let s = String::from_utf8(bytes).map_err(|e| format!("token not utf-8: {}", e))?;
    Ok(s.trim().to_string())
}

#[tauri::command]
pub async fn read_daemon_token() -> Result<String, String> {
    let path = daemon_token_path();
    tokio::task::spawn_blocking(move || read_token_from(&path))
        .await
        .map_err(|e| format!("join error: {}", e))?
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn reads_and_trims_token() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("daemon.token");
        let mut f = std::fs::File::create(&p).unwrap();
        writeln!(f, "abc123").unwrap();
        assert_eq!(read_token_from(&p).unwrap(), "abc123");
    }

    #[test]
    fn missing_token_returns_err() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("daemon.token");
        let err = read_token_from(&p).unwrap_err();
        assert!(err.contains("daemon.token"), "got: {}", err);
    }
}
```

Register `tempfile` if not already a dev-dep (check `src-tauri/Cargo.toml`; likely already used).

**Step 2: Run — expect fail initially, then pass**

Run: `cd src-tauri && cargo test commands::daemon::tests -- --nocapture`
Expected: compile errors until `mod daemon;` is added.

**Step 3: Wire the module**

Modify `src-tauri/src/commands.rs` around the existing `mod github;` / `mod media;` lines:
```rust
mod daemon;
pub use daemon::read_daemon_token;
```

Modify `src-tauri/src/lib.rs:61-118` — add `commands::read_daemon_token,` to the `tauri::generate_handler![...]` list.

**Step 4: Run — expect pass**

Run: `cd src-tauri && cargo test commands::daemon::tests`
Expected: `test result: ok. 2 passed`.

**Step 5: Commit**

```bash
git add src-tauri/src/commands.rs src-tauri/src/commands/daemon.rs src-tauri/src/lib.rs
git commit -m "feat(chat): add read_daemon_token Tauri command"
```

---

## Task 5: Daemon wire types in TypeScript

**Definition:** Mirror the daemon's `model.rs` + event schema in `src/lib/daemon/types.ts`.
**Constraints:** Exactly match the daemon's JSON shape — `snake_case` fields, union strings. No runtime validation for v1 (types only).
**Validation:** A round-trip test parses a fixture JSON file captured from the daemon's test fixtures.

**Files:**
- Create: `src/lib/daemon/types.ts`
- Create: `src/lib/daemon/__fixtures__/events.json` — copy representative outbox/inbox/system events from `~/projects/the-controller-daemon/tests/fixtures/` (pick 5–10 diverse events)
- Create: `src/lib/daemon/types.test.ts`

**Step 1: Write failing test**

```ts
// src/lib/daemon/types.test.ts
import { describe, it, expect } from "vitest";
import fixture from "./__fixtures__/events.json";
import type { EventRecord, OutboxEvent, InboxEvent, SystemEvent } from "./types";

describe("daemon event types", () => {
  it("parses fixture events without type errors", () => {
    const events = fixture as EventRecord[];
    expect(events.length).toBeGreaterThan(0);
    for (const e of events) {
      expect(["inbox", "outbox", "system"]).toContain(e.channel);
      expect(typeof e.seq).toBe("number");
    }
  });

  it("narrows outbox agent_text payload", () => {
    const e: OutboxEvent = {
      kind: "agent_text",
      payload: { message_id: "m1", block_id: "b1", text: "hello" },
    };
    expect(e.payload.text).toBe("hello");
  });
});
```

**Step 2: Run — expect fail**

Run: `pnpm test -- --run src/lib/daemon/types.test.ts`
Expected: fail — module not found.

**Step 3: Implement**

Create `src/lib/daemon/types.ts`:
```ts
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
```

Populate `events.json` by running the daemon with fake agent and piping `curl /sessions/<id>/messages?since=0` into the file. If the daemon is not running, handcraft a minimal array of 5 events covering one of each channel.

**Step 4: Run — expect pass**

Run: `pnpm test -- --run src/lib/daemon/types.test.ts`
Expected: PASS.

**Step 5: Commit**

```bash
git add src/lib/daemon/types.ts src/lib/daemon/types.test.ts src/lib/daemon/__fixtures__/events.json
git commit -m "feat(chat): add daemon wire types"
```

---

## Task 6: HTTP client (`client.ts`)

**Definition:** Typed fetch wrapper over the daemon's HTTP API with bearer auth. Only endpoints needed for v1.
**Constraints:** No third-party HTTP lib; use `fetch`. Token passed in explicitly per call — no global. Throw a typed `DaemonHttpError` on non-2xx with status + body for error-handling paths to discriminate.
**Validation:** Unit tests using `fetch` mocked via `vi.stubGlobal` cover happy paths + 401/404/409/503.

**Files:**
- Create: `src/lib/daemon/client.ts`
- Create: `src/lib/daemon/client.test.ts`

**Step 1: Write failing test**

```ts
// src/lib/daemon/client.test.ts
import { describe, it, expect, vi, beforeEach } from "vitest";
import { DaemonClient, DaemonHttpError } from "./client";

function mockFetch(responses: Array<{ status: number; body: unknown }>) {
  const queue = [...responses];
  return vi.fn(async () => {
    const r = queue.shift()!;
    return {
      ok: r.status >= 200 && r.status < 300,
      status: r.status,
      json: async () => r.body,
      text: async () => JSON.stringify(r.body),
    } as any;
  });
}

describe("DaemonClient", () => {
  beforeEach(() => vi.restoreAllMocks());

  it("listSessions sets Authorization header and returns array", async () => {
    const fetchMock = mockFetch([{ status: 200, body: [{ id: "s1", label: "x", agent: "claude" }] }]);
    vi.stubGlobal("fetch", fetchMock);
    const c = new DaemonClient("http://127.0.0.1:4867", "TOK");
    const sessions = await c.listSessions();
    expect(sessions[0].id).toBe("s1");
    const [url, init] = fetchMock.mock.calls[0];
    expect(url).toBe("http://127.0.0.1:4867/sessions");
    expect((init as any).headers.Authorization).toBe("Bearer TOK");
  });

  it("throws DaemonHttpError on 404", async () => {
    vi.stubGlobal("fetch", mockFetch([{ status: 404, body: { error: "not found" } }]));
    const c = new DaemonClient("http://127.0.0.1:4867", "TOK");
    await expect(c.getSession("missing")).rejects.toMatchObject({
      name: "DaemonHttpError",
      status: 404,
    });
  });
});
```

**Step 2: Run — expect fail (module not found)**

**Step 3: Implement**

```ts
// src/lib/daemon/client.ts
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
  constructor(private baseUrl: string, private token: string) {}

  private async call<T>(path: string, init?: RequestInit): Promise<T> {
    const res = await fetch(`${this.baseUrl}${path}`, {
      ...init,
      headers: {
        "Authorization": `Bearer ${this.token}`,
        "Content-Type": "application/json",
        ...(init?.headers ?? {}),
      },
    });
    if (!res.ok) {
      const body = await res.text().catch(() => "");
      throw new DaemonHttpError(res.status, body, `daemon ${res.status} on ${path}: ${body}`);
    }
    // 204 has no body
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
    // payload shape mirrors the daemon's inbox kinds
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
    q.set("token", this.token); // fallback if Authorization header isn't supported on WS; actual auth is via header
    return `${base}/sessions/${id}/stream?${q.toString()}`;
  }

  get bearer(): string { return this.token; }
}
```

**Step 4: Run — expect pass**

Run: `pnpm test -- --run src/lib/daemon/client.test.ts`
Expected: PASS.

**Step 5: Commit**

```bash
git add src/lib/daemon/client.ts src/lib/daemon/client.test.ts
git commit -m "feat(chat): add typed DaemonClient"
```

---

## Task 7: Event reducer (pure function)

**Definition:** A pure `reduce(events, deltaState) => renderable blocks`. Handles:
- finalized `agent_text` finalizes a block
- `agent_text_delta` accumulates into in-progress
- `tool_result` joined to `tool_call` by `call_id`
- dedupe by `(seq)`
**Constraints:** No Svelte imports. Operate on plain data for easy unit testing.
**Validation:** Golden fixture tests; revert implementation → test fails.

**Files:**
- Create: `src/lib/daemon/reducer.ts`
- Create: `src/lib/daemon/reducer.test.ts`

**Step 1: Write failing test**

```ts
// src/lib/daemon/reducer.test.ts
import { describe, it, expect } from "vitest";
import { reduceTranscript, emptyTranscript } from "./reducer";
import type { EventRecord } from "./types";

const makeEvt = (seq: number, channel: "inbox"|"outbox"|"system", kind: string, payload: any): EventRecord => ({
  session_id: "s", seq, channel, kind, payload, created_at: seq, applied_at: null,
});

describe("reduceTranscript", () => {
  it("accumulates deltas, drops in-progress on finalize", () => {
    let t = emptyTranscript();
    t = reduceTranscript(t, makeEvt(1, "outbox", "agent_text_delta", { message_id: "m1", block_id: "b1", delta: "Hel" }));
    t = reduceTranscript(t, makeEvt(2, "outbox", "agent_text_delta", { message_id: "m1", block_id: "b1", delta: "lo" }));
    expect(t.inProgressBlocks.get("b1")).toBe("Hello");
    t = reduceTranscript(t, makeEvt(3, "outbox", "agent_text", { message_id: "m1", block_id: "b1", text: "Hello" }));
    expect(t.inProgressBlocks.has("b1")).toBe(false);
    expect(t.events.at(-1)?.kind).toBe("agent_text");
  });

  it("dedupes repeated (seq)", () => {
    let t = emptyTranscript();
    const e = makeEvt(1, "inbox", "user_text", { text: "hi" });
    t = reduceTranscript(t, e);
    t = reduceTranscript(t, e);
    expect(t.events.length).toBe(1);
  });

  it("tracks status_changed", () => {
    let t = emptyTranscript();
    t = reduceTranscript(t, makeEvt(1, "system", "status_changed", { state: "working" }));
    expect(t.statusState).toBe("working");
  });
});
```

**Step 2: Run — expect fail**

**Step 3: Implement**

```ts
// src/lib/daemon/reducer.ts
import type { EventRecord, StatusState } from "./types";

export interface TranscriptState {
  events: EventRecord[];
  lastSeq: number;
  inProgressBlocks: Map<string, string>;
  statusState: StatusState | null;
  tokenUsage: { input: number; output: number; cache_read: number; cache_write: number } | null;
  seenSeq: Set<number>;
}

export function emptyTranscript(): TranscriptState {
  return {
    events: [],
    lastSeq: 0,
    inProgressBlocks: new Map(),
    statusState: null,
    tokenUsage: null,
    seenSeq: new Set(),
  };
}

export function reduceTranscript(prev: TranscriptState, e: EventRecord): TranscriptState {
  if (prev.seenSeq.has(e.seq)) return prev;
  const seenSeq = new Set(prev.seenSeq); seenSeq.add(e.seq);
  const lastSeq = Math.max(prev.lastSeq, e.seq);
  let inProgressBlocks = prev.inProgressBlocks;
  let statusState = prev.statusState;
  let tokenUsage = prev.tokenUsage;
  let events = prev.events;

  if (e.channel === "outbox" && e.kind === "agent_text_delta") {
    const p = e.payload as { block_id: string; delta: string };
    inProgressBlocks = new Map(inProgressBlocks);
    inProgressBlocks.set(p.block_id, (inProgressBlocks.get(p.block_id) ?? "") + p.delta);
    // deltas are not persisted; do not append to events
    return { ...prev, seenSeq, lastSeq, inProgressBlocks };
  }
  if (e.channel === "outbox" && e.kind === "agent_text") {
    const p = e.payload as { block_id: string };
    if (inProgressBlocks.has(p.block_id)) {
      inProgressBlocks = new Map(inProgressBlocks);
      inProgressBlocks.delete(p.block_id);
    }
    events = [...events, e];
    return { ...prev, seenSeq, lastSeq, inProgressBlocks, events };
  }
  if (e.channel === "outbox" && e.kind === "token_usage") {
    tokenUsage = e.payload as any;
  }
  if (e.channel === "system" && e.kind === "status_changed") {
    statusState = (e.payload as { state: StatusState }).state;
  }

  events = [...events, e];
  return { ...prev, seenSeq, lastSeq, events, inProgressBlocks, statusState, tokenUsage };
}
```

**Step 4: Run — expect pass**

Run: `pnpm test -- --run src/lib/daemon/reducer.test.ts`
Expected: PASS.

**Step 5: Commit**

```bash
git add src/lib/daemon/reducer.ts src/lib/daemon/reducer.test.ts
git commit -m "feat(chat): add pure event reducer"
```

---

## Task 8: Daemon store + bootstrap

**Definition:** A module-level Svelte 5 `$state` graph exposing: `token`, `reachable`, `sessions` Map, `transcripts` Map. `bootstrap()` reads the token via `read_daemon_token` and pings `GET /sessions`. Exposes `pingDaemon()`, `loadSessions()`.
**Constraints:** Svelte runes, not writables. WS handling is separate (Task 9). No WS here.
**Validation:** Test `bootstrap` by stubbing `command` and `fetch`; assert `reachable` flips correctly.

**Files:**
- Create: `src/lib/daemon/store.ts`
- Create: `src/lib/daemon/store.test.ts`

**Step 1: Write failing test**

```ts
// src/lib/daemon/store.test.ts
import { describe, it, expect, vi } from "vitest";

vi.mock("$lib/backend", () => ({
  command: vi.fn(async (cmd: string) => {
    if (cmd === "read_daemon_token") return "TOK";
    throw new Error("unexpected command " + cmd);
  }),
  listen: () => () => {},
}));

describe("daemon store bootstrap", () => {
  it("sets reachable=true on successful ping", async () => {
    const fetchMock = vi.fn(async () => ({ ok: true, status: 200, json: async () => [] } as any));
    vi.stubGlobal("fetch", fetchMock);
    const { daemonStore, bootstrap } = await import("./store");
    await bootstrap();
    expect(daemonStore.reachable).toBe(true);
    expect(daemonStore.token).toBe("TOK");
  });

  it("sets reachable=false when ping fails", async () => {
    vi.resetModules();
    vi.doMock("$lib/backend", () => ({
      command: vi.fn(async () => "TOK"),
      listen: () => () => {},
    }));
    vi.stubGlobal("fetch", vi.fn(async () => { throw new TypeError("connect refused"); }));
    const { daemonStore, bootstrap } = await import("./store");
    await bootstrap();
    expect(daemonStore.reachable).toBe(false);
  });
});
```

**Step 2: Run — expect fail**

**Step 3: Implement**

```ts
// src/lib/daemon/store.ts
import { command } from "$lib/backend";
import { DaemonClient } from "./client";
import type { DaemonSession } from "./types";
import { emptyTranscript, type TranscriptState } from "./reducer";

const BASE_URL = "http://127.0.0.1:4867";

interface StoreState {
  token: string | null;
  reachable: boolean;
  client: DaemonClient | null;
  sessions: Map<string, DaemonSession>;
  transcripts: Map<string, TranscriptState>;
  activeSessionId: string | null;
}

export const daemonStore = $state<StoreState>({
  token: null,
  reachable: false,
  client: null,
  sessions: new Map(),
  transcripts: new Map(),
  activeSessionId: null,
});

export async function bootstrap(): Promise<void> {
  try {
    const token = await command<string>("read_daemon_token");
    daemonStore.token = token;
    daemonStore.client = new DaemonClient(BASE_URL, token);
  } catch (e) {
    daemonStore.reachable = false;
    daemonStore.token = null;
    daemonStore.client = null;
    return;
  }
  await pingDaemon();
}

export async function pingDaemon(): Promise<void> {
  if (!daemonStore.client) { daemonStore.reachable = false; return; }
  try {
    await daemonStore.client.listSessions();
    daemonStore.reachable = true;
  } catch {
    daemonStore.reachable = false;
  }
}

export async function loadSessions(): Promise<void> {
  if (!daemonStore.client) return;
  const list = await daemonStore.client.listSessions();
  const map = new Map<string, DaemonSession>();
  for (const s of list) {
    map.set(s.id, s);
    if (!daemonStore.transcripts.has(s.id)) {
      daemonStore.transcripts.set(s.id, emptyTranscript());
    }
  }
  daemonStore.sessions = map;
}
```

Note: `$state` at module top-level must be used inside `.svelte.ts` files in Svelte 5. Rename to `store.svelte.ts`.

**Step 4: Run — expect pass**

Run: `pnpm test -- --run src/lib/daemon/store.test.ts`
Expected: PASS. If `$state` isn't allowed at module top-level in `.ts`, rename to `src/lib/daemon/store.svelte.ts` and update imports.

**Step 5: Commit**

```bash
git add src/lib/daemon/store.svelte.ts src/lib/daemon/store.test.ts
git commit -m "feat(chat): add daemon store + bootstrap"
```

---

## Task 9: WS stream with reconnect

**Definition:** `openStream(sessionId)` manages one WebSocket per session. Feeds incoming events into `reduceTranscript` via the store. Reconnect with exponential backoff cap 10s, resume with `?since=<lastSeq>`.
**Constraints:** One WS per active session (close on inactive). Auth header via `Authorization: Bearer <token>` in the WS upgrade — if browsers disallow, fall back to the `?token=` query param from `wsUrl`. Start with `?token=` for v1; revisit.
**Validation:** Unit test with a mock `WebSocket` global; simulate close + reconnect, assert URL contains correct `since`.

**Files:**
- Create: `src/lib/daemon/stream.ts`
- Create: `src/lib/daemon/stream.test.ts`

**Step 1: Write failing test**

```ts
// src/lib/daemon/stream.test.ts
import { describe, it, expect, vi, beforeEach } from "vitest";

class MockWebSocket {
  static instances: MockWebSocket[] = [];
  static clear() { this.instances = []; }
  onopen: ((e: any) => void) | null = null;
  onmessage: ((e: any) => void) | null = null;
  onclose: ((e: any) => void) | null = null;
  onerror: ((e: any) => void) | null = null;
  readyState = 0;
  constructor(public url: string) { MockWebSocket.instances.push(this); }
  send() {}
  close() { this.readyState = 3; this.onclose?.({ code: 1006 }); }
}

describe("stream", () => {
  beforeEach(() => {
    vi.useFakeTimers();
    MockWebSocket.clear();
    vi.stubGlobal("WebSocket", MockWebSocket as any);
  });

  it("reconnects with since=<lastSeq>", async () => {
    vi.resetModules();
    vi.doMock("$lib/backend", () => ({ command: vi.fn(async () => "TOK"), listen: () => () => {} }));
    const { daemonStore } = await import("./store");
    daemonStore.token = "TOK";
    daemonStore.client = { wsUrl: (id: string, since: number) => `ws://x/sessions/${id}/stream?since=${since}&token=TOK` } as any;
    daemonStore.transcripts.set("s1", { events: [], lastSeq: 5, inProgressBlocks: new Map(), statusState: null, tokenUsage: null, seenSeq: new Set() });
    const { openStream } = await import("./stream");
    const handle = openStream("s1");
    expect(MockWebSocket.instances.length).toBe(1);
    MockWebSocket.instances[0].close();
    await vi.advanceTimersByTimeAsync(1100);
    expect(MockWebSocket.instances[1].url).toContain("since=5");
    handle.close();
  });
});
```

**Step 2: Run — expect fail**

**Step 3: Implement**

```ts
// src/lib/daemon/stream.ts
import { daemonStore } from "./store.svelte";
import { reduceTranscript } from "./reducer";
import type { EventRecord } from "./types";

export interface StreamHandle {
  close(): void;
}

export function openStream(sessionId: string): StreamHandle {
  let closed = false;
  let ws: WebSocket | null = null;
  let attempt = 0;

  function connect() {
    if (closed) return;
    const client = daemonStore.client;
    if (!client) return;
    const t = daemonStore.transcripts.get(sessionId);
    const since = t?.lastSeq ?? 0;
    const url = client.wsUrl(sessionId, since);
    ws = new WebSocket(url);
    ws.onmessage = (ev) => {
      try {
        const evt = JSON.parse(ev.data) as EventRecord;
        const prev = daemonStore.transcripts.get(sessionId) ?? undefined;
        const next = reduceTranscript(prev ?? { events: [], lastSeq: 0, inProgressBlocks: new Map(), statusState: null, tokenUsage: null, seenSeq: new Set() }, evt);
        daemonStore.transcripts.set(sessionId, next);
      } catch {}
    };
    ws.onclose = () => {
      if (closed) return;
      const delay = Math.min(10_000, 500 * 2 ** attempt);
      attempt += 1;
      setTimeout(connect, delay);
    };
    ws.onopen = () => { attempt = 0; };
  }

  connect();
  return {
    close() {
      closed = true;
      ws?.close();
    },
  };
}
```

**Step 4: Run — expect pass**

Run: `pnpm test -- --run src/lib/daemon/stream.test.ts`
Expected: PASS.

**Step 5: Commit**

```bash
git add src/lib/daemon/stream.ts src/lib/daemon/stream.test.ts
git commit -m "feat(chat): WS stream with reconnect and resume-from-seq"
```

---

## Task 10: `ChatWorkspace.svelte` + `DaemonEmptyState.svelte`

**Definition:** Root chat component that renders either the empty state (unreachable) or a placeholder chat area (filled in later).
**Constraints:** On mount, call `bootstrap()`. Render `<DaemonEmptyState />` when `!reachable`.
**Validation:** Component test — stub `bootstrap` to set `reachable=false`, assert empty state renders.

**Files:**
- Create: `src/lib/chat/ChatWorkspace.svelte`
- Create: `src/lib/chat/DaemonEmptyState.svelte`
- Create: `src/lib/chat/ChatWorkspace.test.ts`

**Step 1: Write failing test**

```ts
import { describe, it, expect, vi } from "vitest";
import { render, fireEvent } from "@testing-library/svelte";

vi.mock("../daemon/store.svelte", async () => {
  const actual = await vi.importActual("../daemon/store.svelte");
  return {
    ...actual,
    bootstrap: vi.fn(async () => {}),
    pingDaemon: vi.fn(async () => {}),
  };
});

import ChatWorkspace from "./ChatWorkspace.svelte";
import { daemonStore } from "../daemon/store.svelte";

describe("ChatWorkspace", () => {
  it("renders empty state when daemon unreachable", async () => {
    daemonStore.reachable = false;
    const { findByText } = render(ChatWorkspace);
    expect(await findByText(/Daemon not running/)).toBeTruthy();
  });

  it("shows Retry button that calls pingDaemon", async () => {
    daemonStore.reachable = false;
    const { getByText } = render(ChatWorkspace);
    const btn = getByText("Retry");
    await fireEvent.click(btn);
    const { pingDaemon } = await import("../daemon/store.svelte");
    expect(pingDaemon).toHaveBeenCalled();
  });
});
```

**Step 2: Run — expect fail**

**Step 3: Implement**

```svelte
<!-- src/lib/chat/DaemonEmptyState.svelte -->
<script lang="ts">
  let { onRetry }: { onRetry: () => void } = $props();
</script>

<div class="empty">
  <h2>Daemon not running</h2>
  <p>Start it with:</p>
  <pre><code>./target/release/the-controller-daemon</code></pre>
  <p class="muted">Expected token: <code>~/.the-controller/daemon.token</code></p>
  <button onclick={onRetry}>Retry</button>
</div>

<style>
  .empty { max-width: 560px; margin: 10vh auto; text-align: center; color: var(--text-secondary); }
  pre { background: var(--bg-elevated); padding: 12px; border-radius: 6px; }
  button { margin-top: 16px; padding: 6px 14px; }
  .muted { font-size: 12px; opacity: 0.7; }
</style>
```

```svelte
<!-- src/lib/chat/ChatWorkspace.svelte -->
<script lang="ts">
  import { onMount } from "svelte";
  import { daemonStore, bootstrap, pingDaemon, loadSessions } from "../daemon/store.svelte";
  import DaemonEmptyState from "./DaemonEmptyState.svelte";

  onMount(async () => {
    await bootstrap();
    if (daemonStore.reachable) await loadSessions();
  });

  async function handleRetry() {
    await pingDaemon();
    if (daemonStore.reachable) await loadSessions();
  }
</script>

{#if !daemonStore.reachable}
  <DaemonEmptyState onRetry={handleRetry} />
{:else}
  <div class="chat-main">
    <p>Chat mode (placeholder)</p>
  </div>
{/if}

<style>
  .chat-main { padding: 16px; color: var(--text-primary); }
</style>
```

**Step 4: Wire into App.svelte**

Modify `src/App.svelte:375-379`:
```svelte
        {#if workspaceModeState.current === "agents"}
          <AgentDashboard />
        {:else if workspaceModeState.current === "chat"}
          <ChatWorkspace />
        {:else}
          <TerminalManager />
        {/if}
```

Add import at `src/App.svelte` top alongside other component imports:
```ts
import ChatWorkspace from "./lib/chat/ChatWorkspace.svelte";
```

**Step 5: Run — expect pass**

Run: `pnpm test -- --run src/lib/chat/ChatWorkspace.test.ts`
Expected: PASS.

**Step 6: Commit**

```bash
git add src/lib/chat/ChatWorkspace.svelte src/lib/chat/DaemonEmptyState.svelte src/lib/chat/ChatWorkspace.test.ts src/App.svelte
git commit -m "feat(chat): mount ChatWorkspace with DaemonEmptyState"
```

---

## Task 11: `ChatSessionList.svelte` + Sidebar integration

**Definition:** In chat mode, the sidebar shows daemon sessions grouped by project (match by `cwd === project.repo_path`), plus an "Other" group for unmatched sessions, and a "+ New chat" row per project.
**Constraints:** Reuse the existing `Sidebar.svelte` container. Branch on `currentMode === "chat"` the way it already branches for `agents`. Pure grouping logic extracted to `src/lib/daemon/grouping.ts` for unit testing.
**Validation:** Grouping unit test + a component render test that "Other" group appears when a session cwd doesn't match.

**Files:**
- Create: `src/lib/daemon/grouping.ts`
- Create: `src/lib/daemon/grouping.test.ts`
- Create: `src/lib/chat/ChatSessionList.svelte`
- Modify: `src/lib/Sidebar.svelte` (branch for chat mode)

**Step 1: Failing test (grouping)**

```ts
// src/lib/daemon/grouping.test.ts
import { describe, it, expect } from "vitest";
import { groupSessionsByProject } from "./grouping";

describe("groupSessionsByProject", () => {
  it("buckets sessions by project.repo_path; unmatched into Other", () => {
    const projects = [{ id: "p1", name: "A", repo_path: "/tmp/a" }, { id: "p2", name: "B", repo_path: "/tmp/b" }] as any;
    const sessions = [
      { id: "s1", cwd: "/tmp/a" }, { id: "s2", cwd: "/tmp/a" }, { id: "s3", cwd: "/other" },
    ] as any;
    const groups = groupSessionsByProject(projects, sessions);
    expect(groups.byProject.get("p1")!.map((s: any) => s.id)).toEqual(["s1", "s2"]);
    expect(groups.byProject.get("p2")).toEqual([]);
    expect(groups.other.map((s: any) => s.id)).toEqual(["s3"]);
  });
});
```

**Step 2: Implement**

```ts
// src/lib/daemon/grouping.ts
import type { DaemonSession } from "./types";
import type { Project } from "$lib/stores";

export interface SessionGroups {
  byProject: Map<string, DaemonSession[]>;
  other: DaemonSession[];
}

export function groupSessionsByProject(projects: Project[], sessions: DaemonSession[]): SessionGroups {
  const byProject = new Map<string, DaemonSession[]>();
  for (const p of projects) byProject.set(p.id, []);
  const pathToId = new Map(projects.map(p => [p.repo_path, p.id]));
  const other: DaemonSession[] = [];
  for (const s of sessions) {
    const pid = pathToId.get(s.cwd);
    if (pid) byProject.get(pid)!.push(s);
    else other.push(s);
  }
  return { byProject, other };
}
```

**Step 3: ChatSessionList**

```svelte
<!-- src/lib/chat/ChatSessionList.svelte -->
<script lang="ts">
  import { projects as projectsStore } from "$lib/stores";
  import { fromStore } from "svelte/store";
  import { daemonStore } from "../daemon/store.svelte";
  import { groupSessionsByProject } from "../daemon/grouping";

  const projectsState = fromStore(projectsStore);
  const projectList = $derived(projectsState.current);
  const sessionsList = $derived([...daemonStore.sessions.values()]);
  const groups = $derived(groupSessionsByProject(projectList, sessionsList));

  let { onNewChat, onSelect }: { onNewChat: (projectId: string) => void; onSelect: (sessionId: string) => void } = $props();
</script>

<div class="list">
  {#each projectList as p}
    <div class="project-row">{p.name}</div>
    {#each groups.byProject.get(p.id) ?? [] as s (s.id)}
      <button class="session-row" onclick={() => onSelect(s.id)}>
        <span class="status status-{s.status}"></span>
        <span class="label">{s.label}</span>
        <span class="agent">{s.agent}</span>
      </button>
    {/each}
    <button class="new" onclick={() => onNewChat(p.id)}>+ New chat</button>
  {/each}
  {#if groups.other.length}
    <div class="project-row">Other</div>
    {#each groups.other as s (s.id)}
      <button class="session-row" onclick={() => onSelect(s.id)}>
        <span class="status status-{s.status}"></span>
        <span class="label">{s.label}</span>
        <span class="agent">{s.agent}</span>
      </button>
    {/each}
  {/if}
</div>

<style>
  .list { display: flex; flex-direction: column; gap: 4px; padding: 8px; }
  .project-row { font-weight: 600; font-size: 13px; padding: 4px 0; }
  .session-row, .new { background: transparent; border: 0; color: inherit; text-align: left; padding: 4px 8px; cursor: pointer; display: flex; gap: 8px; align-items: center; }
  .session-row:hover, .new:hover { background: rgba(255,255,255,0.05); }
  .status { width: 8px; height: 8px; border-radius: 50%; }
  .status-running { background: #a6e3a1; }
  .status-ended, .status-failed { background: #f38ba8; }
  .status-starting, .status-interrupted { background: #f9e2af; }
  .agent { margin-left: auto; font-size: 11px; opacity: 0.6; }
</style>
```

**Step 4: Sidebar integration**

Modify `src/lib/Sidebar.svelte` — find the `{#if currentMode === "agents"}` branch and add a peer for chat mode (or refactor into three branches). Mount `<ChatSessionList onNewChat={...} onSelect={...} />` passing handlers that set `daemonStore.activeSessionId` and open the new chat dialog (Task 12 wires these up).

**Step 5: Run tests**

Run: `pnpm test -- --run src/lib/daemon/grouping.test.ts`
Expected: PASS.

**Step 6: Commit**

```bash
git add src/lib/daemon/grouping.ts src/lib/daemon/grouping.test.ts src/lib/chat/ChatSessionList.svelte src/lib/Sidebar.svelte
git commit -m "feat(chat): sidebar session list grouped by project"
```

---

## Task 12: `NewChatDialog.svelte` (create session flow)

**Definition:** Modal invoked from "+ New chat". Fields: agent (`claude` | `codex`) dropdown, optional initial prompt. On submit, calls `client.createSession({ agent, cwd: project.repo_path, initial_prompt })`.
**Constraints:** Submit disabled until agent selected. On success, close and set `daemonStore.activeSessionId` to the new id. On 422 error, show inline error ("Agent binary not configured on daemon").
**Validation:** Component test: submit fires expected payload; 422 shows inline error.

**Files:**
- Create: `src/lib/chat/NewChatDialog.svelte`
- Create: `src/lib/chat/NewChatDialog.test.ts`

**Step 1: Failing test**

```ts
// src/lib/chat/NewChatDialog.test.ts
import { describe, it, expect, vi } from "vitest";
import { render, fireEvent } from "@testing-library/svelte";

const createSession = vi.fn();
vi.mock("../daemon/store.svelte", () => ({
  daemonStore: { client: { createSession }, activeSessionId: null } as any,
}));

import NewChatDialog from "./NewChatDialog.svelte";

describe("NewChatDialog", () => {
  it("submits agent + cwd + initial prompt", async () => {
    createSession.mockResolvedValueOnce({ id: "s1", label: "session-1" });
    const onClose = vi.fn();
    const { getByLabelText, getByText } = render(NewChatDialog, { projectId: "p1", projectCwd: "/tmp/a", onClose });
    await fireEvent.change(getByLabelText("Agent"), { target: { value: "claude" } });
    await fireEvent.change(getByLabelText("Initial prompt"), { target: { value: "hi" } });
    await fireEvent.click(getByText("Create"));
    expect(createSession).toHaveBeenCalledWith({ agent: "claude", cwd: "/tmp/a", initial_prompt: "hi" });
    expect(onClose).toHaveBeenCalled();
  });
});
```

**Step 2: Implement**

```svelte
<!-- src/lib/chat/NewChatDialog.svelte -->
<script lang="ts">
  import { daemonStore } from "../daemon/store.svelte";
  import { DaemonHttpError } from "../daemon/client";
  import type { Agent } from "../daemon/types";

  let { projectId, projectCwd, onClose }: { projectId: string; projectCwd: string; onClose: () => void } = $props();

  let agent = $state<Agent | "">("");
  let initialPrompt = $state("");
  let error = $state<string | null>(null);
  let busy = $state(false);

  const canSubmit = $derived(agent !== "" && !busy);

  async function submit() {
    if (!agent || !daemonStore.client) return;
    busy = true;
    error = null;
    try {
      const res = await daemonStore.client.createSession({
        agent,
        cwd: projectCwd,
        ...(initialPrompt ? { initial_prompt: initialPrompt } : {}),
      });
      daemonStore.activeSessionId = res.id;
      onClose();
    } catch (e) {
      if (e instanceof DaemonHttpError && e.status === 422) {
        error = "Agent binary not configured on the daemon.";
      } else {
        error = String(e);
      }
    } finally {
      busy = false;
    }
  }
</script>

<div class="overlay" role="dialog">
  <div class="dialog">
    <h3>New chat</h3>
    <label>Agent
      <select bind:value={agent} aria-label="Agent">
        <option value="">Select…</option>
        <option value="claude">Claude Code</option>
        <option value="codex">Codex</option>
      </select>
    </label>
    <label>Initial prompt
      <textarea bind:value={initialPrompt} aria-label="Initial prompt" rows="3"></textarea>
    </label>
    {#if error}<p class="err">{error}</p>{/if}
    <div class="actions">
      <button onclick={onClose}>Cancel</button>
      <button disabled={!canSubmit} onclick={submit}>Create</button>
    </div>
  </div>
</div>

<style>
  .overlay { position: fixed; inset: 0; background: rgba(0,0,0,0.7); display: flex; align-items: center; justify-content: center; z-index: 200; }
  .dialog { background: var(--bg-elevated); padding: 24px; border-radius: 8px; min-width: 360px; display: flex; flex-direction: column; gap: 12px; }
  label { display: flex; flex-direction: column; gap: 4px; font-size: 12px; }
  .err { color: #f38ba8; font-size: 12px; }
  .actions { display: flex; justify-content: flex-end; gap: 8px; }
</style>
```

Then wire the `onNewChat` handler in `ChatWorkspace.svelte` to render this dialog with the selected project's `repo_path`.

**Step 3: Run tests**

Run: `pnpm test -- --run src/lib/chat/NewChatDialog.test.ts`
Expected: PASS.

**Step 4: Commit**

```bash
git add src/lib/chat/NewChatDialog.svelte src/lib/chat/NewChatDialog.test.ts src/lib/chat/ChatWorkspace.svelte
git commit -m "feat(chat): new chat dialog with agent selector"
```

---

## Task 13: `ChatView.svelte` + `Transcript.svelte` scaffolding

**Definition:** When a session is active, render header + scrollable transcript + input area. On mount, hydrate events via `client.readEvents(sid, 0)` and open a WS stream. Tear down on unmount.
**Constraints:** Virtualization deferred — plain scroll container for v1. The transcript derives its block list from `TranscriptState`. Messages render via `MessageBlock` (Task 14+). Header shows `label · agent · status`.
**Validation:** Component test: initial `readEvents` response is applied; a push via WS stream appears.

**Files:**
- Create: `src/lib/chat/ChatView.svelte`
- Create: `src/lib/chat/Transcript.svelte`
- Create: `src/lib/chat/ChatView.test.ts`

**Step 1: Failing test**

```ts
import { describe, it, expect, vi } from "vitest";
import { render, waitFor } from "@testing-library/svelte";

const readEvents = vi.fn(async () => [
  { session_id: "s1", seq: 1, channel: "outbox", kind: "agent_text", payload: { block_id: "b1", message_id: "m1", text: "hello" }, created_at: 1, applied_at: null },
]);
vi.mock("../daemon/store.svelte", () => ({
  daemonStore: {
    client: { readEvents, wsUrl: () => "ws://x" } as any,
    sessions: new Map([["s1", { id: "s1", label: "Chat 1", agent: "claude", status: "running" }]]),
    transcripts: new Map(),
    activeSessionId: "s1",
  },
}));
vi.mock("../daemon/stream", () => ({ openStream: vi.fn(() => ({ close: vi.fn() })) }));

import ChatView from "./ChatView.svelte";

describe("ChatView", () => {
  it("hydrates from readEvents and renders the agent text", async () => {
    const { findByText } = render(ChatView, { sessionId: "s1" });
    expect(await findByText("hello")).toBeTruthy();
  });
});
```

**Step 2: Implement**

```svelte
<!-- src/lib/chat/ChatView.svelte -->
<script lang="ts">
  import { onMount, onDestroy } from "svelte";
  import { daemonStore } from "../daemon/store.svelte";
  import { reduceTranscript, emptyTranscript } from "../daemon/reducer";
  import { openStream } from "../daemon/stream";
  import Transcript from "./Transcript.svelte";

  let { sessionId }: { sessionId: string } = $props();
  const session = $derived(daemonStore.sessions.get(sessionId));
  const transcript = $derived(daemonStore.transcripts.get(sessionId) ?? emptyTranscript());

  let handle: { close(): void } | null = null;

  onMount(async () => {
    if (!daemonStore.client) return;
    const events = await daemonStore.client.readEvents(sessionId, 0);
    let t = daemonStore.transcripts.get(sessionId) ?? emptyTranscript();
    for (const e of events) t = reduceTranscript(t, e);
    daemonStore.transcripts.set(sessionId, t);
    handle = openStream(sessionId);
  });
  onDestroy(() => handle?.close());
</script>

{#if session}
  <div class="view">
    <header>
      <span class="label">{session.label}</span>
      <span class="agent">{session.agent}</span>
      <span class="status status-{session.status}">{session.status}</span>
    </header>
    <Transcript {transcript} />
    <!-- ChatInput mounted in Task 17 -->
  </div>
{:else}
  <p>Session not found.</p>
{/if}

<style>
  .view { display: flex; flex-direction: column; height: 100%; }
  header { display: flex; gap: 12px; padding: 8px 12px; border-bottom: 1px solid var(--border-default); }
  .agent, .status { font-size: 11px; opacity: 0.7; }
</style>
```

```svelte
<!-- src/lib/chat/Transcript.svelte -->
<script lang="ts">
  import type { TranscriptState } from "../daemon/reducer";
  let { transcript }: { transcript: TranscriptState } = $props();
  // placeholder; MessageBlock wiring lands in Task 14
</script>

<div class="scroll">
  {#each transcript.events as e (e.seq)}
    {#if e.channel === "outbox" && e.kind === "agent_text"}
      <div class="agent">{(e.payload as any).text}</div>
    {:else if e.channel === "inbox" && e.kind === "user_text"}
      <div class="user">{(e.payload as any).text}</div>
    {/if}
  {/each}
  {#each [...transcript.inProgressBlocks.entries()] as [id, text] (id)}
    <div class="agent partial">{text}</div>
  {/each}
</div>

<style>
  .scroll { flex: 1; overflow-y: auto; padding: 12px; }
  .agent, .user { padding: 6px 10px; margin-bottom: 6px; border-radius: 6px; }
  .agent { background: var(--bg-elevated); }
  .user { background: rgba(137, 180, 250, 0.15); text-align: right; }
  .partial::after { content: " ▋"; }
</style>
```

**Step 3: Run**

Run: `pnpm test -- --run src/lib/chat/ChatView.test.ts`
Expected: PASS.

**Step 4: Commit**

```bash
git add src/lib/chat/ChatView.svelte src/lib/chat/Transcript.svelte src/lib/chat/ChatView.test.ts
git commit -m "feat(chat): ChatView + Transcript scaffolding"
```

---

## Task 14: `MessageBlock.svelte` dispatch + `UserMessage` / `AgentMessage` with markdown

**Definition:** Replace the inline branches in `Transcript.svelte` with a `<MessageBlock>` that dispatches on kind. `AgentMessage` renders markdown via `src/lib/markdown.ts`.
**Constraints:** Keep delta rendering (from `inProgressBlocks`) separate from finalized blocks — deltas are plain text until finalized, then re-render as markdown.
**Validation:** Component test: markdown headings/code render in AgentMessage.

**Files:**
- Create: `src/lib/chat/MessageBlock.svelte`
- Create: `src/lib/chat/UserMessage.svelte`
- Create: `src/lib/chat/AgentMessage.svelte`
- Modify: `src/lib/chat/Transcript.svelte`
- Create: `src/lib/chat/AgentMessage.test.ts`

Code omitted for brevity — implement straightforwardly using existing `markdown.ts`:
```svelte
<!-- AgentMessage -->
<script lang="ts">
  import { renderMarkdown } from "$lib/markdown";
  let { text }: { text: string } = $props();
</script>
<div class="agent">{@html renderMarkdown(text)}</div>
```

**Validation test** (`AgentMessage.test.ts`):
```ts
it("renders inline code", async () => {
  const { container } = render(AgentMessage, { text: "Run `ls`" });
  expect(container.querySelector("code")?.textContent).toBe("ls");
});
```

**Commit:**
```bash
git commit -m "feat(chat): MessageBlock dispatch with markdown AgentMessage"
```

---

## Task 15: `ToolCallBlock.svelte` (inline collapsed, expand on click, tool_result join)

**Definition:** Renders a one-liner `🔧 {tool}: {summary}` with a disclosure triangle. Click to expand input (JSON pretty) and attached `tool_result` (if any).
**Constraints:** `tool_result` is matched by `call_id`. Matching done in `Transcript.svelte` — pass `{ call: ToolCallEvent, result: ToolResultEvent | null }` into the component.
**Validation:** Component test: click expands; tool_result attaches when present; `is_error` adds an error class.

**Files:**
- Create: `src/lib/chat/ToolCallBlock.svelte`
- Create: `src/lib/chat/ToolCallBlock.test.ts`
- Modify: `src/lib/chat/Transcript.svelte` (do call-id join)

**Key test:**
```ts
it("expands to show tool_result", async () => {
  const call = { kind: "tool_call", payload: { call_id: "c1", tool: "Bash", input: { command: "ls" } } } as any;
  const result = { kind: "tool_result", payload: { call_id: "c1", output: "file\n", is_error: false } } as any;
  const { getByRole, getByText } = render(ToolCallBlock, { call, result });
  await fireEvent.click(getByRole("button"));
  expect(getByText(/file/)).toBeTruthy();
});
```

**Commit:**
```bash
git commit -m "feat(chat): collapsed ToolCallBlock with tool_result join"
```

---

## Task 16: `ToolApprovalBlock.svelte` (inline Approve / Deny)

**Definition:** When `statusState === "waiting_for_tool_approval"` and the last `tool_call` has no matching `tool_result`, render Approve / Deny / Deny-with-reason buttons on that block. On click, POST `tool_approval` inbox.
**Constraints:** Buttons disabled while request in flight. Deny-with-reason expands an inline textarea.
**Validation:** Component test: clicking Approve calls `client.sendMessage` with `{kind:'tool_approval', call_id, approved:true}`.

**Files:**
- Create: `src/lib/chat/ToolApprovalBlock.svelte`
- Create: `src/lib/chat/ToolApprovalBlock.test.ts`
- Modify: `src/lib/chat/Transcript.svelte` (pass status + last pending call_id)

**Commit:**
```bash
git commit -m "feat(chat): inline tool approval Approve/Deny"
```

---

## Task 17: `ChatInput.svelte` (send + interrupt + disabled states)

**Definition:** Textarea. `Cmd+Enter` sends `user_text`. `Esc` sends `interrupt` when `statusState !== "idle"`. Disabled on `ended`/`failed`.
**Constraints:** Optimistic insert with seq from 202 response. Dedupe relies on the reducer (Task 7).
**Validation:** Component test: `Cmd+Enter` triggers `client.sendMessage`; `Esc` triggers interrupt only when running.

**Files:**
- Create: `src/lib/chat/ChatInput.svelte`
- Create: `src/lib/chat/ChatInput.test.ts`
- Modify: `src/lib/chat/ChatView.svelte` (mount `<ChatInput sessionId={...} />` below transcript)

**Commit:**
```bash
git commit -m "feat(chat): ChatInput with Cmd+Enter send and Esc interrupt"
```

---

## Task 18: HTTP-error policy in `ChatView`

**Definition:** Route the `DaemonHttpError` cases per the design doc table: 401 (re-read + retry once), 404 (close view, toast), 409 (disable input, banner), 422/503 (toast + keep open), network error (inline retry).
**Constraints:** Shared helper `src/lib/daemon/errors.ts` with a `classifyError(err)` function; `ChatView` / `ChatInput` call it.
**Validation:** Unit tests per case in `errors.test.ts`.

**Files:**
- Create: `src/lib/daemon/errors.ts`
- Create: `src/lib/daemon/errors.test.ts`
- Modify: `src/lib/chat/ChatView.svelte`, `src/lib/chat/ChatInput.svelte`

**Commit:**
```bash
git commit -m "feat(chat): standardize daemon HTTP-error handling"
```

---

## Task 19: Sidebar keyboard navigation for chat items

**Definition:** Extend `HotkeyManager.getVisibleItems` (`src/lib/HotkeyManager.svelte:110-131`) so in `chat` mode the walked items are project → chats → next project, analogous to the `development` branch. `Enter` on chat → set `daemonStore.activeSessionId`. `Enter` on "+ New chat" → open `NewChatDialog`.
**Constraints:** Reuse existing `navigateItem` / `getVisibleItems` patterns.
**Validation:** Add a case to `HotkeyManager.test.ts` (pattern file already exists).

**Files:**
- Modify: `src/lib/HotkeyManager.svelte`
- Modify: `src/lib/HotkeyManager.test.ts`

**Commit:**
```bash
git commit -m "feat(chat): keyboard navigation for chat sidebar"
```

---

## Task 20: HotkeyHelp for chat mode

**Definition:** Add chat mode hotkey reference in `src/lib/HotkeyHelp.svelte:36`.
**Constraints:** Pattern mirrors how `"agents"` is labeled.
**Validation:** Snapshot / text-content test that "Chat" mode label appears when mode = chat.

**Files:**
- Modify: `src/lib/HotkeyHelp.svelte`

**Commit:**
```bash
git commit -m "feat(chat): HotkeyHelp label for chat mode"
```

---

## Task 21: Integration test — fake agent end-to-end (spine)

**Definition:** Playwright test that exercises the full stack: start the daemon with the fake agent, open the Tauri app, switch to chat mode, create a chat, send a message, observe streaming deltas, finalized text, a tool_call expand, a tool approval flow, then kill the daemon and verify empty state.
**Constraints:** Reuse existing e2e harness. Reference skill: `@the-controller-general-e2e-eval`. The daemon binary must be built once per run; fake agent binary is built by the daemon's own `cargo test` (shared by `cargo build` with `[[bin]]`).
**Validation:** The test passes end-to-end.

**Files:**
- Create: `tests/e2e/chat-mode.spec.ts` (match whatever directory the existing Playwright tests use; discover via `git ls-files | grep '\.spec\.ts$'`)
- Create: `scripts/test-chat-integration.sh` — builds daemon, starts it with `TCD_STATE_DIR=$(mktemp -d)` + `TCD_AGENT_CLAUDE_BINARY=<fake_agent_path>`, runs Playwright, tears down.

**Sketch:**
```ts
test("chat mode end-to-end against fake agent", async ({ page }) => {
  // daemon already running via scripts/test-chat-integration.sh
  await page.goto(appUrl);
  await page.keyboard.press(" ");
  await page.keyboard.press("c");
  await expect(page.getByText("Chat")).toBeVisible();
  // select first project's "+ New chat"
  await page.getByText("+ New chat").first().click();
  await page.getByLabel("Agent").selectOption("claude");
  await page.getByRole("button", { name: "Create" }).click();
  // type a message
  await page.keyboard.type("hello");
  await page.keyboard.press("Meta+Enter");
  await expect(page.getByText(/scripted-reply/)).toBeVisible({ timeout: 5000 });
  // ...
});
```

**Commit:**
```bash
git commit -m "test(chat): Playwright integration against fake agent"
```

---

## Final task: Manual smoke and PR

1. Start daemon: `~/projects/the-controller-daemon/target/release/the-controller-daemon` with `TCD_AGENT_CLAUDE_BINARY` pointed at the real `claude` binary.
2. `pnpm tauri dev` in the worktree. `Space c` → chat mode.
3. Create a chat in any project. Send "hello". Verify streamed reply + tool calls + interrupt work.
4. Stop the daemon; verify `DaemonEmptyState` appears.
5. Restart daemon; click Retry; verify state restores.
6. Run full test suite: `pnpm test -- --run` + `cd src-tauri && cargo test`.
7. Use `@the-controller-verification-before-completion` before declaring done.
8. Open PR per `@the-controller-finishing-a-development-branch`.

---

## Notes for implementer

- **Svelte 5 runes:** module-level `$state` requires `.svelte.ts` or `.svelte` files — see Task 8.
- **Token over WS:** using `?token=` query param for v1; revisit if we need Bearer in Upgrade.
- **Markdown escaping:** use the existing `src/lib/markdown.ts` helpers — they already handle the sanitization. Do not introduce a new markdown library.
- **Dev UX:** while iterating, run `pnpm test` in one terminal (watch mode) and `pnpm tauri dev` in another.
- **Daemon unreachable during tests:** every test that would hit `fetch('http://127.0.0.1:4867/...')` MUST stub `fetch` or `DaemonClient`. Never let unit tests hit the network.
