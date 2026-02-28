<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { showToast } from "./toast";
  import type { Project } from "./stores";

  interface Props {
    onCreated: (project: Project) => void;
    onClose: () => void;
  }

  let { onCreated, onClose }: Props = $props();

  let step = $state<"describe" | "pick">("describe");
  let description = $state("");
  let suggestions = $state<string[]>([]);
  let customName = $state("");
  let loading = $state(false);
  let selectedIndex = $state(0);

  async function generateNames() {
    if (!description.trim() || loading) return;
    loading = true;
    try {
      suggestions = await invoke<string[]>("generate_project_names", {
        description: description.trim(),
      });
      step = "pick";
      selectedIndex = 0;
    } catch (e) {
      showToast(String(e), "error");
    } finally {
      loading = false;
    }
  }

  async function createWithName(name: string) {
    if (!name.trim() || loading) return;
    loading = true;
    try {
      const project = await invoke<Project>("scaffold_project", {
        name: name.trim(),
      });
      onCreated(project);
    } catch (e) {
      showToast(String(e), "error");
    } finally {
      loading = false;
    }
  }

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === "Escape") {
      e.preventDefault();
      onClose();
    } else if (step === "describe" && e.key === "Enter") {
      e.preventDefault();
      generateNames();
    } else if (step === "pick") {
      if (e.key === "ArrowDown") {
        e.preventDefault();
        selectedIndex = Math.min(selectedIndex + 1, suggestions.length);
      } else if (e.key === "ArrowUp") {
        e.preventDefault();
        selectedIndex = Math.max(selectedIndex - 1, 0);
      } else if (e.key === "Enter") {
        e.preventDefault();
        if (selectedIndex < suggestions.length) {
          createWithName(suggestions[selectedIndex]);
        } else if (customName.trim()) {
          createWithName(customName);
        }
      }
    }
  }
</script>

<div class="overlay" onclick={onClose} onkeydown={handleKeydown} role="dialog">
  <!-- svelte-ignore a11y_click_events_have_key_events -->
  <div class="modal" onclick={(e) => e.stopPropagation()} role="presentation">
    {#if step === "describe"}
      <div class="modal-header">New Project</div>
      <p class="hint">Describe your project in a few words</p>
      <input
        bind:value={description}
        placeholder="e.g. real-time chat app"
        class="input"
        disabled={loading}
      />
      <button
        class="btn-primary"
        onclick={generateNames}
        disabled={!description.trim() || loading}
      >
        {loading ? "Generating..." : "Generate Names"}
      </button>
    {:else}
      <div class="modal-header">Pick a name</div>
      <div class="suggestions">
        {#each suggestions as name, i}
          <div
            class="suggestion"
            class:selected={i === selectedIndex}
            onclick={() => !loading && createWithName(name)}
            role="option"
            aria-selected={i === selectedIndex}
          >
            {name}
          </div>
        {/each}
      </div>
      <div class="custom-name">
        <input
          bind:value={customName}
          placeholder="Or type a custom name..."
          class="input"
          class:selected={selectedIndex === suggestions.length}
          onfocus={() => (selectedIndex = suggestions.length)}
        />
      </div>
      <div class="actions">
        <button class="btn-secondary" onclick={() => (step = "describe")}
          >Back</button
        >
      </div>
    {/if}
  </div>
</div>

<style>
  .overlay {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.6);
    display: flex;
    align-items: flex-start;
    justify-content: center;
    padding-top: 20vh;
    z-index: 100;
  }
  .modal {
    background: #1e1e2e;
    border: 1px solid #313244;
    border-radius: 8px;
    width: 420px;
    padding: 24px;
    display: flex;
    flex-direction: column;
    gap: 12px;
  }
  .modal-header {
    font-size: 16px;
    font-weight: 600;
    color: #cdd6f4;
  }
  .hint {
    color: #a6adc8;
    font-size: 13px;
    margin: 0;
  }
  .input {
    background: #313244;
    color: #cdd6f4;
    border: 1px solid #45475a;
    padding: 10px 12px;
    border-radius: 6px;
    font-size: 14px;
    outline: none;
    width: 100%;
    box-sizing: border-box;
  }
  .input:focus {
    border-color: #89b4fa;
  }
  .btn-primary {
    background: #89b4fa;
    color: #1e1e2e;
    border: none;
    padding: 10px;
    border-radius: 6px;
    font-size: 14px;
    font-weight: 600;
    cursor: pointer;
  }
  .btn-primary:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }
  .btn-secondary {
    background: #45475a;
    color: #cdd6f4;
    border: none;
    padding: 8px 16px;
    border-radius: 6px;
    font-size: 13px;
    cursor: pointer;
  }
  .suggestions {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  .suggestion {
    padding: 10px 12px;
    border-radius: 6px;
    cursor: pointer;
    color: #cdd6f4;
    font-size: 14px;
    font-family: monospace;
  }
  .suggestion:hover,
  .suggestion.selected {
    background: #313244;
  }
  .custom-name {
    margin-top: 4px;
  }
  .actions {
    display: flex;
    justify-content: flex-start;
  }
</style>
