import { describe, expect, it, vi } from "vitest";
import {
  bindArchitectureDiagramInteractions,
  findArchitectureDiagramNode,
  syncArchitectureDiagramSelection,
} from "./architecture-diagram";

function createDiagram() {
  const container = document.createElement("div");
  container.innerHTML = `
    <svg>
      <g class="node" data-id="ui" id="flowchart-ui-0">
        <rect />
        <text>UI</text>
      </g>
      <g class="node" id="flowchart-backend-1">
        <rect />
        <text>Backend</text>
      </g>
    </svg>
  `;
  return container;
}

describe("architecture-diagram", () => {
  it("finds the rendered node for a component id", () => {
    const container = createDiagram();

    const node = findArchitectureDiagramNode(container, "backend");

    expect(node).not.toBeNull();
    expect(node).toHaveAttribute("id", "flowchart-backend-1");
  });

  it("highlights and scrolls the selected node into view", () => {
    const container = createDiagram();
    const backendNode = findArchitectureDiagramNode(container, "backend")!;
    const uiNode = container.querySelector('g[data-id="ui"]')!;
    const scrollIntoView = vi.fn();
    backendNode.scrollIntoView = scrollIntoView;

    syncArchitectureDiagramSelection(container, "backend");

    expect(backendNode).toHaveClass("architecture-node-selected");
    expect(uiNode).not.toHaveClass("architecture-node-selected");
    expect(scrollIntoView).toHaveBeenCalledOnce();
  });

  it("publishes component selection when a rendered node is clicked", () => {
    const container = createDiagram();
    const onSelectComponent = vi.fn();
    const cleanup = bindArchitectureDiagramInteractions(container, onSelectComponent);

    try {
      findArchitectureDiagramNode(container, "backend")?.dispatchEvent(
        new MouseEvent("click", { bubbles: true }),
      );

      expect(onSelectComponent).toHaveBeenCalledWith("backend");
      expect(onSelectComponent).toHaveBeenCalledTimes(1);
    } finally {
      cleanup();
    }
  });
});
