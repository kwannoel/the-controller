import { describe, it, expect } from "vitest";
import { render, screen } from "@testing-library/svelte";
import InfrastructureDashboard from "./InfrastructureDashboard.svelte";

describe("InfrastructureDashboard", () => {
  it("renders the empty state", () => {
    render(InfrastructureDashboard);
    expect(screen.getByText("Infrastructure")).toBeTruthy();
    expect(screen.getByText(/no services deployed/i)).toBeTruthy();
  });
});
