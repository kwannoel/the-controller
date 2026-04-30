import type { Agent } from "../daemon/types";

export type ProfileDraftField =
  | "name"
  | "handle"
  | "runtime"
  | "model"
  | "description"
  | "prompt"
  | "skills"
  | "outbox_instructions"
  | "default_workspace_behavior";

export interface ProfileDraft {
  id?: string;
  name: string;
  handle: string;
  runtime: Agent | "";
  model: string;
  description: string;
  prompt: string;
  skills: string[];
  outbox_instructions: string;
  default_workspace_behavior: string;
}

export interface ProfileValidationMessage {
  field: ProfileDraftField;
  message: string;
}

export interface ProfileValidationResult {
  blocking: ProfileValidationMessage[];
  warnings: ProfileValidationMessage[];
}

const HANDLE_PATTERN = /^[a-z0-9-]+$/;

export function validateProfileDraft(draft: ProfileDraft): ProfileValidationResult {
  const blocking: ProfileValidationMessage[] = [];
  const warnings: ProfileValidationMessage[] = [];

  if (!draft.name.trim()) {
    blocking.push({ field: "name", message: "Name is required." });
  }
  if (!draft.handle.trim()) {
    blocking.push({ field: "handle", message: "Handle is required." });
  } else if (!HANDLE_PATTERN.test(draft.handle.trim())) {
    blocking.push({ field: "handle", message: "Handle can use lowercase letters, numbers, and hyphens only." });
  }
  if (!draft.runtime) {
    blocking.push({ field: "runtime", message: "Runtime is required." });
  }
  if (!draft.prompt.trim()) {
    blocking.push({ field: "prompt", message: "System prompt is required." });
  }

  if (draft.skills.length === 0) {
    warnings.push({ field: "skills", message: "No skills selected." });
  }
  if (!draft.outbox_instructions.trim()) {
    warnings.push({ field: "outbox_instructions", message: "Outbox instructions are empty." });
  }
  if (!draft.default_workspace_behavior.trim()) {
    warnings.push({ field: "default_workspace_behavior", message: "Workspace behavior is unset." });
  }

  return { blocking, warnings };
}
