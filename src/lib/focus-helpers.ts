import type { FocusTarget, WorkspaceMode } from "./stores";

/**
 * Translate focus target when switching workspace modes.
 * Keeps the same project context and collapses mode-specific children when
 * moving into a mode that cannot render them.
 */
export function focusForModeSwitch(
  current: FocusTarget,
  newMode: WorkspaceMode,
): FocusTarget {
  if (!current) return null;

  if (current.type === "session" && newMode !== "chat") {
    return { type: "project", projectId: current.projectId };
  }

  if ((current.type === "agent" || current.type === "agent-panel") && newMode !== "agents") {
    return { type: "project", projectId: current.projectId };
  }

  return current;
}
