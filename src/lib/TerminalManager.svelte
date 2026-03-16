<script lang="ts">
  import { fromStore } from "svelte/store";
  import Terminal from "./Terminal.svelte";
  import SummaryPane from "./SummaryPane.svelte";
  import { projects, activeSessionId, hotkeyAction, focusTarget, type Project } from "./stores";

  const projectsState = fromStore(projects);
  let projectList: Project[] = $derived(projectsState.current);
  const activeSessionIdState = fromStore(activeSessionId);
  let activeSession: string | null = $derived(activeSessionIdState.current);
  let terminalComponents: Record<string, Terminal> = $state({});
  let allSessionIds: Set<string> = $derived(
    new Set(projectList.flatMap((p) => p.sessions.map((s) => s.id))),
  );

  $effect(() => {
    const unsub = hotkeyAction.subscribe((action) => {
      if (action?.type === "focus-terminal" && activeSession) {
        terminalComponents[activeSession]?.focus();
      }
    });
    return unsub;
  });

  const focusTargetState = fromStore(focusTarget);
  let isFocused = $derived(focusTargetState.current?.type === "terminal");
  let focusedSessionId: string | null = $derived(
    focusTargetState.current?.type === "session" ? focusTargetState.current.sessionId : null,
  );

  function handleFocusIn() {
    const project = activeSession
      ? projectList.find((p) => p.sessions.some((s) => s.id === activeSession))
      : null;
    if (project) {
      focusTarget.set({ type: "terminal", projectId: project.id });
    }
  }

  let allSessions: { id: string; kind: string }[] = $derived(
    projectList.flatMap((p) => p.sessions.map((s) => ({ id: s.id, kind: s.kind }))),
  );

</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="terminal-manager" class:focused={isFocused} onfocusin={handleFocusIn}>
  {#each allSessions as session (session.id)}
    {@const sessionId = session.id}
    <div class="terminal-wrapper" class:visible={activeSession === sessionId}>
      {#if focusedSessionId === sessionId}
        <SummaryPane {sessionId} />
      {/if}
      <div class="terminal-inner">
        <Terminal {sessionId} kind={session.kind} bind:this={terminalComponents[sessionId]} />
      </div>
    </div>
  {/each}

  {#if !activeSession}
    <div class="empty-state">
      <div class="empty-content">
        <div class="empty-title">No active session</div>
        <div class="empty-hint">press <kbd>c</kbd> to create a session, or <kbd>n</kbd> to add a project</div>
      </div>
    </div>
  {/if}
</div>

<style>
  .terminal-manager {
    width: 100%;
    height: 100%;
    position: relative;
  }

  .terminal-manager.focused {
    outline: 2px solid var(--focus-ring);
    outline-offset: -2px;
  }

  .terminal-wrapper {
    position: absolute;
    inset: 0;
    display: none;
    flex-direction: column;
  }

  .terminal-wrapper.visible {
    display: flex;
  }

  .terminal-inner {
    flex: 1;
    min-height: 0;
    overflow: hidden;
  }

  .empty-state {
    display: flex;
    align-items: center;
    justify-content: center;
    height: 100%;
  }

  .empty-content {
    text-align: center;
  }

  .empty-title {
    color: var(--text-primary);
    font-size: 16px;
    font-weight: 500;
    margin-bottom: 8px;
  }

  .empty-hint {
    color: var(--text-secondary);
    font-size: 13px;
  }

  .empty-hint kbd {
    background: var(--bg-hover);
    color: var(--text-emphasis);
    padding: 1px 6px;
    border-radius: 3px;
    font-family: var(--font-mono);
    font-size: 12px;
  }
</style>
