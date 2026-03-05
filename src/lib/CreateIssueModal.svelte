<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { onMount } from "svelte";
  import { showToast } from "./toast";

  interface GithubIssue {
    number: number;
    title: string;
    url: string;
    labels: { name: string }[];
  }

  interface Props {
    repoPath: string;
    onCreated: (issue: GithubIssue) => void;
    onClose: () => void;
  }

  let { repoPath, onCreated, onClose }: Props = $props();

  let title = $state("");
  let loading = $state(false);
  let titleInput: HTMLInputElement | undefined = $state();

  onMount(() => {
    titleInput?.focus();
  });

  async function create() {
    if (!title.trim() || loading) return;
    loading = true;
    try {
      const issue = await invoke<GithubIssue>("create_github_issue", {
        repoPath,
        title: title.trim(),
      });
      onCreated(issue);
    } catch (e) {
      showToast(String(e), "error");
      loading = false;
    }
  }

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === "Escape") {
      e.preventDefault();
      e.stopPropagation();
      onClose();
    } else if (e.key === "Enter") {
      e.preventDefault();
      create();
    }
  }
</script>

<div class="overlay" onclick={onClose} onkeydown={handleKeydown} role="dialog">
  <!-- svelte-ignore a11y_click_events_have_key_events -->
  <div class="modal" onclick={(e) => e.stopPropagation()} role="presentation">
    <div class="modal-header">New GitHub Issue</div>
    <input
      bind:this={titleInput}
      bind:value={title}
      placeholder="Issue title"
      class="input"
      disabled={loading}
    />
    <button
      class="btn-primary"
      onclick={create}
      disabled={!title.trim() || loading}
    >
      {loading ? "Creating..." : "Create Issue"}
    </button>
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
    width: 380px;
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
</style>
