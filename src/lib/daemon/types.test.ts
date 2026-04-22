import { describe, it, expect } from "vitest";
import fixture from "./__fixtures__/events.json";
import type { EventRecord, OutboxEvent } from "./types";

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
