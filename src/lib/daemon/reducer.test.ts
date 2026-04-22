import { describe, it, expect } from "vitest";
import { reduceTranscript, emptyTranscript } from "./reducer";
import type { EventRecord } from "./types";

const makeEvt = (seq: number, channel: "inbox" | "outbox" | "system", kind: string, payload: unknown): EventRecord => ({
  session_id: "s",
  seq,
  channel,
  kind,
  payload,
  created_at: seq,
  applied_at: null,
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

  it("tracks token_usage", () => {
    let t = emptyTranscript();
    t = reduceTranscript(t, makeEvt(1, "outbox", "token_usage", { input: 10, output: 20, cache_read: 0, cache_write: 0 }));
    expect(t.tokenUsage?.input).toBe(10);
  });

  it("does NOT append agent_text_delta to events (ephemeral only)", () => {
    let t = emptyTranscript();
    t = reduceTranscript(t, makeEvt(1, "outbox", "agent_text_delta", { message_id: "m", block_id: "b", delta: "x" }));
    expect(t.events.length).toBe(0);
  });
});
