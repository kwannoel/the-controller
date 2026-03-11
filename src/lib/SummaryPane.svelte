<script lang="ts">
  import { fromStore } from "svelte/store";
  import { command, listen } from "$lib/backend";
  import { projects, sessionStatuses, type Project, type SessionStatus } from "./stores";
  import TokenChart from "./TokenChart.svelte";

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
  interface TokenDataPoint {
    timestamp: string;
    input_tokens: number;
    output_tokens: number;
    cache_read_tokens: number;
    cache_write_tokens: number;
  }

  let tokenData: TokenDataPoint[] = $state([]);

  let session = $derived(
    projectList.flatMap((p) =>
      p.sessions.map((s) => ({ ...s, projectId: p.id }))
    ).find((s) => s.id === sessionId) ?? null
  );

  let status = $derived(statuses.get(sessionId) ?? "idle");

  function fetchCommits() {
    if (!session) return;
    command<CommitInfo[]>("get_session_commits", {
      projectId: session.projectId,
      sessionId: session.id,
    }).then((result) => {
      commits = result;
    }).catch(() => {
      // Ignore errors (e.g., no repo yet)
    });
  }

  function fetchTokenUsage() {
    if (!session) return;
    command<TokenDataPoint[]>("get_session_token_usage", {
      projectId: session.projectId,
      sessionId: session.id,
    }).then((result) => {
      tokenData = result;
    }).catch(() => {
      // Ignore errors (e.g., no session files yet)
    });
  }

  // Fetch commits and tokens on mount and when session changes
  $effect(() => {
    if (session) {
      fetchCommits();
      fetchTokenUsage();
    }
  });

  // Refresh when session transitions to idle (likely just committed)
  let prevStatus: SessionStatus | null = $state(null);
  $effect(() => {
    if (prevStatus === "working" && status === "idle") {
      // Delay slightly to let git/files finish writing
      setTimeout(() => {
        fetchCommits();
        fetchTokenUsage();
      }, 1000);
    }
    prevStatus = status;
  });

  // Also listen for status hook events to catch transitions
  $effect(() => {
    const unlisten = listen<string>(`session-status-hook:${sessionId}`, (payload) => {
      if (payload === "idle") {
        setTimeout(() => {
          fetchCommits();
          fetchTokenUsage();
        }, 1000);
      }
    });

    return () => { unlisten(); };
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
    <TokenChart dataPoints={tokenData} />
  </div>
{/if}

<style>
  .summary-pane {
    display: flex;
    flex-direction: column;
    gap: 6px;
    padding: 12px 18px;
    background: var(--bg-base);
    border-bottom: 1px solid var(--border-subtle);
    font-size: 18px;
    flex-shrink: 0;
    max-height: 280px;
    overflow: hidden;
  }

  .summary-row {
    display: flex;
    align-items: baseline;
    gap: 12px;
    min-width: 0;
  }

  .progress-row {
    align-items: flex-start;
    overflow-y: auto;
    min-height: 0;
    scrollbar-width: none;
  }

  .progress-row::-webkit-scrollbar {
    display: none;
  }

  .label {
    color: var(--text-secondary);
    font-weight: 600;
    font-size: 15px;
    letter-spacing: 0.75px;
    flex-shrink: 0;
    width: 63px;
  }

  .value {
    color: var(--text-primary);
    min-width: 0;
  }

  .prompt-text {
    white-space: normal;
    word-wrap: break-word;
  }

  .muted {
    color: var(--text-secondary);
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
    color: var(--text-emphasis);
    font-family: var(--font-mono);
    font-size: 16px;
    flex-shrink: 0;
  }

  .commit-msg {
    color: var(--text-primary);
    white-space: normal;
    word-wrap: break-word;
  }
</style>
