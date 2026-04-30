<script lang="ts">
  import type { AgentProfile } from "../daemon/types";
  import { validateProfileDraft, type ProfileDraft } from "./profile-validation";

  interface Props {
    profiles: AgentProfile[];
    selectedId: string | null;
    onNewProfile: () => void;
    onSelectProfile: (id: string) => void;
  }

  let { profiles, selectedId, onNewProfile, onSelectProfile }: Props = $props();
  let query = $state("");
  let showArchived = $state(false);

  function draftFromProfile(profile: AgentProfile): ProfileDraft {
    return {
      id: profile.id,
      name: profile.name,
      handle: profile.handle,
      runtime: profile.runtime,
      model: "",
      description: profile.description,
      prompt: profile.prompt,
      skills: profile.skills,
      outbox_instructions: "",
      default_workspace_behavior: "focused",
    };
  }

  function statusFor(profile: AgentProfile): "archived" | "invalid" | "warn" | "valid" {
    if (profile.archived_at !== null) return "archived";
    const result = validateProfileDraft(draftFromProfile(profile));
    if (result.blocking.length > 0) return "invalid";
    if (result.warnings.length > 0) return "warn";
    return "valid";
  }

  function initials(name: string): string {
    const words = name.trim().split(/\s+/).filter(Boolean);
    if (words.length === 0) return "?";
    return words.slice(0, 2).map((word) => word[0]?.toUpperCase()).join("");
  }

  let filteredProfiles = $derived(
    profiles.filter((profile) => {
      if (!showArchived && profile.archived_at !== null) return false;
      const haystack = [
        profile.name,
        profile.handle,
        profile.runtime,
        profile.description,
        ...profile.skills,
      ].join(" ").toLowerCase();
      return haystack.includes(query.trim().toLowerCase());
    })
  );
</script>

<aside class="profile-list-pane" aria-label="Agent profiles">
  <header class="list-header">
    <div>
      <p class="eyebrow">Agent Profiles</p>
      <h2>Profiles</h2>
    </div>
    <button type="button" class="icon-action" onclick={onNewProfile} aria-label="Create profile">+</button>
  </header>

  <div class="list-controls">
    <label>
      <span>Search</span>
      <input type="search" bind:value={query} placeholder="@reviewer" />
    </label>
    <div class="filter-row" role="group" aria-label="Profile filter">
      <button type="button" class:active={!showArchived} onclick={() => (showArchived = false)}>Active</button>
      <button type="button" class:active={showArchived} onclick={() => (showArchived = true)}>Archived</button>
    </div>
  </div>

  <div class="rows" role="listbox" aria-label="Profiles">
    {#each filteredProfiles as profile (profile.id)}
      {@const status = statusFor(profile)}
      <button
        type="button"
        class="profile-row"
        class:selected={selectedId === profile.id}
        onclick={() => onSelectProfile(profile.id)}
        role="option"
        aria-selected={selectedId === profile.id}
      >
        <span class="avatar">{initials(profile.name)}</span>
        <span class="row-main">
          <span class="row-title">{profile.name || profile.handle}</span>
          <span class="row-meta">@{profile.handle} · {profile.runtime}</span>
        </span>
        <span class="status" class:warn={status === "warn"} class:invalid={status === "invalid"} class:archived={status === "archived"}>
          <span class="status-dot"></span>{status}
        </span>
      </button>
    {/each}
  </div>
</aside>

<style>
  .profile-list-pane {
    width: 300px;
    min-width: 260px;
    height: 100%;
    display: flex;
    flex-direction: column;
    border-right: 1px solid var(--border-default);
    background: var(--bg-base);
    overflow: hidden;
  }

  .list-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 18px 18px 14px;
    border-bottom: 1px solid var(--border-default);
  }

  .eyebrow {
    color: var(--text-secondary);
    font-family: var(--font-mono);
    font-size: 11px;
    text-transform: uppercase;
  }

  h2 {
    color: var(--text-primary);
    font-size: 17px;
    font-weight: 600;
    line-height: 1.3;
  }

  .icon-action {
    width: 32px;
    height: 32px;
    border: 1px solid var(--border-default);
    border-radius: 6px;
    background: var(--bg-surface);
    color: var(--text-primary);
    font-family: var(--font-mono);
    font-size: 20px;
    cursor: pointer;
  }

  .icon-action:hover {
    background: var(--bg-hover);
  }

  .list-controls {
    display: grid;
    gap: 10px;
    padding: 12px;
    border-bottom: 1px solid var(--border-subtle);
  }

  label {
    display: grid;
    gap: 6px;
    color: var(--text-secondary);
    font-size: 11px;
    text-transform: uppercase;
  }

  input {
    width: 100%;
    min-width: 0;
    border: 1px solid var(--border-default);
    border-radius: 6px;
    background: var(--bg-void);
    color: var(--text-primary);
    padding: 8px 10px;
    font: 13px var(--font-mono);
  }

  .filter-row {
    display: grid;
    grid-template-columns: 1fr 1fr;
    border: 1px solid var(--border-default);
    border-radius: 6px;
    overflow: hidden;
  }

  .filter-row button {
    border: 0;
    border-right: 1px solid var(--border-default);
    background: var(--bg-surface);
    color: var(--text-secondary);
    padding: 7px 8px;
    font-size: 12px;
    cursor: pointer;
  }

  .filter-row button:last-child {
    border-right: 0;
  }

  .filter-row button.active {
    background: var(--bg-active);
    color: var(--text-primary);
  }

  .rows {
    flex: 1;
    overflow-y: auto;
    padding: 8px;
  }

  .profile-row {
    width: 100%;
    min-width: 0;
    display: grid;
    grid-template-columns: 32px minmax(0, 1fr) auto;
    align-items: center;
    gap: 10px;
    border: 1px solid transparent;
    border-radius: 6px;
    background: transparent;
    color: var(--text-primary);
    padding: 10px 8px;
    text-align: left;
    cursor: pointer;
  }

  .profile-row:hover,
  .profile-row.selected {
    border-color: var(--border-default);
    background: var(--bg-surface);
  }

  .avatar {
    width: 32px;
    height: 32px;
    display: grid;
    place-items: center;
    border: 1px solid var(--border-default);
    border-radius: 6px;
    background: var(--bg-void);
    color: var(--text-primary);
    font: 600 12px var(--font-mono);
  }

  .row-main {
    min-width: 0;
    display: grid;
    gap: 3px;
  }

  .row-title,
  .row-meta {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .row-title {
    font-size: 13px;
  }

  .row-meta {
    color: var(--text-secondary);
    font: 12px var(--font-mono);
  }

  .status {
    display: flex;
    align-items: center;
    gap: 6px;
    color: var(--status-idle);
    font: 12px var(--font-mono);
  }

  .status.warn {
    color: var(--status-working);
  }

  .status.invalid {
    color: var(--status-error);
  }

  .status.archived {
    color: var(--text-tertiary);
  }

  .status-dot {
    width: 7px;
    height: 7px;
    border-radius: 999px;
    background: currentColor;
  }

  @media (max-width: 860px) {
    .profile-list-pane {
      width: 100%;
      height: auto;
      max-height: 42vh;
      border-right: 0;
      border-bottom: 1px solid var(--border-default);
    }
  }
</style>
