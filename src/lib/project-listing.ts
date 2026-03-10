import { command } from "$lib/backend";
import { projects, type ProjectInventory } from "./stores";

export async function refreshProjectsFromBackend() {
  const result = await command<ProjectInventory>("list_projects");
  projects.set(result.projects);
  return result;
}
