import { writable } from "svelte/store";

export interface DeployedService {
  uuid: string;
  name: string;
  subdomain: string;
  projectType: "static" | "node" | "python" | "docker";
  status: "running" | "stopped" | "deploying" | "error";
  cpuPercent: number;
  memoryMb: number;
  uptimeSeconds: number;
  lastDeployedAt: string;
  deployTarget: "coolify" | "cloudflare-pages";
}

export const deployedServices = writable<DeployedService[]>([]);
export const selectedServiceId = writable<string | null>(null);
export const serviceLogLines = writable<string[]>([]);
