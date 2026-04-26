import { describe, it, expect } from "vitest";
import { render } from "@testing-library/svelte";
import WorkspaceModePicker from "../WorkspaceModePicker.svelte";

describe("WorkspaceModePicker", () => {
  it("lists only agents, kanban, and chat", () => {
    const { getByText, queryByText, container } = render(WorkspaceModePicker);
    expect(queryByText("Development")).toBeNull();
    expect(container.querySelectorAll(".picker-option")).toHaveLength(3);

    expect(getByText("Agents")).toBeTruthy();
    expect(getByText("Kanban")).toBeTruthy();
    expect(getByText("Chat")).toBeTruthy();

    const chat = getByText("Chat");
    const row = chat.closest(".picker-option")!;
    expect(row.querySelector("kbd")!.textContent).toBe("c");
  });
});
