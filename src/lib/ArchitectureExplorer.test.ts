import { fireEvent, render, screen, waitFor, within } from "@testing-library/svelte";
import { afterAll, beforeAll, describe, expect, it, vi } from "vitest";
import ArchitectureExplorer from "./ArchitectureExplorer.svelte";
import { findArchitectureDiagramNode } from "./architecture-diagram";
import type { ArchitectureResult } from "./stores";

const originalGetBBox = SVGElement.prototype.getBBox;
const originalGetComputedTextLength = SVGElement.prototype.getComputedTextLength;

beforeAll(() => {
  SVGElement.prototype.getBBox = function getBBox() {
    return {
      x: 0,
      y: 0,
      width: 160,
      height: 48,
    };
  };

  SVGElement.prototype.getComputedTextLength = function getComputedTextLength() {
    return 160;
  };
});

afterAll(() => {
  SVGElement.prototype.getBBox = originalGetBBox;
  SVGElement.prototype.getComputedTextLength = originalGetComputedTextLength;
});

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

  it("syncs list and diagram selection through the shared component id", async () => {
    const onSelectComponent = vi.fn();
    const scrollIntoView = vi.fn();
    const originalScrollIntoView = Element.prototype.scrollIntoView;
    Element.prototype.scrollIntoView = scrollIntoView;

    try {
      const view = render(ArchitectureExplorer, {
        props: {
          architecture,
          selectedComponentId: "ui",
          onSelectComponent,
        },
      });

      await waitFor(() => {
        expect(findArchitectureDiagramNode(document.body, "ui")).toBeInTheDocument();
      });

      await fireEvent.click(screen.getByRole("button", { name: "Backend Command Layer" }));
      expect(onSelectComponent).toHaveBeenCalledWith("backend");

      await view.rerender({
        architecture,
        selectedComponentId: "backend",
        onSelectComponent,
      });

      await waitFor(() => {
        expect(screen.getByRole("heading", { name: "Backend Command Layer" })).toBeInTheDocument();
      });

      await waitFor(() => {
        expect(findArchitectureDiagramNode(document.body, "backend")).toHaveClass(
          "architecture-node-selected",
        );
      });

      expect(findArchitectureDiagramNode(document.body, "backend")).toHaveClass(
        "architecture-node-selected",
      );
      expect(findArchitectureDiagramNode(document.body, "ui")).not.toHaveClass(
        "architecture-node-selected",
      );
      expect(scrollIntoView).toHaveBeenCalled();

      await fireEvent.click(findArchitectureDiagramNode(document.body, "backend")!);
      expect(onSelectComponent).toHaveBeenCalledWith("backend");
    } finally {
      Element.prototype.scrollIntoView = originalScrollIntoView;
    }
  });
});
