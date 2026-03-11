<script lang="ts">
  import type { ArchitectureResult } from "./stores";
  import {
    bindArchitectureDiagramInteractions,
    syncArchitectureDiagramSelection,
  } from "./architecture-diagram";

  interface Props {
    architecture?: ArchitectureResult | null;
    projectName?: string;
    selectedComponentId?: string | null;
    onSelectComponent?: (componentId: string) => void;
    onGenerateArchitecture?: () => void;
    isGenerating?: boolean;
    error?: string | null;
  }

  let {
    architecture = null,
    projectName = "Architecture",
    selectedComponentId = null,
    onSelectComponent = () => {},
    onGenerateArchitecture = () => {},
    isGenerating = false,
    error = null,
  }: Props = $props();

  let components = $derived(architecture?.components ?? []);
  let diagramContainer = $state<HTMLDivElement | null>(null);
  let diagramError = $state<string | null>(null);
  let renderedDiagramVersion = $state(0);
  let renderedMermaidSource = $state<string | null>(null);
  let activeDiagramCleanup = () => {};
  const diagramId = `architecture-diagram-${Math.random().toString(36).slice(2)}`;
  let selectedComponent = $derived.by(() => {
    if (components.length === 0) {
      return null;
    }

    if (!selectedComponentId) {
      return components[0];
    }

    return components.find((component) => component.id === selectedComponentId) ?? components[0];
  });
  let resolvedSelectedComponentId = $derived(selectedComponent?.id ?? null);

  $effect(() => {
    if (!resolvedSelectedComponentId) {
      return;
    }

    if (selectedComponentId === resolvedSelectedComponentId) {
      return;
    }

    onSelectComponent(resolvedSelectedComponentId);
  });

  function selectComponent(componentId: string) {
    onSelectComponent(componentId);
  }

  function triggerArchitectureGeneration() {
    onGenerateArchitecture();
  }

  $effect(() => {
    const container = diagramContainer;
    const mermaidSource = architecture?.mermaid ?? null;

    if (!container) {
      return;
    }

    if (
      mermaidSource &&
      renderedMermaidSource === mermaidSource &&
      container.childElementCount > 0
    ) {
      activeDiagramCleanup = bindArchitectureDiagramInteractions(container, onSelectComponent);
      return () => {
        activeDiagramCleanup();
        activeDiagramCleanup = () => {};
      };
    }

    activeDiagramCleanup();
    activeDiagramCleanup = () => {};

    container.innerHTML = "";
    diagramError = null;
    renderedDiagramVersion = 0;
    renderedMermaidSource = null;

    if (!mermaidSource) {
      return;
    }

    let cancelled = false;

    void (async () => {
      try {
        const mermaidModule = await import("mermaid");
        const mermaid = mermaidModule.default;

        mermaid.initialize({
          startOnLoad: false,
          securityLevel: "strict",
          theme: "base",
          flowchart: {
            useMaxWidth: false,
            htmlLabels: false,
          },
          themeVariables: {
            primaryColor: "#1e1e2e",
            primaryTextColor: "#cdd6f4",
            primaryBorderColor: "#45475a",
            lineColor: "#89b4fa",
            tertiaryColor: "#181825",
            clusterBkg: "#11111b",
            clusterBorder: "#45475a",
          },
        });

        const { svg } = await mermaid.render(diagramId, mermaidSource);
        if (cancelled) {
          return;
        }

        container.innerHTML = svg;
        activeDiagramCleanup = bindArchitectureDiagramInteractions(container, onSelectComponent);
        renderedMermaidSource = mermaidSource;
        renderedDiagramVersion += 1;
      } catch (error) {
        if (cancelled) {
          return;
        }

        container.innerHTML = "";
        diagramError =
          error instanceof Error ? error.message : "Failed to render Mermaid diagram.";
      }
    })();

    return () => {
      cancelled = true;
      activeDiagramCleanup();
      activeDiagramCleanup = () => {};
    };
  });

  $effect(() => {
    if (!diagramContainer || renderedDiagramVersion === 0) {
      return;
    }

    syncArchitectureDiagramSelection(diagramContainer, resolvedSelectedComponentId);
  });
</script>

<div class="architecture-explorer">
  <section class="diagram-pane" aria-label="Architecture diagram">
    <div class="pane-header">
      <h2>{architecture?.title ?? projectName}</h2>
      <button
        type="button"
        class="generate-action"
        disabled={isGenerating}
        onclick={triggerArchitectureGeneration}
      >
        {architecture ? "Regenerate" : "Generate"}{isGenerating ? "…" : ""}
      </button>
    </div>

    <div class="diagram-surface" class:is-generating={isGenerating && architecture}>
      {#if architecture}
        <div class="diagram-render" bind:this={diagramContainer}></div>
        {#if diagramError}
          <div class="diagram-error">
            <span class="error-label">Render failed</span>
            <span>{diagramError}</span>
          </div>
        {/if}
      {:else}
        <div class="empty-state">
          <span>No architecture generated yet</span>
          <span class="empty-hint">press <kbd>r</kbd> to generate</span>
        </div>
      {/if}
    </div>
    {#if error}
      <p class="generation-error">{error}</p>
    {/if}
  </section>

  <aside class="inspector-rail">
    <section class="component-list-pane">
      <div class="section-heading">
        <span class="section-title">Components</span>
        <span class="section-count">{components.length}</span>
      </div>

      {#if components.length > 0}
        <ul class="component-list" aria-label="Architecture components">
          {#each components as component (component.id)}
            <li>
              <button
                type="button"
                class:selected={selectedComponent?.id === component.id}
                aria-pressed={selectedComponent?.id === component.id}
                onclick={() => selectComponent(component.id)}
              >
                {component.name}
              </button>
            </li>
          {/each}
        </ul>
      {:else}
        <p class="placeholder-copy">Generate architecture to see components.</p>
      {/if}
    </section>

    <section class="details-pane" aria-label="Component details">
      {#if selectedComponent}
        <h3 class="detail-name">{selectedComponent.name}</h3>
        <p class="summary">{selectedComponent.summary}</p>

        {#if selectedComponent.contains.length > 0}
          <div class="detail-group">
            <h4>Contains</h4>
            <ul>
              {#each selectedComponent.contains as entry}
                <li>{entry}</li>
              {/each}
            </ul>
          </div>
        {/if}

        {#if selectedComponent.outgoing_relationships.length > 0}
          <div class="detail-group">
            <h4>Outgoing</h4>
            <ul>
              {#each selectedComponent.outgoing_relationships as relationship}
                <li>{relationship.summary}</li>
              {/each}
            </ul>
          </div>
        {/if}

        {#if selectedComponent.incoming_relationships.length > 0}
          <div class="detail-group">
            <h4>Incoming</h4>
            <ul>
              {#each selectedComponent.incoming_relationships as relationship}
                <li>{relationship.summary}</li>
              {/each}
            </ul>
          </div>
        {/if}

        {#if selectedComponent.evidence_paths.length > 0}
          <div class="detail-group">
            <h4>Evidence</h4>
            <ul>
              {#each selectedComponent.evidence_paths as evidencePath}
                <li><code>{evidencePath}</code></li>
              {/each}
            </ul>
          </div>
        {/if}

        {#if selectedComponent.evidence_snippets.length > 0}
          <div class="detail-group">
            <h4>Snippets</h4>
            <ul class="evidence-snippets">
              {#each selectedComponent.evidence_snippets as evidenceSnippet}
                <li><pre>{evidenceSnippet}</pre></li>
              {/each}
            </ul>
          </div>
        {/if}
      {:else}
        <div class="empty-state">
          <span>Select a component to inspect</span>
        </div>
      {/if}
    </section>
  </aside>
</div>

<style>
  .architecture-explorer {
    display: grid;
    grid-template-columns: minmax(0, 1fr) 320px;
    height: 100%;
    color: #cdd6f4;
    background: #1e1e2e;
  }

  .diagram-pane,
  .inspector-rail {
    min-height: 0;
  }

  .diagram-pane {
    display: flex;
    flex-direction: column;
    padding: 16px 24px;
    border-right: 1px solid #313244;
    gap: 12px;
  }

  .pane-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
  }

  .pane-header h2 {
    margin: 0;
    font-size: 16px;
    font-weight: 600;
  }

  .generate-action {
    flex-shrink: 0;
    padding: 6px 12px;
    border: none;
    border-radius: 4px;
    background: #313244;
    color: #cdd6f4;
    font: inherit;
    font-size: 12px;
    cursor: pointer;
    transition: background 0.15s;
  }

  .generate-action:hover:enabled {
    background: #45475a;
  }

  .generate-action:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  .generation-error {
    margin: 0;
    font-size: 12px;
    color: #f38ba8;
  }

  .diagram-surface {
    position: relative;
    flex: 1;
    min-height: 0;
    overflow: auto;
    padding: 16px;
    border: 1px solid #313244;
    border-radius: 6px;
    background: #181825;
    transition: opacity 0.15s;
  }

  .diagram-surface.is-generating {
    opacity: 0.5;
  }

  .diagram-render {
    min-width: max-content;
    min-height: 100%;
  }

  .diagram-error {
    position: sticky;
    left: 12px;
    bottom: 12px;
    display: flex;
    gap: 8px;
    margin-top: 12px;
    padding: 8px 12px;
    border: 1px solid rgba(243, 139, 168, 0.3);
    border-radius: 6px;
    background: #1e1e2e;
    font-size: 12px;
  }

  .error-label {
    color: #f38ba8;
    font-weight: 600;
  }

  .diagram-render :global(svg) {
    display: block;
    width: max-content;
    min-width: 100%;
    height: auto;
  }

  .diagram-render :global(g.node),
  .diagram-render :global(g.cluster) {
    cursor: pointer;
    outline: none;
  }

  .diagram-render :global(g.node rect),
  .diagram-render :global(g.node polygon),
  .diagram-render :global(g.node path),
  .diagram-render :global(g.node circle),
  .diagram-render :global(g.node ellipse),
  .diagram-render :global(g.cluster rect),
  .diagram-render :global(g.cluster polygon),
  .diagram-render :global(g.cluster path),
  .diagram-render :global(g.cluster circle),
  .diagram-render :global(g.cluster ellipse) {
    transition:
      stroke 0.15s,
      stroke-width 0.15s;
  }

  .diagram-render :global(g.architecture-node-selected rect),
  .diagram-render :global(g.architecture-node-selected polygon),
  .diagram-render :global(g.architecture-node-selected path),
  .diagram-render :global(g.architecture-node-selected circle),
  .diagram-render :global(g.architecture-node-selected ellipse) {
    stroke: #89b4fa !important;
    stroke-width: 2px !important;
  }

  .diagram-render :global(g.architecture-node-selected .nodeLabel),
  .diagram-render :global(g.architecture-node-selected text) {
    font-weight: 600;
    fill: #89b4fa !important;
  }

  .empty-state {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    height: 100%;
    gap: 8px;
    font-size: 16px;
    font-weight: 500;
  }

  .empty-hint {
    font-size: 13px;
    font-weight: 400;
    color: #6c7086;
  }

  .empty-hint kbd {
    background: #313244;
    color: #89b4fa;
    padding: 1px 6px;
    border-radius: 3px;
    font-family: monospace;
    font-size: 12px;
  }

  .inspector-rail {
    display: grid;
    grid-template-rows: auto minmax(0, 1fr);
    background: #11111b;
  }

  .component-list-pane,
  .details-pane {
    min-height: 0;
    overflow: auto;
    padding: 12px 16px;
  }

  .component-list-pane {
    border-bottom: 1px solid #313244;
  }

  .section-heading {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin-bottom: 8px;
  }

  .section-title {
    font-size: 13px;
    font-weight: 600;
  }

  .section-count {
    font-size: 11px;
    color: #6c7086;
    background: #313244;
    padding: 1px 6px;
    border-radius: 3px;
  }

  .placeholder-copy {
    font-size: 12px;
    color: #6c7086;
  }

  .component-list {
    display: flex;
    flex-direction: column;
    gap: 2px;
    margin: 0;
    padding: 0;
    list-style: none;
  }

  .component-list button {
    width: 100%;
    border: none;
    border-radius: 4px;
    padding: 8px 12px;
    background: transparent;
    color: #a6adc8;
    font: inherit;
    font-size: 12px;
    text-align: left;
    cursor: pointer;
    transition: color 0.15s, background 0.15s;
  }

  .component-list button:hover {
    background: rgba(49, 50, 68, 0.5);
    color: #cdd6f4;
  }

  .component-list button.selected {
    background: rgba(137, 180, 250, 0.1);
    color: #cdd6f4;
    outline: 1px solid rgba(137, 180, 250, 0.4);
    outline-offset: -1px;
  }

  .detail-name {
    margin: 0 0 4px;
    font-size: 15px;
    font-weight: 600;
  }

  .summary {
    margin: 0 0 12px;
    font-size: 12px;
    color: #bac2de;
    line-height: 1.5;
  }

  .detail-group {
    margin-bottom: 12px;
  }

  .detail-group h4 {
    margin: 0 0 4px;
    font-size: 11px;
    font-weight: 600;
    color: #6c7086;
    text-transform: uppercase;
  }

  .detail-group ul {
    margin: 0;
    padding-left: 16px;
    font-size: 12px;
  }

  .detail-group li + li {
    margin-top: 4px;
  }

  .detail-group code {
    font-family: "SFMono-Regular", "SF Mono", Consolas, "Liberation Mono", Menlo, monospace;
    font-size: 11px;
    color: #a6adc8;
  }

  .evidence-snippets {
    list-style: none;
    padding-left: 0;
  }

  .evidence-snippets pre {
    margin: 0;
    white-space: pre-wrap;
    border: 1px solid rgba(49, 50, 68, 0.5);
    border-radius: 6px;
    padding: 8px 12px;
    background: #1e1e2e;
    color: #cdd6f4;
    font-family: "SFMono-Regular", "SF Mono", Consolas, "Liberation Mono", Menlo, monospace;
    font-size: 11px;
  }

  .evidence-snippets li + li {
    margin-top: 6px;
  }

  .details-pane .empty-state {
    font-size: 13px;
    color: #6c7086;
  }

  @media (max-width: 980px) {
    .architecture-explorer {
      grid-template-columns: 1fr;
      grid-template-rows: minmax(280px, 1fr) minmax(0, 1fr);
    }

    .diagram-pane {
      border-right: 0;
      border-bottom: 1px solid #313244;
    }
  }
</style>
