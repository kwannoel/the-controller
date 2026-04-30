<script lang="ts">
  import type { ProfileDraft, ProfileValidationResult } from "./profile-validation";

  interface Props {
    draft: ProfileDraft;
    validation: ProfileValidationResult;
    open: boolean;
    onClose: () => void;
  }

  let { draft, validation, open, onClose }: Props = $props();

  let generatedContext = $derived(`# System Prompt (@${draft.handle || "agent"})

${draft.prompt || "No system prompt configured."}

## Workspace Behavior
${draft.default_workspace_behavior || "unset"}

## Outbox Contract
${draft.outbox_instructions || "No outbox instructions configured."}

## Skills
${draft.skills.length > 0 ? draft.skills.join(", ") : "none selected"}`);
</script>

{#if open}
  <aside class="preview-drawer" aria-label="Generated context preview">
    <header>
      <div>
        <p class="eyebrow">Generated Context</p>
        <h2>@{draft.handle || "agent"}</h2>
      </div>
      <button type="button" onclick={onClose} aria-label="Collapse preview">×</button>
    </header>

    <section class="preview-section">
      <h3>Runtime Context</h3>
      <pre>{generatedContext}</pre>
    </section>

    <section class="preview-section">
      <h3>Selected Skills</h3>
      <div class="chips">
        {#if draft.skills.length === 0}
          <span class="muted">none selected</span>
        {:else}
          {#each draft.skills as skill}
            <span class="chip">{skill}</span>
          {/each}
        {/if}
      </div>
    </section>

    <section class="preview-section compact">
      <h3>Validation</h3>
      {#if validation.blocking.length === 0 && validation.warnings.length === 0}
        <p class="valid">Ready to save.</p>
      {:else}
        {#each validation.blocking as item}
          <p class="blocking">{item.message}</p>
        {/each}
        {#each validation.warnings as item}
          <p class="warning">{item.message}</p>
        {/each}
      {/if}
    </section>
  </aside>
{/if}

<style>
  .preview-drawer {
    width: 360px;
    min-width: 320px;
    height: 100%;
    display: flex;
    flex-direction: column;
    gap: 14px;
    border-left: 1px solid var(--border-default);
    background: var(--bg-base);
    padding: 18px;
    overflow-y: auto;
  }

  header {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: 12px;
    padding-bottom: 14px;
    border-bottom: 1px solid var(--border-default);
  }

  .eyebrow {
    color: var(--text-secondary);
    font: 11px var(--font-mono);
    text-transform: uppercase;
  }

  h2 {
    color: var(--text-primary);
    font: 600 18px var(--font-mono);
  }

  h3 {
    color: var(--text-primary);
    font-size: 13px;
    font-weight: 600;
  }

  button {
    width: 30px;
    height: 30px;
    border: 1px solid var(--border-default);
    border-radius: 6px;
    background: var(--bg-surface);
    color: var(--text-primary);
    cursor: pointer;
  }

  .preview-section {
    display: grid;
    gap: 10px;
  }

  pre {
    white-space: pre-wrap;
    overflow-wrap: anywhere;
    border: 1px solid var(--border-default);
    border-radius: 6px;
    background: var(--bg-void);
    color: var(--text-primary);
    padding: 12px;
    font: 12px/1.55 var(--font-mono);
  }

  .chips {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }

  .chip {
    border: 1px solid var(--border-default);
    border-radius: 999px;
    color: var(--text-primary);
    padding: 4px 8px;
    font: 12px var(--font-mono);
  }

  .muted,
  .warning,
  .blocking,
  .valid {
    font: 12px/1.4 var(--font-mono);
  }

  .muted {
    color: var(--text-secondary);
  }

  .warning {
    color: var(--status-working);
  }

  .blocking {
    color: var(--status-error);
  }

  .valid {
    color: var(--status-idle);
  }

  @media (max-width: 1060px) {
    .preview-drawer {
      position: absolute;
      inset: 0 0 0 auto;
      z-index: 10;
      box-shadow: -24px 0 48px rgba(0, 0, 0, 0.45);
    }
  }

  @media (max-width: 720px) {
    .preview-drawer {
      width: 100%;
      min-width: 0;
    }
  }
</style>
