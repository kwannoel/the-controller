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
  <header class="chat-header" data-testid="controller-chat-focus">
    {#if session.focus.project_name}
      <span class="focus-project">{session.focus.project_name}</span>
      {#if session.focus.note_filename}
        <span class="focus-separator">/</span>
        <span class="focus-note">{session.focus.note_filename}</span>
      {/if}
    {:else}
      <span class="focus-empty">No focus</span>
    {/if}
  </header>

  <div class="transcript" data-testid="controller-chat-transcript">
    {#if session.items.length === 0}
      <div class="empty">No messages yet</div>
    {:else}
      {#each session.items as item, index}
        <div class={`item item-${item.kind}`} data-testid={`controller-chat-item-${index}`}>
          {item.text}
        </div>
      {/each}
    {/if}
  </div>

  <div class="composer">
    <textarea
      bind:value={draft}
      rows="2"
      placeholder="Ask the controller..."
      disabled={session.turn_in_progress}
      data-testid="controller-chat-input"
      onkeydown={(e) => {
        if (e.key === "Enter" && !e.shiftKey) {
          e.preventDefault();
          submitMessage();
        }
      }}
    ></textarea>
    {#if session.turn_in_progress}
      <span class="working-indicator">working...</span>
    {/if}
  </div>
</aside>

<style>
  .global-chat {
    width: 280px;
    min-width: 280px;
    border-left: 1px solid #313244;
    background: #1e1e2e;
    color: #cdd6f4;
    display: flex;
    flex-direction: column;
  }

  .chat-header {
    padding: 12px 16px;
    border-bottom: 1px solid #313244;
    font-size: 14px;
    font-weight: 600;
    display: flex;
    align-items: center;
    gap: 4px;
    min-height: 20px;
  }

  .focus-project {
    color: #cdd6f4;
  }

  .focus-separator {
    color: #6c7086;
  }

  .focus-note {
    color: #bac2de;
  }

  .focus-empty {
    color: #6c7086;
  }

  .transcript {
    flex: 1;
    overflow-y: auto;
    padding: 8px;
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .empty {
    color: #6c7086;
    font-size: 13px;
    text-align: center;
    margin-top: 24px;
  }

  .item {
    padding: 8px 12px;
    font-size: 13px;
    white-space: pre-wrap;
    word-break: break-word;
    border-left: 3px solid transparent;
  }

  .item-user {
    border-left-color: #89b4fa;
  }

  .item-assistant {
    border-left-color: #a6e3a1;
  }

  .item-tool {
    border-left-color: #f9e2af;
  }

  .composer {
    padding: 8px;
    border-top: 1px solid #313244;
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  textarea {
    width: 100%;
    resize: none;
    border: 1px solid #45475a;
    border-radius: 4px;
    background: #11111b;
    color: #cdd6f4;
    padding: 8px 10px;
    font: inherit;
    font-size: 13px;
    box-sizing: border-box;
  }

  textarea:disabled {
    opacity: 0.6;
  }

  .working-indicator {
    font-size: 11px;
    color: #6c7086;
    padding: 0 2px;
  }
</style>
