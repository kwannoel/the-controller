<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { listen, type UnlistenFn } from "@tauri-apps/api/event";
  import { onDestroy } from "svelte";
  import { projects, sessionStatuses, type Project, type SessionStatus } from "./stores";

  interface Props {
    sessionId: string;
  }

  let { sessionId }: Props = $props();

  interface CommitInfo {
    hash: string;
    message: string;
  }

  let projectList: Project[] = $state([]);
  let statuses: Map<string, SessionStatus> = $state(new Map());
  let commits: CommitInfo[] = $state([]);

  $effect(() => {
    const unsub = projects.subscribe((v) => { projectList = v; });
    return unsub;
  });

  $effect(() => {
    const unsub = sessionStatuses.subscribe((v) => { statuses = v; });
    return unsub;
  });

  let session = $derived(
    projectList.flatMap((p) =>
      p.sessions.map((s) => ({ ...s, projectId: p.id }))
    ).find((s) => s.id === sessionId) ?? null
  );

  let status = $derived(statuses.get(sessionId) ?? "idle");

  function fetchCommits() {
    if (!session) return;
    invoke<CommitInfo[]>("get_session_commits", {
      projectId: session.projectId,
      sessionId: session.id,
    }).then((result) => {
      commits = result;
    }).catch(() => {
      // Ignore errors (e.g., no repo yet)
    });
  }

  // Fetch commits on mount and when session changes
  $effect(() => {
    if (session) fetchCommits();
  });

  // Refresh commits when session transitions to idle (likely just committed)
  let prevStatus: SessionStatus | null = $state(null);
  $effect(() => {
    if (prevStatus === "working" && status === "idle") {
      // Delay slightly to let git finish writing
      setTimeout(fetchCommits, 1000);
    }
    prevStatus = status;
  });

  // Also listen for status hook events to catch transitions
  let unlistenHook: UnlistenFn | undefined;
  $effect(() => {
    listen<string>(`session-status-hook:${sessionId}`, (event) => {
      if (event.payload === "idle") {
        setTimeout(fetchCommits, 1000);
      }
    }).then((fn) => { unlistenHook = fn; });

    return () => { unlistenHook?.(); };
  });
</script>

{#if session}
  <div class="summary-pane">
    <div class="summary-row">
      <span class="label">PROMPT</span>
      <span class="value prompt-text">
        {#if session.github_issue}
          #{session.github_issue.number}: {session.github_issue.title}
        {:else if session.initial_prompt}
          {session.initial_prompt}
        {:else}
          <span class="muted">No prompt</span>
        {/if}
      </span>
    </div>
    <div class="summary-row progress-row">
      <span class="label">DONE</span>
      {#if commits.length > 0}
        <ul class="commit-list">
          {#each commits as commit (commit.hash)}
            <li class="commit-item">
              <span class="commit-hash">{commit.hash}</span>
              <span class="commit-msg">{commit.message}</span>
            </li>
          {/each}
        </ul>
      {:else}
        <span class="muted">No commits yet</span>
      {/if}
    </div>
  </div>
{/if}

<style>
  .summary-pane {
    display: flex;
    flex-direction: column;
    gap: 4px;
    padding: 8px 12px;
    background: #181825;
    border-bottom: 1px solid #313244;
    font-size: 12px;
    flex-shrink: 0;
    max-height: 120px;
    overflow-y: auto;
  }

  .summary-row {
    display: flex;
    align-items: baseline;
    gap: 8px;
    min-width: 0;
  }

  .progress-row {
    align-items: flex-start;
  }

  .label {
    color: #6c7086;
    font-weight: 600;
    font-size: 10px;
    letter-spacing: 0.5px;
    flex-shrink: 0;
    width: 42px;
  }

  .value {
    color: #cdd6f4;
    min-width: 0;
  }

  .prompt-text {
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .muted {
    color: #6c7086;
    font-style: italic;
  }

  .commit-list {
    list-style: none;
    margin: 0;
    padding: 0;
    min-width: 0;
  }

  .commit-item {
    display: flex;
    align-items: baseline;
    gap: 6px;
    line-height: 1.5;
  }

  .commit-hash {
    color: #89b4fa;
    font-family: monospace;
    font-size: 11px;
    flex-shrink: 0;
  }

  .commit-msg {
    color: #cdd6f4;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
</style>
