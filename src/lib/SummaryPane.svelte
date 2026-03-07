<script lang="ts">
  import { fromStore } from "svelte/store";
  import { invoke } from "@tauri-apps/api/core";
  import { listen, type UnlistenFn } from "@tauri-apps/api/event";
  import { projects, sessionStatuses, type Project, type SessionStatus } from "./stores";

  interface Props {
    sessionId: string;
  }

  let { sessionId }: Props = $props();

  interface CommitInfo {
    hash: string;
    message: string;
  }

  const projectsState = fromStore(projects);
  let projectList: Project[] = $derived(projectsState.current);
  const sessionStatusesState = fromStore(sessionStatuses);
  let statuses: Map<string, SessionStatus> = $derived(sessionStatusesState.current);
  let commits: CommitInfo[] = $state([]);

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
    gap: 6px;
    padding: 12px 18px;
    background: #181825;
    border-bottom: 1px solid #313244;
    font-size: 18px;
    flex-shrink: 0;
    max-height: 540px;
    overflow-y: auto;
  }

  .summary-row {
    display: flex;
    align-items: baseline;
    gap: 12px;
    min-width: 0;
  }

  .progress-row {
    align-items: flex-start;
  }

  .label {
    color: #6c7086;
    font-weight: 600;
    font-size: 15px;
    letter-spacing: 0.75px;
    flex-shrink: 0;
    width: 63px;
  }

  .value {
    color: #cdd6f4;
    min-width: 0;
  }

  .prompt-text {
    white-space: normal;
    word-wrap: break-word;
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
    gap: 9px;
    line-height: 1.5;
  }

  .commit-hash {
    color: #89b4fa;
    font-family: monospace;
    font-size: 16px;
    flex-shrink: 0;
  }

  .commit-msg {
    color: #cdd6f4;
    white-space: normal;
    word-wrap: break-word;
  }
</style>
