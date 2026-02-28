<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { appConfig, onboardingComplete } from "./stores";
  import { showToast } from "./toast";

  let step = $state<1 | 2>(1);
  let projectsRoot = $state("");
  let claudeStatus = $state<
    "checking" | "authenticated" | "not_authenticated" | "not_installed"
  >("checking");

  async function handleNextStep1() {
    if (!projectsRoot.trim()) return;
    try {
      await invoke("save_onboarding_config", {
        projectsRoot: projectsRoot.trim(),
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
    appConfig.set({ projects_root: projectsRoot.trim() });
    onboardingComplete.set(true);
  }
</script>

<div class="onboarding">
  <div class="card">
    {#if step === 1}
      <h1>Welcome to The Controller</h1>
      <p>Where do your projects live?</p>
      <input
        type="text"
        bind:value={projectsRoot}
        placeholder="~/projects"
        onkeydown={(e) => e.key === "Enter" && handleNextStep1()}
      />
      <button onclick={handleNextStep1} disabled={!projectsRoot.trim()}>
        Next
      </button>
    {:else}
      <h1>Claude CLI</h1>

      {#if claudeStatus === "checking"}
        <p>Checking Claude CLI...</p>
      {:else if claudeStatus === "authenticated"}
        <p class="success">Claude CLI is ready.</p>
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
    {/if}
  </div>
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
  h1 {
    font-size: 20px;
    font-weight: 600;
    margin: 0;
  }
  p {
    margin: 0;
    color: #a6adc8;
    font-size: 14px;
  }
  input {
    background: #313244;
    color: #cdd6f4;
    border: 1px solid #45475a;
    padding: 10px 12px;
    border-radius: 6px;
    font-size: 14px;
    outline: none;
  }
  input:focus {
    border-color: #89b4fa;
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
