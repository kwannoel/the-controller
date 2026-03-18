<script lang="ts">
  import { onMount } from "svelte";
  import { fromStore } from "svelte/store";
  import { projects, type Project, type SessionConfig } from "./stores";

  interface Props {
    onSelect: (session: { projectId: string; sessionId: string }) => void;
    onNewSession: () => void;
    onClose: () => void;
  }

  let { onSelect, onNewSession, onClose }: Props = $props();

  interface PickerItem {
    type: "new" | "session";
    projectId?: string;
    projectName?: string;
    session?: SessionConfig;
  }

  const projectsState = fromStore(projects);

  let items: PickerItem[] = $derived.by(() => {
    const result: PickerItem[] = [{ type: "new" }];
    for (const project of projectsState.current) {
      if (project.name !== "the-controller") continue;
      for (const session of project.sessions) {
        if (session.archived || session.auto_worker_session) continue;
        result.push({
          type: "session",
          projectId: project.id,
          projectName: project.name,
          session,
        });
      }
    }
    return result;
  });

  let selectedIndex = $state(0);

  function confirm() {
    const item = items[selectedIndex];
    if (!item) return;
    if (item.type === "new") {
      onNewSession();
    } else {
      onSelect({ projectId: item.projectId!, sessionId: item.session!.id });
    }
  }

  function scrollSelectedIntoView() {
    requestAnimationFrame(() => {
      const el = document.querySelector(".session-picker-list .picker-btn.selected");
      el?.scrollIntoView({ block: "nearest" });
    });
  }

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === "Escape") {
      e.preventDefault();
      e.stopPropagation();
      onClose();
      return;
    }

    switch (e.key) {
      case "j":
        e.preventDefault();
        e.stopPropagation();
        selectedIndex = (selectedIndex + 1) % items.length;
        scrollSelectedIntoView();
        break;
      case "k":
        e.preventDefault();
        e.stopPropagation();
        selectedIndex = (selectedIndex - 1 + items.length) % items.length;
        scrollSelectedIntoView();
        break;
      case "l":
      case "Enter":
        e.preventDefault();
        e.stopPropagation();
        confirm();
        break;
    }
  }

  onMount(() => {
    window.addEventListener("keydown", handleKeydown, { capture: true });
    return () => {
      window.removeEventListener("keydown", handleKeydown, { capture: true });
    };
  });
</script>

<div
  class="overlay"
  onclick={onClose}
  onkeydown={handleKeydown}
  role="dialog"
  tabindex="-1"
  aria-modal="true"
>
  <!-- svelte-ignore a11y_click_events_have_key_events -->
  <div class="modal" onclick={(e) => e.stopPropagation()} role="presentation">
    <div class="modal-header">Send Screenshot To</div>
    <ul class="session-picker-list">
      {#each items as item, index (item.type === "new" ? "__new__" : item.session?.id)}
        <li>
          {#if item.type === "new"}
            <button
              class="picker-btn new-session"
              class:selected={selectedIndex === index}
              onclick={() => { selectedIndex = index; confirm(); }}
            >
              <span class="new-label">+ New session</span>
            </button>
          {:else}
            <button
              class="picker-btn"
              class:selected={selectedIndex === index}
              onclick={() => { selectedIndex = index; confirm(); }}
            >
              <div class="session-info">
                <div class="session-top-row">
                  <span class="session-label">{item.session?.label}</span>
                  {#if item.session?.github_issue}
                    <span class="issue-tag">#{item.session.github_issue.number}</span>
                  {/if}
                </div>
                {#if item.session?.initial_prompt}
                  <div class="session-summary">{item.session.initial_prompt}</div>
                {/if}
              </div>
            </button>
          {/if}
        </li>
      {/each}
    </ul>
  </div>
</div>

<style>
  .overlay {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.7);
    backdrop-filter: blur(16px);
    display: flex;
    align-items: flex-start;
    justify-content: center;
    padding-top: 20vh;
    z-index: 100;
  }
  .modal {
    background: var(--bg-elevated);
    border: 1px solid var(--border-default);
    border-radius: 8px;
    width: 480px;
    padding: 24px;
    display: flex;
    flex-direction: column;
    gap: 12px;
    box-shadow: 0 8px 32px rgba(0, 0, 0, 0.6);
  }
  .modal-header {
    font-size: 16px;
    font-weight: 600;
    color: var(--text-emphasis);
  }
  .session-picker-list {
    list-style: none;
    margin: 0;
    padding: 0;
    max-height: 50vh;
    overflow-y: auto;
  }
  .session-picker-list li {
    border-bottom: 1px solid var(--border-default);
  }
  .session-picker-list li:last-child {
    border-bottom: none;
  }
  .picker-btn {
    width: 100%;
    display: flex;
    gap: 8px;
    align-items: center;
    padding: 10px 8px;
    background: none;
    border: none;
    color: var(--text-primary);
    font-size: 13px;
    cursor: pointer;
    text-align: left;
    box-shadow: none;
  }
  .picker-btn:hover,
  .picker-btn.selected {
    background: var(--bg-hover);
    border-radius: 4px;
  }
  .new-label {
    color: var(--text-emphasis);
    font-weight: 500;
  }
  .session-info {
    display: flex;
    flex-direction: column;
    gap: 2px;
    overflow: hidden;
    flex: 1;
  }
  .session-top-row {
    display: flex;
    gap: 8px;
    align-items: center;
  }
  .session-label {
    color: var(--text-primary);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .session-summary {
    color: var(--text-secondary);
    font-size: 11px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .issue-tag {
    color: var(--text-secondary);
    font-size: 11px;
    flex-shrink: 0;
  }
</style>
