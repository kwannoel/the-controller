<script lang="ts">
  import type { GithubIssue } from "../lib/stores";

  type Priority = "high" | "low";
  type Complexity = "high" | "low";

  let {
    onCreateIssue,
    onAssignIssue,
    onClose,
  }: {
    repoPath: string;
    projectId: string;
    onCreateIssue: (title: string, priority: Priority, complexity: Complexity) => void;
    onAssignIssue: (issue: GithubIssue) => void;
    onClose: () => void;
  } = $props();

  const mockIssue: GithubIssue = {
    number: 42,
    title: "Mock issue",
    url: "https://example.com/issues/42",
    body: "Mock issue body",
    labels: [],
  };
</script>

<div role="dialog">
  <button type="button" data-testid="mock-create-issue-submit" onclick={() => onCreateIssue("Mock issue", "low", "low")}>
    Submit issue
  </button>
  <button type="button" data-testid="mock-issue-assign" onclick={() => onAssignIssue(mockIssue)}>
    Assign issue
  </button>
  <button type="button" data-testid="mock-issues-close" onclick={onClose}>Close</button>
</div>
