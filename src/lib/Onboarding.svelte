<script lang="ts">
  import { command } from "$lib/backend";
  import { onMount, onDestroy } from "svelte";
  import { appConfig, onboardingComplete, type DirEntry } from "./stores";
  import { showToast } from "./toast";
  import Terminal from "./Terminal.svelte";

  let step = $state<"pick-dir" | "cli-check">("pick-dir");
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

  // Login terminal state
  let loginSessionId = $state<string | null>(null);

  onMount(async () => {
    try {
      const homeDir =
        (await command<string | null>("home_dir")) ?? "/Users";
      entries = await command<DirEntry[]>("list_directories_at", {
        path: homeDir,
      });
    } catch (e) {
      showToast(String(e), "error");
    }
    inputEl?.focus();
  });

  onDestroy(async () => {
    if (loginSessionId) {
      try {
        await command("stop_claude_login", { sessionId: loginSessionId });
      } catch (_) {}
    }
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
    step = "cli-check";
    command("save_onboarding_config", { projectsRoot: entry.path }).catch((e) =>
      showToast(String(e), "error"),
    );
    checkClaude();
  }

  async function checkClaude() {
    claudeStatus = "checking";
    try {
      const status = await command<string>("check_claude_cli");
      claudeStatus = status as typeof claudeStatus;

      if (status === "authenticated") {
        // Show the success state briefly, then auto-proceed
        setTimeout(finishOnboarding, 1000);
      } else if (status === "not_authenticated") {
        await startLogin();
      }
    } catch (e) {
      claudeStatus = "not_installed";
    }
  }

  async function startLogin() {
    try {
      loginSessionId = await command<string>("start_claude_login");
    } catch (e) {
      showToast(String(e), "error");
    }
  }

  async function handleLoginDone() {
    // Clean up login PTY
    if (loginSessionId) {
      try {
        await command("stop_claude_login", { sessionId: loginSessionId });
      } catch (_) {}
      loginSessionId = null;
    }
    // Re-check auth
    await checkClaude();
  }

  function finishOnboarding() {
    appConfig.set({ projects_root: projectsRoot });
    onboardingComplete.set(true);
  }
</script>

<div class="onboarding">
  {#if step === "pick-dir"}
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
      <p class="hint-text">
        Select the folder that contains your project directories
      </p>
    </div>
  {:else}
    <div class="card">
      <h1>Claude CLI</h1>
      <p class="selected-path">
        Projects root: <code>{projectsRoot}</code>
      </p>

      {#if claudeStatus === "checking"}
        <div class="checking">
          <div class="spinner"></div>
          <p>Checking Claude CLI...</p>
        </div>
      {:else if claudeStatus === "authenticated"}
        <p class="success">Claude CLI is ready</p>
        <button onclick={finishOnboarding}>Get Started</button>
      {:else if claudeStatus === "not_authenticated" && loginSessionId}
        <p class="hint">Complete the login below, then click Done:</p>
        <div class="login-terminal">
          <Terminal sessionId={loginSessionId} />
        </div>
        <button onclick={handleLoginDone}>Done</button>
      {:else if claudeStatus === "not_authenticated"}
        <p class="warning">Claude CLI found but not authenticated.</p>
        <button onclick={startLogin}>Log In</button>
      {:else}
        <p class="warning">Claude CLI not found.</p>
        <p class="hint">
          Install from
          <code>https://docs.anthropic.com/en/docs/claude-code</code>, then:
        </p>
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
    background: var(--bg-void);
    color: var(--text-primary);
  }
  .finder {
    background: var(--bg-elevated);
    border: 1px solid var(--border-default);
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
    background: var(--bg-elevated);
    color: var(--text-primary);
    border: none;
    border-bottom: 1px solid var(--border-default);
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
    background: var(--bg-hover);
  }
  .entry-name {
    color: var(--text-primary);
    font-size: 14px;
  }
  .entry-path {
    color: var(--text-secondary);
    font-size: 12px;
  }
  .empty {
    padding: 20px 16px;
    color: var(--text-secondary);
    font-size: 13px;
    text-align: center;
  }
  .hint-text {
    padding: 10px 16px;
    margin: 0;
    color: var(--text-secondary);
    font-size: 12px;
    border-top: 1px solid var(--border-default);
  }
  .card {
    background: var(--bg-elevated);
    padding: 40px;
    border-radius: 12px;
    border: 1px solid var(--border-default);
    max-width: 560px;
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
    color: var(--text-secondary);
    font-size: 14px;
  }
  .selected-path {
    font-size: 13px;
  }
  .login-terminal {
    height: 300px;
    border: 1px solid var(--border-default);
    border-radius: 6px;
    overflow: hidden;
  }
  button {
    background: var(--text-emphasis);
    color: var(--bg-void);
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
  .checking {
    display: flex;
    align-items: center;
    gap: 12px;
  }
  .spinner {
    width: 20px;
    height: 20px;
    border: 2.5px solid var(--border-default);
    border-top-color: var(--text-emphasis);
    border-radius: 50%;
    animation: spin 0.8s linear infinite;
  }
  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }
  .success {
    color: var(--status-idle);
  }
  .warning {
    color: var(--status-working);
  }
  .hint {
    font-size: 13px;
  }
  code {
    background: var(--bg-hover);
    padding: 2px 6px;
    border-radius: 4px;
    font-size: 13px;
  }
</style>
