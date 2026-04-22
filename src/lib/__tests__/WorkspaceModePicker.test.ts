import { describe, it, expect } from "vitest";
import { render } from "@testing-library/svelte";
import WorkspaceModePicker from "../WorkspaceModePicker.svelte";

describe("WorkspaceModePicker", () => {
  it("lists chat with hotkey c", () => {
    const { getByText } = render(WorkspaceModePicker);
    expect(getByText("Chat")).toBeTruthy();
    const chat = getByText("Chat");
    const row = chat.closest(".picker-option")!;
    expect(row.querySelector("kbd")!.textContent).toBe("c");
  });
});
