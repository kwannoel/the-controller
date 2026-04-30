import { describe, expect, it } from "vitest";
import { validateProfileDraft, type ProfileDraft } from "./profile-validation";

describe("validateProfileDraft", () => {
  it("blocks unsupported handles and empty prompts while warning for optional launch context", () => {
    const draft: ProfileDraft = {
      name: "Reviewer",
      handle: "Reviewer!",
      runtime: "codex",
      model: "",
      description: "",
      prompt: "",
      skills: [],
      outbox_instructions: "",
      default_workspace_behavior: "focused",
    };

    const result = validateProfileDraft(draft);

    expect(result.blocking).toEqual([
      { field: "handle", message: "Handle can use lowercase letters, numbers, and hyphens only." },
      { field: "prompt", message: "System prompt is required." },
    ]);
    expect(result.warnings).toEqual([
      { field: "skills", message: "No skills selected." },
      { field: "outbox_instructions", message: "Outbox instructions are empty." },
    ]);
  });
});
