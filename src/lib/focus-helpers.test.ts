import { describe, it, expect } from "vitest";
import { focusForModeSwitch } from "./focus-helpers";

describe("focusForModeSwitch", () => {
  it("preserves project focus across remaining workspace modes", () => {
    const focus = { type: "project" as const, projectId: "p1" };
    expect(focusForModeSwitch(focus, "agents")).toBe(focus);
    expect(focusForModeSwitch(focus, "kanban")).toBe(focus);
    expect(focusForModeSwitch(focus, "chat")).toBe(focus);
  });

  it("collapses chat session focus to project when switching to agents or kanban", () => {
    const focus = { type: "session" as const, sessionId: "s1", projectId: "p1" };
    expect(focusForModeSwitch(focus, "agents")).toEqual({ type: "project", projectId: "p1" });
    expect(focusForModeSwitch(focus, "kanban")).toEqual({ type: "project", projectId: "p1" });
  });

  it("preserves chat session focus when staying in chat", () => {
    const focus = { type: "session" as const, sessionId: "s1", projectId: "p1" };
    expect(focusForModeSwitch(focus, "chat")).toBe(focus);
  });

  it("collapses agent focus to project when switching away from agents", () => {
    const focus = { type: "agent" as const, agentKind: "maintainer" as const, projectId: "p1" };
    expect(focusForModeSwitch(focus, "kanban")).toEqual({ type: "project", projectId: "p1" });
    expect(focusForModeSwitch(focus, "chat")).toEqual({ type: "project", projectId: "p1" });
  });

  it("preserves agent focus when staying in agents", () => {
    const focus = { type: "agent-panel" as const, agentKind: "auto-worker" as const, projectId: "p1" };
    expect(focusForModeSwitch(focus, "agents")).toBe(focus);
  });

  it("returns null when current focus is null", () => {
    expect(focusForModeSwitch(null, "agents")).toBeNull();
  });
});
