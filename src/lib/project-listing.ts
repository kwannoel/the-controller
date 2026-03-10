import { invoke } from "@tauri-apps/api/core";
import { corruptProjectEntries, projects, type ProjectInventory } from "./stores";

export async function refreshProjectsFromBackend() {
  const result = await invoke<ProjectInventory>("list_projects");
  projects.set(result.projects);
  corruptProjectEntries.set(result.corrupt_entries);
  return result;
}
