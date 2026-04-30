<script lang="ts">
  import { onMount } from "svelte";
  import { daemonStore, loadProfiles } from "../daemon/store.svelte";
  import type { AgentProfile } from "../daemon/types";
  import { workspaceMode } from "../stores";
  import ProfileEditor from "./ProfileEditor.svelte";
  import ProfileList from "./ProfileList.svelte";
  import ProfilePreviewDrawer from "./ProfilePreviewDrawer.svelte";
  import { validateProfileDraft, type ProfileDraft } from "./profile-validation";

  let selectedProfileId: string | null = $state(null);
  let draft: ProfileDraft | null = $state(null);
  let baseline = $state("");
  let previewOpen = $state(false);
  let saving = $state(false);
  let actionError: string | null = $state(null);

  let profiles = $derived([...daemonStore.profiles.values()].sort((a, b) => a.name.localeCompare(b.name)));
  let selectedProfile = $derived(selectedProfileId ? daemonStore.profiles.get(selectedProfileId) ?? null : null);
  let validation = $derived(draft ? validateProfileDraft(draft) : { blocking: [], warnings: [] });
  let dirty = $derived(draft ? serializeDraft(draft) !== baseline : false);
  let clientAvailable = $derived(daemonStore.client !== null);
  let canSave = $derived(Boolean(draft && dirty && validation.blocking.length === 0 && clientAvailable && !saving));
  let savedDraftId = $derived(getSavedDraftId(draft));
  let canTest = $derived(Boolean(savedDraftId && !dirty && validation.blocking.length === 0 && clientAvailable));

  onMount(() => {
    window.addEventListener("keydown", handleWorkspaceKeydown, { capture: true });
    if (daemonStore.client) {
      void loadProfiles().catch(() => {
        actionError = "Unable to load profiles from the daemon.";
      });
    }
    return () => {
      window.removeEventListener("keydown", handleWorkspaceKeydown, { capture: true });
    };
  });

  $effect(() => {
    if (!draft && profiles.length > 0) {
      openProfile(profiles[0].id);
    }
  });

  function serializeDraft(value: ProfileDraft): string {
    return JSON.stringify({
      id: value.id ?? null,
      name: value.name,
      handle: value.handle,
      runtime: value.runtime,
      model: value.model,
      description: value.description,
      prompt: value.prompt,
      skills: value.skills,
      outbox_instructions: value.outbox_instructions,
      default_workspace_behavior: value.default_workspace_behavior,
    });
  }

  function getSavedDraftId(value: ProfileDraft | null): string | undefined {
    return value?.id;
  }

  function isEditableElementFocused(): boolean {
    const element = document.activeElement;
    if (!element) return false;
    if (element.tagName === "INPUT" || element.tagName === "TEXTAREA" || element.tagName === "SELECT") return true;
    return (element as HTMLElement).isContentEditable;
  }

  function handleWorkspaceKeydown(event: KeyboardEvent) {
    if (event.repeat || event.metaKey || event.ctrlKey || event.altKey) return;
    if (event.key !== "n") return;
    if (isEditableElementFocused()) return;
    event.preventDefault();
    event.stopPropagation();
    startNewProfile();
  }

  function newDraft(): ProfileDraft {
    return {
      name: "",
      handle: "",
      runtime: "codex",
      model: "",
      description: "",
      prompt: "",
      skills: [],
      outbox_instructions: "",
      default_workspace_behavior: "focused",
    };
  }

  function draftFromProfile(profile: AgentProfile): ProfileDraft {
    return {
      id: profile.id,
      name: profile.name,
      handle: profile.handle,
      runtime: profile.runtime,
      model: "",
      description: profile.description,
      prompt: profile.prompt,
      skills: [...profile.skills],
      outbox_instructions: "",
      default_workspace_behavior: "focused",
    };
  }

  function startNewProfile() {
    actionError = null;
    selectedProfileId = null;
    draft = newDraft();
    baseline = serializeDraft(draft);
    previewOpen = false;
  }

  function openProfile(id: string) {
    const profile = daemonStore.profiles.get(id);
    if (!profile) return;
    actionError = null;
    selectedProfileId = id;
    draft = draftFromProfile(profile);
    baseline = serializeDraft(draft);
    previewOpen = false;
  }

  function updateDraft(patch: Partial<ProfileDraft>) {
    if (!draft) return;
    draft = { ...draft, ...patch };
  }

  function duplicateDraft() {
    if (!draft) return;
    draft = {
      ...draft,
      id: undefined,
      name: `${draft.name || "Profile"} Copy`,
      handle: draft.handle ? `${draft.handle}-copy` : "",
    };
    selectedProfileId = null;
    baseline = serializeDraft(newDraft());
    previewOpen = false;
  }

  async function saveDraft() {
    if (!draft || !daemonStore.client || !canSave) return;
    saving = true;
    actionError = null;
    try {
      const saved = await daemonStore.client.saveProfile({
        id: draft.id,
        name: draft.name.trim(),
        handle: draft.handle.trim(),
        runtime: draft.runtime || undefined,
        model: draft.model.trim() || null,
        description: draft.description.trim() || null,
        prompt: draft.prompt.trim(),
        skills: draft.skills,
        default_workspace_behavior: draft.default_workspace_behavior || null,
        outbox_instructions: draft.outbox_instructions.trim() || null,
      });
      daemonStore.profiles.set(saved.profile.id, saved.profile);
      selectedProfileId = saved.profile.id;
      draft = {
        ...draft,
        id: saved.profile.id,
        name: saved.profile.name,
        handle: saved.profile.handle,
        runtime: saved.profile.runtime,
        description: saved.profile.description,
        prompt: saved.profile.prompt,
        skills: [...saved.profile.skills],
        model: saved.version.model ?? draft.model,
        default_workspace_behavior: saved.version.default_workspace_behavior,
        outbox_instructions: saved.version.outbox_instructions,
      };
      baseline = serializeDraft(draft);
    } catch (error) {
      actionError = String(error);
    } finally {
      saving = false;
    }
  }

  async function archiveProfile() {
    if (!draft?.id || !daemonStore.client) return;
    actionError = null;
    try {
      const profile = await daemonStore.client.archiveProfile(draft.id);
      daemonStore.profiles.set(profile.id, profile);
    } catch (error) {
      actionError = String(error);
    }
  }

  async function restoreProfile() {
    if (!draft?.id || !daemonStore.client) return;
    actionError = null;
    try {
      const profile = await daemonStore.client.restoreProfile(draft.id);
      daemonStore.profiles.set(profile.id, profile);
    } catch (error) {
      actionError = String(error);
    }
  }

  async function testProfile() {
    if (!draft?.id || !daemonStore.client || !canTest) return;
    actionError = null;
    try {
      await daemonStore.client.testProfileInChat(draft.id, { body: `Smoke test @${draft.handle}` });
      workspaceMode.set("chat");
    } catch (error) {
      actionError = String(error);
    }
  }
</script>

<div class="agent-create-workspace">
  <ProfileList
    {profiles}
    selectedId={selectedProfileId}
    onNewProfile={startNewProfile}
    onSelectProfile={openProfile}
  />

  <div class="workspace-main">
    {#if draft}
      <ProfileEditor
        {draft}
        {validation}
        {dirty}
        saved={Boolean(draft.id)}
        archived={selectedProfile?.archived_at !== null && selectedProfile !== null}
        {previewOpen}
        {canSave}
        {canTest}
        {clientAvailable}
        {saving}
        {actionError}
        onDraftChange={updateDraft}
        onSave={saveDraft}
        onDuplicate={duplicateDraft}
        onArchive={archiveProfile}
        onRestore={restoreProfile}
        onTest={testProfile}
        onTogglePreview={() => (previewOpen = !previewOpen)}
      />
      <ProfilePreviewDrawer
        {draft}
        {validation}
        open={previewOpen}
        onClose={() => (previewOpen = false)}
      />
    {:else}
      <section class="empty-state">
        <div class="empty-inner">
          <p class="eyebrow">Agent Profiles</p>
          <h1>No agent profiles</h1>
          <p>Profiles become available as @agent and %agent in chat.</p>
          <button type="button" onclick={startNewProfile}>New Profile</button>
        </div>
      </section>
    {/if}
  </div>
</div>

<style>
  .agent-create-workspace {
    position: relative;
    width: 100%;
    height: 100%;
    display: flex;
    background: var(--bg-void);
    color: var(--text-primary);
    overflow: hidden;
  }

  .workspace-main {
    position: relative;
    min-width: 0;
    flex: 1;
    height: 100%;
    display: flex;
    overflow: hidden;
  }

  .empty-state {
    flex: 1;
    height: 100%;
    display: grid;
    place-items: center;
    background: var(--bg-void);
  }

  .empty-inner {
    width: min(420px, calc(100% - 32px));
    display: grid;
    gap: 12px;
    border: 1px solid var(--border-default);
    border-radius: 8px;
    padding: 28px;
    background: var(--bg-base);
  }

  .eyebrow {
    color: var(--text-secondary);
    font: 11px var(--font-mono);
    text-transform: uppercase;
  }

  h1 {
    color: var(--text-primary);
    font-size: 22px;
  }

  p {
    color: var(--text-secondary);
    font-size: 14px;
    line-height: 1.5;
  }

  button {
    justify-self: start;
    border: 1px solid var(--text-emphasis);
    border-radius: 6px;
    background: var(--text-emphasis);
    color: var(--bg-void);
    padding: 9px 12px;
    font-weight: 600;
    cursor: pointer;
  }

  @media (max-width: 860px) {
    .agent-create-workspace {
      flex-direction: column;
    }

    .workspace-main {
      min-height: 0;
    }
  }
</style>
