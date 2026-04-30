import { describe, it, expect } from "vitest";
import { render } from "@testing-library/svelte";
import WorkspaceModePicker from "../WorkspaceModePicker.svelte";

describe("WorkspaceModePicker", () => {
  it("lists the available workspace modes with compact keys", () => {
    const { getByText, queryByText, container } = render(WorkspaceModePicker);
    expect(queryByText("Development")).toBeNull();
    expect(container.querySelectorAll(".picker-option")).toHaveLength(5);

    expect(getByText("Agents")).toBeTruthy();
    expect(getByText("Kanban")).toBeTruthy();
    expect(getByText("Chat")).toBeTruthy();
    expect(getByText("Create")).toBeTruthy();
    expect(getByText("Observe")).toBeTruthy();

    const chat = getByText("Chat");
    const row = chat.closest(".picker-option")!;
    expect(row.querySelector("kbd")!.textContent).toBe("c");

    const create = getByText("Create");
    const createRow = create.closest(".picker-option")!;
    expect(createRow.querySelector("kbd")!.textContent).toBe("p");
  });
});
