import { invoke } from "@tauri-apps/api/core";
import { projects, type ProjectInventory } from "./stores";

export async function refreshProjectsFromBackend() {
  const result = await invoke<ProjectInventory>("list_projects");
  projects.set(result.projects);
  return result;
}
