const DIAGRAM_NODE_SELECTOR = "g.node, g.cluster";

export const ARCHITECTURE_NODE_SELECTED_CLASS = "architecture-node-selected";

function escapeAttributeValue(value: string): string {
  return value.replaceAll("\\", "\\\\").replaceAll('"', '\\"');
}

function escapeRegExp(value: string): string {
  return value.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
}

function matchesComponentId(node: SVGGElement, componentId: string): boolean {
  const dataId = node.getAttribute("data-id");
  if (dataId === componentId) {
    return true;
  }

  const id = node.getAttribute("id");
  if (!id) {
    return false;
  }

  if (id === componentId) {
    return true;
  }

  return new RegExp(`(?:^|[-_])${escapeRegExp(componentId)}(?:[-_]|$)`).test(id);
}

function getDiagramNodes(container: ParentNode): SVGGElement[] {
  return Array.from(container.querySelectorAll<SVGGElement>(DIAGRAM_NODE_SELECTOR));
}

export function findArchitectureDiagramNode(
  container: ParentNode,
  componentId: string,
): SVGGElement | null {
  const directMatch = container.querySelector<SVGGElement>(
    `g[data-id="${escapeAttributeValue(componentId)}"]`,
  );
  if (directMatch) {
    return directMatch;
  }

  return getDiagramNodes(container).find((node) => matchesComponentId(node, componentId)) ?? null;
}

export function syncArchitectureDiagramSelection(
  container: ParentNode,
  selectedComponentId: string | null,
): SVGGElement | null {
  const selectedNode = selectedComponentId
    ? findArchitectureDiagramNode(container, selectedComponentId)
    : null;

  for (const node of getDiagramNodes(container)) {
    const isSelected = node === selectedNode;
    node.classList.toggle(ARCHITECTURE_NODE_SELECTED_CLASS, isSelected);
    node.setAttribute("role", "button");
    node.setAttribute("tabindex", "0");
    node.setAttribute("aria-pressed", String(isSelected));
  }

  selectedNode?.scrollIntoView({
    behavior: "smooth",
    block: "nearest",
    inline: "nearest",
  });

  return selectedNode;
}

export function bindArchitectureDiagramInteractions(
  container: HTMLElement,
  onSelectComponent: (componentId: string) => void,
): () => void {
  function findClickedNode(target: EventTarget | null): SVGGElement | null {
    if (!(target instanceof Element)) {
      return null;
    }

    return target.closest<SVGGElement>(DIAGRAM_NODE_SELECTOR);
  }

  function publishSelection(node: SVGGElement | null) {
    const componentId = node?.dataset.id;
    if (componentId) {
      onSelectComponent(componentId);
    }
  }

  function handleClick(event: Event) {
    publishSelection(findClickedNode(event.target));
  }

  function handleKeydown(event: KeyboardEvent) {
    if (event.key !== "Enter" && event.key !== " ") {
      return;
    }

    const node = findClickedNode(event.target);
    if (!node) {
      return;
    }

    event.preventDefault();
    publishSelection(node);
  }

  container.addEventListener("click", handleClick);
  container.addEventListener("keydown", handleKeydown);

  for (const node of getDiagramNodes(container)) {
    node.setAttribute("role", "button");
    node.setAttribute("tabindex", "0");
  }

  return () => {
    container.removeEventListener("click", handleClick);
    container.removeEventListener("keydown", handleKeydown);
  };
}
