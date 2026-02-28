<script lang="ts">
  import Sidebar from "./lib/Sidebar.svelte";
  import TerminalManager from "./lib/TerminalManager.svelte";
  import Toast from "./lib/Toast.svelte";
</script>

<div class="app-layout">
  <Sidebar />
  <main class="terminal-area">
    <TerminalManager />
  </main>
</div>
<Toast />

<style>
  .app-layout {
    display: flex;
    height: 100vh;
    width: 100vw;
    overflow: hidden;
  }

  .terminal-area {
    flex: 1;
    display: flex;
    background: #11111b;
    color: #cdd6f4;
    overflow: hidden;
  }
</style>
