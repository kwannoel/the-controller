<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { onMount } from "svelte";
  import { appConfig, onboardingComplete } from "./stores";
  import { showToast } from "./toast";

  interface DirEntry {
    name: string;
    path: string;
  }

  let step = $state<1 | 2>(1);
  let projectsRoot = $state("");
  let claudeStatus = $state<
    "checking" | "authenticated" | "not_authenticated" | "not_installed"
  >("checking");

  // Fuzzy finder state
  let query = $state("");
  let entries = $state<DirEntry[]>([]);
  let filtered = $derived(
    query.trim() === ""
      ? entries
      : entries.filter((e) =>
          e.name.toLowerCase().includes(query.toLowerCase()),
        ),
  );
  let selectedIndex = $state(0);
  let inputEl: HTMLInputElement | undefined = $state();

  onMount(async () => {
    try {
      const homeDir =
        (await invoke<string | null>("home_dir")) ?? "/Users";
      entries = await invoke<DirEntry[]>("list_directories_at", {
        path: homeDir,
      });
    } catch (e) {
      showToast(String(e), "error");
    }
    inputEl?.focus();
  });

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === "ArrowDown") {
      e.preventDefault();
      selectedIndex = Math.min(selectedIndex + 1, filtered.length - 1);
    } else if (e.key === "ArrowUp") {
      e.preventDefault();
      selectedIndex = Math.max(selectedIndex - 1, 0);
    } else if (e.key === "Enter" && filtered.length > 0) {
      e.preventDefault();
      selectDirectory(filtered[selectedIndex]);
    }
  }

  $effect(() => {
    query;
    selectedIndex = 0;
  });

  async function selectDirectory(entry: DirEntry) {
    projectsRoot = entry.path;
    try {
      await invoke("save_onboarding_config", {
        projectsRoot: entry.path,
      });
      step = 2;
      await checkClaude();
    } catch (e) {
      showToast(String(e), "error");
    }
  }

  async function checkClaude() {
    claudeStatus = "checking";
    try {
      const status = await invoke<string>("check_claude_cli");
      claudeStatus = status as typeof claudeStatus;
    } catch (e) {
      claudeStatus = "not_installed";
    }
  }

  function finishOnboarding() {
    appConfig.set({ projects_root: projectsRoot });
    onboardingComplete.set(true);
  }
</script>

<div class="onboarding">
  {#if step === 1}
    <div class="finder">
      <h1>Where do your projects live?</h1>
      <input
        bind:this={inputEl}
        bind:value={query}
        placeholder="Search directories..."
        class="search-input"
        onkeydown={handleKeydown}
      />
      <div class="results">
        {#each filtered as entry, i (entry.path)}
          <div
            class="result-item"
            class:selected={i === selectedIndex}
            onclick={() => selectDirectory(entry)}
            role="option"
            tabindex="0"
            aria-selected={i === selectedIndex}
          >
            <span class="entry-name">{entry.name}</span>
            <span class="entry-path">{entry.path}</span>
          </div>
        {/each}
        {#if filtered.length === 0}
          <div class="empty">No matching directories</div>
        {/if}
      </div>
      <p class="hint-text">Select the folder that contains your project directories</p>
    </div>
  {:else}
    <div class="card">
      <h1>Claude CLI</h1>
      <p class="selected-path">Projects root: <code>{projectsRoot}</code></p>

      {#if claudeStatus === "checking"}
        <p>Checking Claude CLI...</p>
      {:else if claudeStatus === "authenticated"}
        <p class="success">Claude CLI is ready</p>
        <button onclick={finishOnboarding}>Get Started</button>
      {:else if claudeStatus === "not_authenticated"}
        <p class="warning">Claude CLI found but not authenticated.</p>
        <p class="hint">
          Run <code>claude login</code> in your terminal, then:
        </p>
        <button onclick={checkClaude}>Check Again</button>
      {:else}
        <p class="warning">Claude CLI not found.</p>
        <p class="hint">Install it, then:</p>
        <button onclick={checkClaude}>Check Again</button>
      {/if}
    </div>
  {/if}
</div>

<style>
  .onboarding {
    width: 100vw;
    height: 100vh;
    display: flex;
    align-items: center;
    justify-content: center;
    background: #11111b;
    color: #cdd6f4;
  }
  .finder {
    background: #1e1e2e;
    border: 1px solid #313244;
    border-radius: 8px;
    width: 500px;
    max-height: 500px;
    display: flex;
    flex-direction: column;
    overflow: hidden;
  }
  .finder h1 {
    font-size: 16px;
    font-weight: 600;
    margin: 0;
    padding: 16px 16px 0;
  }
  .search-input {
    background: #1e1e2e;
    color: #cdd6f4;
    border: none;
    border-bottom: 1px solid #313244;
    padding: 14px 16px;
    font-size: 15px;
    outline: none;
  }
  .results {
    overflow-y: auto;
    max-height: 340px;
  }
  .result-item {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 10px 16px;
    cursor: pointer;
  }
  .result-item:hover,
  .result-item.selected {
    background: #313244;
  }
  .entry-name {
    color: #cdd6f4;
    font-size: 14px;
  }
  .entry-path {
    color: #6c7086;
    font-size: 12px;
  }
  .empty {
    padding: 20px 16px;
    color: #6c7086;
    font-size: 13px;
    text-align: center;
  }
  .hint-text {
    padding: 10px 16px;
    margin: 0;
    color: #6c7086;
    font-size: 12px;
    border-top: 1px solid #313244;
  }
  .card {
    background: #1e1e2e;
    padding: 40px;
    border-radius: 12px;
    border: 1px solid #313244;
    max-width: 480px;
    width: 100%;
    display: flex;
    flex-direction: column;
    gap: 16px;
  }
  .card h1 {
    font-size: 20px;
    font-weight: 600;
    margin: 0;
  }
  p {
    margin: 0;
    color: #a6adc8;
    font-size: 14px;
  }
  .selected-path {
    font-size: 13px;
  }
  button {
    background: #89b4fa;
    color: #1e1e2e;
    border: none;
    padding: 10px 20px;
    border-radius: 6px;
    font-size: 14px;
    font-weight: 600;
    cursor: pointer;
  }
  button:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }
  .success {
    color: #a6e3a1;
  }
  .warning {
    color: #fab387;
  }
  .hint {
    font-size: 13px;
  }
  code {
    background: #313244;
    padding: 2px 6px;
    border-radius: 4px;
    font-size: 13px;
  }
</style>
