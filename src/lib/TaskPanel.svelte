<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { focusTarget, projects, type Project, type FocusTarget } from "./stores";

  interface Props {
    visible: boolean;
  }

  let { visible }: Props = $props();

  interface GithubIssue {
    number: number;
    title: string;
    url: string;
    labels: { name: string }[];
  }

  let issues: GithubIssue[] = $state([]);

  export function insertIssue(issue: GithubIssue) {
    issues = [issue, ...issues];
  }
  let loading = $state(false);
  let error: string | null = $state(null);
  let currentRepoPath: string | null = $state(null);

  let projectList: Project[] = $state([]);
  let currentFocus: FocusTarget = $state(null);

  $effect(() => {
    const unsub = projects.subscribe((v) => { projectList = v; });
    return unsub;
  });

  $effect(() => {
    const unsub = focusTarget.subscribe((v) => { currentFocus = v; });
    return unsub;
  });

  $effect(() => {
    const projectId = currentFocus?.type === "project"
      ? currentFocus.projectId
      : currentFocus?.type === "session"
        ? currentFocus.projectId
        : null;

    const project = projectId
      ? projectList.find((p) => p.id === projectId)
      : projectList[0] ?? null;

    const repoPath = project?.repo_path ?? null;

    if (repoPath && repoPath !== currentRepoPath) {
      currentRepoPath = repoPath;
      fetchIssues(repoPath);
    }
  });

  $effect(() => {
    if (visible && currentRepoPath) {
      fetchIssues(currentRepoPath);
    }
  });

  async function fetchIssues(repoPath: string) {
    loading = true;
    error = null;
    try {
      const allIssues = await invoke<GithubIssue[]>("list_github_issues", { repoPath });
      issues = allIssues.filter(issue =>
        !issue.labels.some(l => l.name === "in-progress")
      );
    } catch (e) {
      error = String(e);
      issues = [];
    } finally {
      loading = false;
    }
  }
</script>

<aside class="task-panel">
  <div class="panel-header">GitHub Issues</div>
  {#if loading}
    <div class="status">Loading...</div>
  {:else if error}
    <div class="status error">{error}</div>
  {:else if issues.length === 0}
    <div class="status">No open issues</div>
  {:else}
    <ul class="issue-list">
      {#each issues as issue}
        <li class="issue-item">
          <span class="issue-number">#{issue.number}</span>
          <span class="issue-title">{issue.title}</span>
        </li>
      {/each}
    </ul>
  {/if}
</aside>

<style>
  .task-panel {
    width: 320px;
    min-width: 320px;
    height: 100vh;
    background: #1e1e2e;
    border-left: 1px solid #313244;
    display: flex;
    flex-direction: column;
    overflow: hidden;
  }
  .panel-header {
    padding: 12px 16px;
    font-size: 13px;
    font-weight: 600;
    color: #cdd6f4;
    border-bottom: 1px solid #313244;
  }
  .status {
    padding: 16px;
    color: #6c7086;
    font-size: 13px;
  }
  .status.error {
    color: #f38ba8;
  }
  .issue-list {
    list-style: none;
    margin: 0;
    padding: 0;
    overflow-y: auto;
    flex: 1;
  }
  .issue-item {
    padding: 8px 16px;
    border-bottom: 1px solid rgba(49, 50, 68, 0.5);
    font-size: 13px;
    display: flex;
    gap: 8px;
    align-items: baseline;
  }
  .issue-number {
    color: #89b4fa;
    font-weight: 500;
    white-space: nowrap;
  }
  .issue-title {
    color: #cdd6f4;
  }
</style>
