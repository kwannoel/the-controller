<script lang="ts">
  import type { FocusTarget, Project } from "../stores";

  interface Props {
    projects: Project[];
    currentFocus: FocusTarget;
    onProjectFocus: (projectId: string) => void;
  }

  let { projects, currentFocus, onProjectFocus }: Props = $props();
</script>

{#each projects as project (project.id)}
  <div class="project-item">
    <!-- svelte-ignore a11y_no_noninteractive_tabindex -->
    <div
      class="project-header"
      class:focus-target={currentFocus?.type === "project" && currentFocus.projectId === project.id}
      tabindex="0"
      data-project-id={project.id}
      onfocusin={(e: FocusEvent) => {
        if (e.target === e.currentTarget) onProjectFocus(project.id);
      }}
    >
      <span class="project-name">{project.name}</span>
    </div>
  </div>
{/each}

{#if projects.length === 0}
  <div class="empty">No projects</div>
{/if}

<style>
  .project-item {
    border-bottom: 1px solid var(--border-default);
  }

  .project-header {
    display: flex;
    align-items: center;
    padding: 8px 16px;
  }

  .project-header:hover {
    background: var(--bg-hover);
  }

  .project-header.focus-target {
    outline: 2px solid var(--focus-ring);
    outline-offset: -2px;
    border-radius: 4px;
  }

  .project-name {
    flex: 1;
    font-size: 13px;
    font-weight: 500;
    word-break: break-word;
  }

  .empty {
    padding: 16px;
    color: var(--text-secondary);
    font-size: 13px;
    text-align: center;
  }
</style>
