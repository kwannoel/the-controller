<script lang="ts">
  let { text }: { text: string } = $props();
</script>

<div class="user">{text}</div>

<style>
  .user {
    padding: 6px 10px;
    margin-bottom: 6px;
    border-radius: 6px;
    background: rgba(137, 180, 250, 0.15);
    text-align: right;
  }
</style>
