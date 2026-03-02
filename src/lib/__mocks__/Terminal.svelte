<script lang="ts">
  let { sessionId }: { sessionId: string } = $props();

  export function focus() {}
</script>

<div data-testid="mock-terminal" data-session={sessionId}></div>
