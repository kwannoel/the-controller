<script lang="ts">
  import type { AgentProfile, RouteTokenKind } from "../daemon/types";

  export interface AgentTokenSelection {
    kind: RouteTokenKind;
    profileId: string;
    handle: string;
  }

  let {
    kind,
    query = "",
    profiles,
    onSelect,
    onClose,
  }: {
    kind: RouteTokenKind;
    query?: string;
    profiles: AgentProfile[];
    onSelect: (selection: AgentTokenSelection) => void;
    onClose?: () => void;
  } = $props();

  let activeIndex = $state(0);

  const marker = $derived(kind === "reusable" ? "@" : "%");
  const filteredProfiles = $derived.by(() => {
    const needle = query.trim().toLowerCase();
    return profiles
      .filter((profile) => {
        if (profile.archived_at !== null) return false;
        if (!needle) return true;
        return (
          profile.handle.toLowerCase().includes(needle) ||
          profile.name.toLowerCase().includes(needle)
        );
      })
      .sort((a, b) => a.handle.localeCompare(b.handle));
  });

  $effect(() => {
    if (activeIndex >= filteredProfiles.length) activeIndex = 0;
  });

  function selectProfile(profile: AgentProfile) {
    onSelect({ kind, profileId: profile.id, handle: profile.handle });
  }

  function handleKeyDown(e: KeyboardEvent) {
    if (filteredProfiles.length === 0 && e.key !== "Escape") return;
    if (e.key === "ArrowDown") {
      e.preventDefault();
      activeIndex = (activeIndex + 1) % filteredProfiles.length;
    } else if (e.key === "ArrowUp") {
      e.preventDefault();
      activeIndex = (activeIndex - 1 + filteredProfiles.length) % filteredProfiles.length;
    } else if (e.key === "Enter") {
      e.preventDefault();
      const profile = filteredProfiles[activeIndex];
      if (profile) selectProfile(profile);
    } else if (e.key === "Escape") {
      e.preventDefault();
      onClose?.();
    }
  }
</script>

<div
  class="menu"
  role="listbox"
  aria-label="Agent token suggestions"
  tabindex="0"
  onkeydown={handleKeyDown}
>
  {#if filteredProfiles.length > 0}
    {#each filteredProfiles as profile, index (profile.id)}
      <button
        class:active={index === activeIndex}
        role="option"
        aria-selected={index === activeIndex}
        type="button"
        onmouseenter={() => activeIndex = index}
        onclick={() => selectProfile(profile)}
      >
        <span class="handle">{marker}{profile.handle}</span>
        <span class="name">{profile.name}</span>
      </button>
    {/each}
  {:else}
    <div class="empty">none</div>
  {/if}
</div>

<style>
  .menu {
    position: absolute;
    left: 12px;
    right: 12px;
    bottom: calc(100% - 8px);
    z-index: 10;
    max-height: 160px;
    overflow-y: auto;
    border: 1px solid var(--border-default);
    border-radius: 4px;
    background: var(--bg-elevated);
    box-shadow: 0 8px 18px rgb(0 0 0 / 0.24);
    padding: 4px;
    outline: none;
  }

  button {
    width: 100%;
    display: grid;
    grid-template-columns: minmax(96px, max-content) 1fr;
    gap: 8px;
    align-items: center;
    min-height: 28px;
    border: 0;
    border-radius: 3px;
    padding: 4px 6px;
    background: transparent;
    color: var(--text-secondary);
    font: inherit;
    font-size: 12px;
    text-align: left;
  }

  button.active,
  button:hover {
    background: var(--bg-hover);
    color: var(--text-primary);
  }

  .handle {
    color: var(--text-emphasis);
    font-weight: 600;
  }

  .name {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .empty {
    padding: 6px;
    color: var(--text-secondary);
    font-size: 12px;
  }
</style>
