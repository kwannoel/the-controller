<script lang="ts">
  import type { ArchitectureResult } from "./stores";

  interface Props {
    architecture?: ArchitectureResult | null;
    projectName?: string;
    selectedComponentId?: string | null;
    onSelectComponent?: (componentId: string) => void;
  }

  let {
    architecture = null,
    projectName = "Architecture",
    selectedComponentId = null,
    onSelectComponent = () => {},
  }: Props = $props();

  let components = $derived(architecture?.components ?? []);
  let selectedComponent = $derived.by(() => {
    if (components.length === 0) {
      return null;
    }

    if (!selectedComponentId) {
      return components[0];
    }

    return components.find((component) => component.id === selectedComponentId) ?? components[0];
  });

  function selectComponent(componentId: string) {
    onSelectComponent(componentId);
  }
</script>

<div class="architecture-explorer">
  <section class="diagram-pane" aria-label="Architecture diagram">
    <div class="pane-header">
      <p class="eyebrow">Architecture</p>
      <h1>{architecture?.title ?? projectName}</h1>
      <p class="note">Generated as a high-level systems view, not an exhaustive code index.</p>
    </div>

    <div class="diagram-surface">
      {#if architecture}
        <pre>{architecture.mermaid}</pre>
      {:else}
        <div class="empty-state">
          <h2>No architecture generated yet</h2>
          <p>{projectName} does not have a cached architecture view yet.</p>
        </div>
      {/if}
    </div>
  </section>

  <aside class="inspector-rail">
    <section class="component-list-pane">
      <div class="section-heading">
        <h2>Components</h2>
        <span>{components.length}</span>
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
        <p class="placeholder-copy">Generate architecture to inspect components and relationships.</p>
      {/if}
    </section>

    <section class="details-pane" aria-label="Component details">
      {#if selectedComponent}
        <h2>{selectedComponent.name}</h2>
        <p class="summary">{selectedComponent.summary}</p>

        <div class="detail-group">
          <h3>Contains</h3>
          {#if selectedComponent.contains.length > 0}
            <ul>
              {#each selectedComponent.contains as entry}
                <li>{entry}</li>
              {/each}
            </ul>
          {:else}
            <p>No nested components listed.</p>
          {/if}
        </div>

        <div class="detail-group">
          <h3>Outgoing relationships</h3>
          {#if selectedComponent.outgoing_relationships.length > 0}
            <ul>
              {#each selectedComponent.outgoing_relationships as relationship}
                <li>{relationship.summary}</li>
              {/each}
            </ul>
          {:else}
            <p>No outgoing relationships.</p>
          {/if}
        </div>

        <div class="detail-group">
          <h3>Incoming relationships</h3>
          {#if selectedComponent.incoming_relationships.length > 0}
            <ul>
              {#each selectedComponent.incoming_relationships as relationship}
                <li>{relationship.summary}</li>
              {/each}
            </ul>
          {:else}
            <p>No incoming relationships.</p>
          {/if}
        </div>
      {:else}
        <div class="empty-details">
          <h2>Component details</h2>
          <p>Select a generated component to inspect its summary and relationships.</p>
        </div>
      {/if}
    </section>
  </aside>
</div>

<style>
  .architecture-explorer {
    display: grid;
    grid-template-columns: minmax(0, 1.6fr) minmax(320px, 0.9fr);
    height: 100%;
    color: #cdd6f4;
    background:
      radial-gradient(circle at top left, rgba(137, 180, 250, 0.12), transparent 28%),
      linear-gradient(180deg, #11111b 0%, #181825 100%);
  }

  .diagram-pane,
  .inspector-rail {
    min-height: 0;
  }

  .diagram-pane {
    display: flex;
    flex-direction: column;
    padding: 1.5rem;
    border-right: 1px solid rgba(205, 214, 244, 0.1);
    gap: 1rem;
  }

  .pane-header h1,
  .section-heading h2,
  .details-pane h2 {
    margin: 0;
  }

  .eyebrow {
    margin: 0 0 0.35rem;
    font-size: 0.78rem;
    text-transform: uppercase;
    letter-spacing: 0.14em;
    color: #89b4fa;
  }

  .note,
  .placeholder-copy,
  .summary,
  .empty-state p,
  .empty-details p {
    color: #bac2de;
  }

  .diagram-surface {
    flex: 1;
    min-height: 0;
    overflow: auto;
    padding: 1.25rem;
    border: 1px solid rgba(205, 214, 244, 0.1);
    border-radius: 18px;
    background: rgba(24, 24, 37, 0.88);
    box-shadow: inset 0 1px 0 rgba(255, 255, 255, 0.03);
  }

  .diagram-surface pre {
    margin: 0;
    white-space: pre-wrap;
    font-size: 0.92rem;
    line-height: 1.55;
    color: #f5e0dc;
  }

  .empty-state,
  .empty-details {
    display: grid;
    place-items: start;
    gap: 0.5rem;
  }

  .empty-state h2,
  .empty-details h2,
  .detail-group h3 {
    margin: 0;
  }

  .inspector-rail {
    display: grid;
    grid-template-rows: minmax(0, 0.9fr) minmax(0, 1.1fr);
    background: rgba(17, 17, 27, 0.9);
  }

  .component-list-pane,
  .details-pane {
    min-height: 0;
    overflow: auto;
    padding: 1.25rem;
  }

  .component-list-pane {
    border-bottom: 1px solid rgba(205, 214, 244, 0.1);
  }

  .section-heading {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin-bottom: 0.9rem;
  }

  .section-heading span {
    color: #89dceb;
    font-size: 0.9rem;
  }

  .component-list {
    display: grid;
    gap: 0.65rem;
    margin: 0;
    padding: 0;
    list-style: none;
  }

  .component-list button {
    width: 100%;
    border: 1px solid rgba(205, 214, 244, 0.09);
    border-radius: 14px;
    padding: 0.8rem 0.95rem;
    background: rgba(30, 30, 46, 0.9);
    color: inherit;
    text-align: left;
    cursor: pointer;
    transition:
      border-color 120ms ease,
      transform 120ms ease,
      background 120ms ease;
  }

  .component-list button:hover {
    border-color: rgba(137, 180, 250, 0.45);
    transform: translateY(-1px);
  }

  .component-list button.selected {
    border-color: #89b4fa;
    background: rgba(49, 50, 68, 0.96);
    box-shadow: 0 0 0 1px rgba(137, 180, 250, 0.18);
  }

  .detail-group + .detail-group {
    margin-top: 1.25rem;
  }

  .detail-group ul {
    margin: 0.55rem 0 0;
    padding-left: 1.1rem;
  }

  .detail-group li + li {
    margin-top: 0.35rem;
  }

  @media (max-width: 980px) {
    .architecture-explorer {
      grid-template-columns: 1fr;
      grid-template-rows: minmax(280px, 1fr) minmax(0, 1fr);
    }

    .diagram-pane {
      border-right: 0;
      border-bottom: 1px solid rgba(205, 214, 244, 0.1);
    }

    .inspector-rail {
      grid-template-rows: minmax(200px, 0.8fr) minmax(0, 1fr);
    }
  }
</style>
