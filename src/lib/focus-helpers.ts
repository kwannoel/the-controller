import type { Project, FocusTarget, WorkspaceMode } from "./stores";

/**
 * Compute the focus target after deleting a session.
 * Prefers the session above; falls back to parent project.
 */
export function focusAfterSessionDelete(
  projectList: Project[],
  projectId: string,
  sessionId: string,
): FocusTarget {
  const project = projectList.find(p => p.id === projectId);
  if (!project) return null;
  const sessions = project.sessions.filter(s => !s.auto_worker_session);
  const idx = sessions.findIndex(s => s.id === sessionId);
  if (idx > 0) {
    return { type: "session", sessionId: sessions[idx - 1].id, projectId };
  }
  return { type: "project", projectId };
}

/**
 * Compute the focus target after deleting a project.
 * Prefers the last visible session of the project above (if expanded);
 * falls back to the project above; returns null if topmost.
 */
export function focusAfterProjectDelete(
  projectList: Project[],
  projectId: string,
  expandedProjects: Set<string>,
): FocusTarget {
  const idx = projectList.findIndex(p => p.id === projectId);
  if (idx <= 0) return null;

  const prevProject = projectList[idx - 1];
  if (expandedProjects.has(prevProject.id)) {
    const sessions = prevProject.sessions.filter(s => !s.auto_worker_session);
    if (sessions.length > 0) {
      const lastSession = sessions[sessions.length - 1];
      return { type: "session", sessionId: lastSession.id, projectId: prevProject.id };
    }
  }
  return { type: "project", projectId: prevProject.id };
}

/**
 * Translate focus target when switching workspace modes.
 * Keeps the same project context but changes focus type to match the new mode.
 *
 * - Switching to development: agent/agent-panel → active session (if in same project) or project
 * - Switching to agents: session → project
 */
export function focusForModeSwitch(
  current: FocusTarget,
  newMode: WorkspaceMode,
  activeSessionId: string | null,
  projectList: Project[],
): FocusTarget {
  if (!current) return null;

  if (newMode === "development") {
    if (current.type === "folder" || current.type === "note" || current.type === "notes-editor") {
      if (activeSessionId) {
        const project = projectList.find(p => p.sessions.some(s => s.id === activeSessionId && !s.auto_worker_session));
        if (project) {
          return { type: "session", sessionId: activeSessionId, projectId: project.id };
        }
      }
      return projectList[0] ? { type: "project", projectId: projectList[0].id } : null;
    }
    if (current.type === "agent" || current.type === "agent-panel") {
      if (activeSessionId) {
        const project = projectList.find(p => p.id === current.projectId);
        if (project?.sessions.some(s => s.id === activeSessionId && !s.auto_worker_session)) {
          return { type: "session", sessionId: activeSessionId, projectId: current.projectId };
        }
      }
      return { type: "project", projectId: current.projectId };
    }
  }

  if (newMode === "agents") {
    if (current.type === "folder" || current.type === "note" || current.type === "notes-editor") {
      return projectList[0] ? { type: "project", projectId: projectList[0].id } : null;
    }
    if (current.type === "session") {
      return { type: "project", projectId: current.projectId };
    }
  }

  if (newMode === "notes") {
    if (current.type === "session" || current.type === "agent" || current.type === "agent-panel" || current.type === "project") {
      return null;
    }
  }

  if (newMode === "architecture") {
    if (current.type === "folder" || current.type === "note" || current.type === "notes-editor") {
      return projectList[0] ? { type: "project", projectId: projectList[0].id } : null;
    }
    if (current.type === "session" || current.type === "agent" || current.type === "agent-panel") {
      return { type: "project", projectId: current.projectId };
    }
  }

  if (newMode === "infrastructure") {
    if (current.type === "folder" || current.type === "note" || current.type === "notes-editor") {
      return projectList[0] ? { type: "project", projectId: projectList[0].id } : null;
    }
    if (current.type === "session" || current.type === "agent" || current.type === "agent-panel") {
      return { type: "project", projectId: current.projectId };
    }
  }

  return current;
}
