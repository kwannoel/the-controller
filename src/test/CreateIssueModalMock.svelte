<script lang="ts">
  type Priority = "high" | "low";
  type Complexity = "high" | "low";

  let {
    onSubmit,
    onClose,
  }: {
    onSubmit: (title: string, priority: Priority, complexity: Complexity) => void;
    onClose: () => void;
  } = $props();
</script>

<button
  type="button"
  data-testid="mock-create-issue-submit"
  onclick={() => onSubmit("Mock issue", "low", "low")}
>
  Submit issue
</button>
<button type="button" data-testid="mock-create-issue-close" onclick={onClose}>Close issue modal</button>
