import type { DaemonSession } from "./types";
import type { Project } from "$lib/stores";

export interface SessionGroups {
  byProject: Map<string, DaemonSession[]>;
  other: DaemonSession[];
}

export function groupSessionsByProject(projects: Project[], sessions: DaemonSession[]): SessionGroups {
  const byProject = new Map<string, DaemonSession[]>();
  for (const p of projects) byProject.set(p.id, []);
  const pathToId = new Map(projects.map(p => [p.repo_path, p.id]));
  const other: DaemonSession[] = [];
  for (const s of sessions) {
    const pid = pathToId.get(s.cwd);
    if (pid) byProject.get(pid)!.push(s);
    else other.push(s);
  }
  return { byProject, other };
}
