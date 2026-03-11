import { command } from "$lib/backend";

export type ProjectType = "static" | "node" | "python" | "docker" | "unknown";

export interface ProjectSignals {
  has_dockerfile: boolean;
  has_package_json: boolean;
  has_vite_config: boolean;
  has_start_script: boolean;
  has_pyproject: boolean;
}

export function classifyProject(signals: ProjectSignals): ProjectType {
  if (signals.has_vite_config && !signals.has_start_script) return "static";
  if (signals.has_dockerfile) return "docker";
  if (signals.has_package_json && signals.has_start_script) return "node";
  if (signals.has_pyproject) return "python";
  return "unknown";
}

export async function detectProjectType(repoPath: string): Promise<ProjectType> {
  const signals = await command<ProjectSignals>("detect_project_type", { repoPath });
  return classifyProject(signals);
}
