<script lang="ts">
  import { onMount } from "svelte";
  import { get } from "svelte/store";
  import { fromStore } from "svelte/store";
  import { command, listen } from "$lib/backend";
  import {
    activeNote,
    controllerChatSession,
    focusTarget,
    noteEntries,
    projects,
    workspaceMode,
    type ControllerChatSession,
    type NoteEntry,
    type Project,
    type FocusTarget,
    type WorkspaceMode,
  } from "./stores";

  const EMPTY_SESSION: ControllerChatSession = {
    focus: {
      project_id: null,
      project_name: null,
      session_id: null,
      note_filename: null,
      workspace_mode: null,
    },
    items: [],
    turn_in_progress: false,
  };

  const controllerChatSessionState = fromStore(controllerChatSession);
  const focusTargetState = fromStore(focusTarget);
  const projectsState = fromStore(projects);
  const workspaceModeState = fromStore(workspaceMode);

  let session: ControllerChatSession = $derived(controllerChatSessionState.current ?? EMPTY_SESSION);
  let currentFocus: FocusTarget = $derived(focusTargetState.current);
  let projectList: Project[] = $derived(projectsState.current);
  let currentWorkspaceMode: WorkspaceMode = $derived(workspaceModeState.current);
  let draft = $state("");
  let syncKey = $state("");

  function parsePayload<T>(payload: T | string): T {
    return typeof payload === "string" ? JSON.parse(payload) as T : payload;
  }

  function focusSnapshot() {
    if (!currentFocus || !("projectId" in currentFocus)) return null;
    const project = projectList.find((entry) => entry.id === currentFocus.projectId);
    if (!project) return null;

    let noteFilename: string | null = null;
    if (currentFocus.type === "note") {
      noteFilename = currentFocus.filename;
    } else if (currentFocus.type === "notes-editor") {
      const note = get(activeNote);
      if (note?.projectId === currentFocus.projectId) {
        noteFilename = note.filename;
      }
    }

    return {
      projectId: currentFocus.projectId,
      projectName: project.name,
      sessionId: currentFocus.type === "session" ? currentFocus.sessionId : null,
      noteFilename,
      workspaceMode: currentWorkspaceMode,
    };
  }

  async function refreshProjectNotes(projectId: string) {
    const project = get(projects).find((entry) => entry.id === projectId);
    if (!project) return;
    const notes = await command<NoteEntry[]>("list_notes", { projectName: project.name });
    noteEntries.update((entries) => {
      const next = new Map(entries);
      next.set(projectId, notes);
      return next;
    });
  }

  async function submitMessage() {
    const message = draft.trim();
    if (!message || session.turn_in_progress) return;

    draft = "";
    const nextSession = await command<ControllerChatSession>("send_controller_chat_message", {
      message,
    });
    if (nextSession) {
      controllerChatSession.set(nextSession);
    }
  }

  $effect(() => {
    const snapshot = focusSnapshot();
    if (!snapshot) return;

    const nextKey = JSON.stringify(snapshot);
    if (nextKey === syncKey) return;
    syncKey = nextKey;

    command<ControllerChatSession>("update_controller_chat_focus", snapshot)
      .then((nextSession) => {
        if (nextSession) {
          controllerChatSession.set(nextSession);
        }
      })
      .catch(() => {});
  });

  onMount(() => {
    command<ControllerChatSession>("get_controller_chat_session")
      .then((nextSession) => {
        if (nextSession) {
          controllerChatSession.set(nextSession);
        }
      })
      .catch(() => {});

    const unlistenSession = listen<string>("controller-chat-session-updated", (payload) => {
      controllerChatSession.set(parsePayload<ControllerChatSession>(payload));
    });

    const unlistenNoteOpened = listen<string>("controller-chat-note-opened", (payload) => {
      const noteEvent = parsePayload<{ project_id: string; filename: string }>(payload);
      workspaceMode.set("notes");
      activeNote.set({ projectId: noteEvent.project_id, filename: noteEvent.filename });
      focusTarget.set({ type: "notes-editor", projectId: noteEvent.project_id });
      refreshProjectNotes(noteEvent.project_id).catch(() => {});
    });

    return () => {
      unlistenSession();
      unlistenNoteOpened();
    };
  });
</script>

<aside class="global-chat" data-testid="global-chat">
  <header class="chat-header">
    <div class="label">Controller Chat</div>
    <div class="focus" data-testid="controller-chat-focus">
      {#if session.focus.project_name}
        <span>{session.focus.project_name}</span>
        {#if session.focus.note_filename}
          <span> / {session.focus.note_filename}</span>
        {/if}
      {:else}
        <span>No focused project</span>
      {/if}
    </div>
  </header>

  <div class="transcript" data-testid="controller-chat-transcript">
    {#if session.items.length === 0}
      <div class="empty">Ask the controller to work with the focused project.</div>
    {:else}
      {#each session.items as item, index}
        <div class={`item item-${item.kind}`} data-testid={`controller-chat-item-${index}`}>
          <span class="kind">{item.kind}</span>
          <span class="text">{item.text}</span>
        </div>
      {/each}
    {/if}
  </div>

  <form
    class="composer"
    onsubmit={(event) => {
      event.preventDefault();
      submitMessage();
    }}
  >
    <textarea
      bind:value={draft}
      rows="4"
      placeholder="Fetch an issue into notes, summarize a file, or update the current note..."
      data-testid="controller-chat-input"
    ></textarea>
    <button type="submit" disabled={session.turn_in_progress}>
      {session.turn_in_progress ? "Working..." : "Send"}
    </button>
  </form>
</aside>

<style>
  .global-chat {
    width: 24rem;
    border-left: 1px solid #313244;
    background: #181825;
    color: #cdd6f4;
    display: flex;
    flex-direction: column;
    min-width: 0;
  }

  .chat-header {
    padding: 0.9rem 1rem;
    border-bottom: 1px solid #313244;
  }

  .label {
    font-size: 0.75rem;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: #89b4fa;
    margin-bottom: 0.25rem;
  }

  .focus {
    font-size: 0.9rem;
    color: #bac2de;
    min-height: 1.3rem;
  }

  .transcript {
    flex: 1;
    overflow: auto;
    padding: 0.85rem;
    display: flex;
    flex-direction: column;
    gap: 0.6rem;
  }

  .empty {
    color: #7f849c;
    font-size: 0.95rem;
  }

  .item {
    display: grid;
    gap: 0.2rem;
    padding: 0.65rem 0.75rem;
    border-radius: 0.75rem;
    background: #1e1e2e;
  }

  .item-user {
    border: 1px solid #45475a;
  }

  .item-tool {
    border: 1px solid #45475a;
    background: #11111b;
  }

  .item-assistant {
    border: 1px solid #89b4fa33;
  }

  .kind {
    font-size: 0.7rem;
    text-transform: uppercase;
    letter-spacing: 0.08em;
    color: #89b4fa;
  }

  .text {
    font-size: 0.95rem;
    white-space: pre-wrap;
    word-break: break-word;
  }

  .composer {
    display: grid;
    gap: 0.75rem;
    padding: 0.85rem;
    border-top: 1px solid #313244;
  }

  textarea {
    width: 100%;
    resize: none;
    border: 1px solid #45475a;
    border-radius: 0.75rem;
    background: #11111b;
    color: #cdd6f4;
    padding: 0.75rem;
    font: inherit;
  }

  button {
    justify-self: end;
    border: none;
    border-radius: 999px;
    padding: 0.55rem 1rem;
    background: #89b4fa;
    color: #11111b;
    font: inherit;
    font-weight: 600;
    cursor: pointer;
  }

  button:disabled {
    opacity: 0.6;
    cursor: default;
  }
</style>
