import { describe, it, expect, beforeEach } from "vitest";
import { render, screen } from "@testing-library/svelte";
import { deployedServices, selectedServiceId } from "./deploy-stores";
import InfrastructureDashboard from "./InfrastructureDashboard.svelte";

beforeEach(() => {
  deployedServices.set([]);
  selectedServiceId.set(null);
});

describe("InfrastructureDashboard", () => {
  it("renders empty state when no services deployed", () => {
    render(InfrastructureDashboard);
    expect(screen.getByText(/no services deployed/i)).toBeTruthy();
  });

  it("renders service cards when services exist", () => {
    deployedServices.set([
      {
        uuid: "svc-1",
        name: "myapp",
        subdomain: "myapp.example.com",
        projectType: "node",
        status: "running",
        cpuPercent: 3,
        memoryMb: 128,
        uptimeSeconds: 86400,
        lastDeployedAt: "2026-03-11T00:00:00Z",
        deployTarget: "coolify",
      },
    ]);
    render(InfrastructureDashboard);
    expect(screen.getByText("myapp")).toBeTruthy();
    expect(screen.getByText(/running/i)).toBeTruthy();
  });
});
