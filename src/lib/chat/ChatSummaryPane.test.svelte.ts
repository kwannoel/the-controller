import { describe, expect, it } from "vitest";
import { render, screen } from "@testing-library/svelte";
import ChatSummaryPane from "./ChatSummaryPane.svelte";

describe("ChatSummaryPane", () => {
  it("shows linked agents, focused agent, workspaces, and focused workspace", () => {
    render(ChatSummaryPane, {
      agents: [
        { id: "agent-1", handle: "reviewer", name: "Reviewer", focused: true },
        { id: "agent-2", handle: "debugger", name: "Debugger", focused: false },
      ],
      workspaces: [
        { id: "workspace-1", label: "Controller", path: "/repo/controller", focused: false },
        { id: "workspace-2", label: "Docs", path: "/repo/docs", focused: true },
      ],
    });

    expect(screen.getByText("agents")).toBeTruthy();
    expect(screen.getByText("@reviewer")).toBeTruthy();
    expect(screen.getByText("@debugger")).toBeTruthy();
    expect(screen.getByText("focused @reviewer")).toBeTruthy();
    expect(screen.getByText("workspaces")).toBeTruthy();
    expect(screen.getByText("Controller")).toBeTruthy();
    expect(screen.getByText("Docs")).toBeTruthy();
    expect(screen.getByText("focused Docs")).toBeTruthy();
  });

  it("renders a quiet none state when nothing is known locally", () => {
    render(ChatSummaryPane, { agents: [], workspaces: [] });

    expect(screen.getByText("none")).toBeTruthy();
  });
});
