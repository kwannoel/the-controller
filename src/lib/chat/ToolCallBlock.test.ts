import { describe, it, expect } from "vitest";
import { render, fireEvent } from "@testing-library/svelte";
import ToolCallBlock from "./ToolCallBlock.svelte";
import type { EventRecord } from "../daemon/types";

const makeCall = (): EventRecord => ({
  session_id: "s",
  seq: 1,
  channel: "outbox",
  kind: "tool_call",
  payload: { call_id: "c1", tool: "Bash", input: { command: "ls" } },
  created_at: 1,
  applied_at: null,
});
const makeResult = (isError = false): EventRecord => ({
  session_id: "s",
  seq: 2,
  channel: "outbox",
  kind: "tool_result",
  payload: { call_id: "c1", output: "file\n", is_error: isError },
  created_at: 2,
  applied_at: null,
});

describe("ToolCallBlock", () => {
  it("renders collapsed by default with tool name visible", () => {
    const { getByText, queryByText } = render(ToolCallBlock, { call: makeCall(), result: null });
    expect(getByText(/Bash/)).toBeTruthy();
    // Output not rendered yet
    expect(queryByText(/file/)).toBeNull();
  });

  it("click toggles expanded; tool_result output renders", async () => {
    const { getByRole, getByText } = render(ToolCallBlock, { call: makeCall(), result: makeResult(false) });
    await fireEvent.click(getByRole("button"));
    expect(getByText(/file/)).toBeTruthy();
  });

  it("applies error class when is_error=true", async () => {
    const { container, getByRole } = render(ToolCallBlock, { call: makeCall(), result: makeResult(true) });
    await fireEvent.click(getByRole("button"));
    const err = container.querySelector(".tool-error");
    expect(err).toBeTruthy();
  });

  it("when no result, expanded body shows input only", async () => {
    const { container, getByRole } = render(ToolCallBlock, { call: makeCall(), result: null });
    await fireEvent.click(getByRole("button"));
    expect(container.textContent).toContain("ls");
  });
});
