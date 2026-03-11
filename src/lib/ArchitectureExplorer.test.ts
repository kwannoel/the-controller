import { describe, expect, it, vi } from "vitest";
import { render, screen, within } from "@testing-library/svelte";
import ArchitectureExplorer from "./ArchitectureExplorer.svelte";
import type { ArchitectureResult } from "./stores";

const architecture: ArchitectureResult = {
  title: "Controller Architecture",
  mermaid: "flowchart TD\nui[UI] --> backend[Backend]\nbackend --> repo[Repository]",
  components: [
    {
      id: "ui",
      name: "UI Shell",
      summary: "Hosts the workspace shell and routes state into focused tools.",
      contains: ["App.svelte", "Sidebar.svelte"],
      incoming_relationships: [
        {
          component_id: "backend",
          summary: "Receives generated architecture payloads from the backend command.",
        },
      ],
      outgoing_relationships: [
        {
          component_id: "backend",
          summary: "Requests project architecture generation and status updates.",
        },
      ],
      evidence_paths: ["src/App.svelte"],
      evidence_snippets: ["{#if workspaceModeState.current === \"architecture\"}"],
    },
    {
      id: "backend",
      name: "Backend Command Layer",
      summary: "Runs architecture analysis and returns normalized component data.",
      contains: ["commands.rs", "architecture.rs"],
      incoming_relationships: [
        {
          component_id: "ui",
          summary: "Receives architecture generation requests from the UI shell.",
        },
      ],
      outgoing_relationships: [
        {
          component_id: "repo",
          summary: "Scans the repository to gather architecture evidence.",
        },
      ],
      evidence_paths: ["src-tauri/src/architecture.rs"],
      evidence_snippets: ["pub struct ArchitectureResult"],
    },
    {
      id: "repo",
      name: "Repository Evidence",
      summary: "Provides the bounded evidence set used to generate the architecture view.",
      contains: ["README.md", "package.json"],
      incoming_relationships: [
        {
          component_id: "backend",
          summary: "Provides source files and metadata for architecture analysis.",
        },
      ],
      outgoing_relationships: [],
      evidence_paths: ["README.md"],
      evidence_snippets: ["# The Controller"],
    },
  ],
};

describe("ArchitectureExplorer", () => {
  it("renders all components and shows the first component details by default", () => {
    render(ArchitectureExplorer, {
      props: {
        architecture,
      },
    });

    const componentList = screen.getByRole("list", { name: "Architecture components" });
    const items = within(componentList).getAllByRole("button");

    expect(items).toHaveLength(architecture.components.length);
    expect(items[0]).toHaveAttribute("aria-pressed", "true");
    expect(items[1]).toHaveAttribute("aria-pressed", "false");
    expect(within(componentList).getByRole("button", { name: "UI Shell" })).toBeInTheDocument();
    expect(within(componentList).getByRole("button", { name: "Backend Command Layer" })).toBeInTheDocument();
    expect(within(componentList).getByRole("button", { name: "Repository Evidence" })).toBeInTheDocument();

    expect(screen.getByRole("heading", { name: "UI Shell" })).toBeInTheDocument();
    expect(
      screen.getByText("Hosts the workspace shell and routes state into focused tools."),
    ).toBeInTheDocument();
    expect(
      screen.getByText("Requests project architecture generation and status updates."),
    ).toBeInTheDocument();
    expect(
      screen.getByText("Receives generated architecture payloads from the backend command."),
    ).toBeInTheDocument();
    expect(screen.getByRole("heading", { name: "Evidence paths" })).toBeInTheDocument();
    expect(screen.getByText("src/App.svelte")).toBeInTheDocument();
    expect(screen.getByRole("heading", { name: "Evidence snippets" })).toBeInTheDocument();
    expect(
      screen.getByText('{#if workspaceModeState.current === "architecture"}'),
    ).toBeInTheDocument();
  });

  it("publishes the first component when the parent has no stored selection", () => {
    const onSelectComponent = vi.fn();

    render(ArchitectureExplorer, {
      props: {
        architecture,
        onSelectComponent,
      },
    });

    expect(onSelectComponent).toHaveBeenCalledWith("ui");
    expect(onSelectComponent).toHaveBeenCalledTimes(1);
  });
});
