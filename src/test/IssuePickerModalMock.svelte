<script lang="ts">
  import type { GithubIssue } from "../lib/stores";

  let {
    onSelect,
    onSkip,
  }: {
    onSelect: (issue: GithubIssue) => void;
    onSkip: () => void;
  } = $props();

  const issue: GithubIssue = {
    number: 42,
    title: "Mock issue",
    url: "https://example.com/issues/42",
    body: "Mock issue body",
    labels: [],
  };
</script>

<button type="button" data-testid="mock-issue-select" onclick={() => onSelect(issue)}>Select issue</button>
<button type="button" data-testid="mock-issue-skip" onclick={onSkip}>Skip issue</button>
