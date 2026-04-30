<script lang="ts">
  import type { ProfileDraft, ProfileValidationResult } from "./profile-validation";

  interface Props {
    draft: ProfileDraft;
    validation: ProfileValidationResult;
    dirty: boolean;
    saved: boolean;
    archived: boolean;
    previewOpen: boolean;
    canSave: boolean;
    canTest: boolean;
    clientAvailable: boolean;
    saving: boolean;
    actionError: string | null;
    onDraftChange: (patch: Partial<ProfileDraft>) => void;
    onSave: () => void;
    onDuplicate: () => void;
    onArchive: () => void;
    onRestore: () => void;
    onTest: () => void;
    onTogglePreview: () => void;
  }

  let {
    draft,
    validation,
    dirty,
    saved,
    archived,
    previewOpen,
    canSave,
    canTest,
    clientAvailable,
    saving,
    actionError,
    onDraftChange,
    onSave,
    onDuplicate,
    onArchive,
    onRestore,
    onTest,
    onTogglePreview,
  }: Props = $props();

  function initials(name: string): string {
    const words = name.trim().split(/\s+/).filter(Boolean);
    if (words.length === 0) return "?";
    return words.slice(0, 2).map((word) => word[0]?.toUpperCase()).join("");
  }

  function setField(field: keyof ProfileDraft, event: Event) {
    const value = (event.currentTarget as HTMLInputElement | HTMLTextAreaElement | HTMLSelectElement).value;
    onDraftChange({ [field]: value } as Partial<ProfileDraft>);
  }

  function setSkills(event: Event) {
    const value = (event.currentTarget as HTMLInputElement).value;
    onDraftChange({
      skills: value.split(",").map((skill) => skill.trim()).filter(Boolean),
    });
  }
</script>

<section class="editor-pane" aria-label="Agent profile editor">
  <header class="editor-header">
    <div class="identity">
      <div class="avatar">{initials(draft.name || draft.handle)}</div>
      <div class="title-block">
        <h1>{draft.name || "Unsaved draft"}</h1>
        <div class="meta-row">
          <span>@{draft.handle || "handle"}</span>
          <span>{draft.runtime || "runtime"}</span>
          {#if dirty}
            <span class="dirty">Unsaved draft</span>
          {:else if saved}
            <span class="clean">Saved</span>
          {/if}
        </div>
      </div>
    </div>

    <div class="actions">
      <button type="button" class="secondary" onclick={onTogglePreview}>{previewOpen ? "Hide Preview" : "Preview"}</button>
      <button type="button" class="secondary" onclick={onDuplicate} disabled={!saved}>Duplicate</button>
      {#if archived}
        <button type="button" class="secondary" onclick={onRestore} disabled={!clientAvailable}>Restore</button>
      {:else}
        <button type="button" class="secondary" onclick={onArchive} disabled={!saved || !clientAvailable}>Archive</button>
      {/if}
      <button type="button" class="secondary" onclick={onTest} disabled={!canTest}>Test in Chat</button>
      <button type="button" class="primary" onclick={onSave} disabled={!canSave}>{saving ? "Saving" : "Save"}</button>
    </div>
  </header>

  {#if !clientAvailable}
    <div class="connection-state">Daemon client unavailable. Profile actions are disabled.</div>
  {/if}
  {#if actionError}
    <div class="connection-state error">{actionError}</div>
  {/if}

  <div class="validation-strip">
    {#if validation.blocking.length === 0}
      <span class="valid">handle: valid</span>
    {:else}
      {#each validation.blocking as item}
        <span class="invalid">{item.message}</span>
      {/each}
    {/if}
    {#each validation.warnings as item}
      <span class="warn">{item.message}</span>
    {/each}
  </div>

  <form class="editor-form" onsubmit={(event) => { event.preventDefault(); if (canSave) onSave(); }}>
    <div class="field-grid">
      <label>
        <span>Name</span>
        <input aria-label="Name" value={draft.name} oninput={(event) => setField("name", event)} placeholder="Reviewer" />
      </label>
      <label>
        <span>Handle</span>
        <input aria-label="Handle" value={draft.handle} oninput={(event) => setField("handle", event)} placeholder="reviewer" />
      </label>
      <label>
        <span>Runtime</span>
        <select aria-label="Runtime" value={draft.runtime} onchange={(event) => setField("runtime", event)}>
          <option value="codex">codex</option>
          <option value="claude">claude</option>
        </select>
      </label>
      <label>
        <span>Model</span>
        <input aria-label="Model" value={draft.model} oninput={(event) => setField("model", event)} placeholder="gpt-5" />
      </label>
    </div>

    <label>
      <span>Description</span>
      <input aria-label="Description" value={draft.description} oninput={(event) => setField("description", event)} placeholder="Short role summary" />
    </label>

    <label>
      <span>System Prompt</span>
      <textarea aria-label="System Prompt" class="prompt" value={draft.prompt} oninput={(event) => setField("prompt", event)} placeholder="Write the agent instructions..."></textarea>
    </label>

    <div class="field-grid lower">
      <label>
        <span>Skills</span>
        <input aria-label="Skills" value={draft.skills.join(", ")} oninput={setSkills} placeholder="code-review, security" />
      </label>
      <label>
        <span>Workspace Behavior</span>
        <select aria-label="Workspace Behavior" value={draft.default_workspace_behavior} onchange={(event) => setField("default_workspace_behavior", event)}>
          <option value="">unset</option>
          <option value="focused">focused</option>
          <option value="read-only">read-only</option>
          <option value="read-write-initial">read-write-initial</option>
        </select>
      </label>
    </div>

    <label>
      <span>Outbox Instructions</span>
      <textarea aria-label="Outbox Instructions" value={draft.outbox_instructions} oninput={(event) => setField("outbox_instructions", event)} placeholder="How this agent should reply in chat..."></textarea>
    </label>
  </form>
</section>

<style>
  .editor-pane {
    min-width: 0;
    flex: 1;
    height: 100%;
    display: flex;
    flex-direction: column;
    background: var(--bg-void);
    overflow: hidden;
  }

  .editor-header {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: 16px;
    padding: 18px 20px;
    border-bottom: 1px solid var(--border-default);
  }

  .identity {
    min-width: 0;
    display: flex;
    align-items: center;
    gap: 12px;
  }

  .avatar {
    width: 44px;
    height: 44px;
    display: grid;
    place-items: center;
    flex: 0 0 auto;
    border: 1px solid var(--border-default);
    border-radius: 6px;
    background: var(--bg-surface);
    color: var(--text-emphasis);
    font: 600 15px var(--font-mono);
  }

  .title-block {
    min-width: 0;
    display: grid;
    gap: 4px;
  }

  h1 {
    color: var(--text-primary);
    font: 600 20px var(--font-mono);
    overflow-wrap: anywhere;
  }

  .meta-row {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
    color: var(--text-secondary);
    font: 12px var(--font-mono);
  }

  .dirty {
    color: var(--status-working);
  }

  .clean {
    color: var(--status-idle);
  }

  .actions {
    display: flex;
    flex-wrap: wrap;
    justify-content: flex-end;
    gap: 8px;
  }

  button {
    border: 1px solid var(--border-default);
    border-radius: 6px;
    padding: 8px 11px;
    color: var(--text-primary);
    font-size: 12px;
    cursor: pointer;
  }

  button:disabled {
    opacity: 0.45;
    cursor: not-allowed;
  }

  .primary {
    background: var(--text-emphasis);
    color: var(--bg-void);
  }

  .secondary {
    background: var(--bg-surface);
  }

  .secondary:not(:disabled):hover {
    background: var(--bg-hover);
  }

  .connection-state,
  .validation-strip {
    display: flex;
    flex-wrap: wrap;
    gap: 14px;
    padding: 9px 20px;
    border-bottom: 1px solid var(--border-subtle);
    color: var(--text-secondary);
    font: 12px var(--font-mono);
  }

  .connection-state.error,
  .invalid {
    color: var(--status-error);
  }

  .valid {
    color: var(--status-idle);
  }

  .warn {
    color: var(--status-working);
  }

  .editor-form {
    min-height: 0;
    flex: 1;
    display: flex;
    flex-direction: column;
    gap: 14px;
    padding: 18px 20px 24px;
    overflow-y: auto;
  }

  .field-grid {
    display: grid;
    grid-template-columns: repeat(4, minmax(0, 1fr));
    gap: 12px;
  }

  .field-grid.lower {
    grid-template-columns: minmax(0, 1.2fr) minmax(180px, 0.8fr);
  }

  label {
    min-width: 0;
    display: grid;
    gap: 6px;
    color: var(--text-secondary);
    font-size: 11px;
    text-transform: uppercase;
  }

  input,
  select,
  textarea {
    width: 100%;
    min-width: 0;
    border: 1px solid var(--border-default);
    border-radius: 6px;
    background: var(--bg-base);
    color: var(--text-primary);
    padding: 9px 10px;
    font: 13px/1.45 var(--font-mono);
  }

  textarea {
    min-height: 120px;
    resize: vertical;
  }

  textarea.prompt {
    min-height: 300px;
  }

  input:focus,
  select:focus,
  textarea:focus,
  button:focus-visible {
    outline: 2px solid var(--focus-ring);
    outline-offset: 2px;
  }

  @media (max-width: 980px) {
    .editor-header {
      flex-direction: column;
    }

    .actions {
      justify-content: flex-start;
    }

    .field-grid,
    .field-grid.lower {
      grid-template-columns: 1fr 1fr;
    }
  }

  @media (max-width: 620px) {
    .field-grid,
    .field-grid.lower {
      grid-template-columns: 1fr;
    }

    .actions button {
      flex: 1 1 120px;
    }
  }
</style>
