import { describe, it, expect } from "vitest";
import { render } from "@testing-library/svelte";
import AgentMessage from "./AgentMessage.svelte";

describe("AgentMessage", () => {
  it("renders inline code as <code>", () => {
    const { container } = render(AgentMessage, { text: "Run `ls`" });
    const code = container.querySelector("code");
    expect(code?.textContent).toBe("ls");
  });

  it("renders plain text without markdown formatting wrapper", () => {
    const { container } = render(AgentMessage, { text: "hello" });
    expect(container.textContent).toContain("hello");
  });
});
