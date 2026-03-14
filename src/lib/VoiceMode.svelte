<script lang="ts">
  import { onMount } from "svelte";
  import { command, listen } from "$lib/backend";

  let voiceState = $state<string>("voice mode");

  const STATE_LABELS: Record<string, string> = {
    listening: "listening...",
    thinking: "thinking...",
    speaking: "speaking...",
  };

  onMount(() => {
    // Listen for state changes from Rust
    const unlisten = listen<string>("voice-state-changed", (payload) => {
      try {
        const data = JSON.parse(payload);
        if (data.state === "downloading" && data.filename) {
          const pct = data.percent != null ? ` ${data.percent}%` : "";
          voiceState = `downloading ${data.filename}${pct}`;
        } else if (data.state === "error") {
          voiceState = `error: ${data.error ?? "unknown"}`;
        } else {
          voiceState = STATE_LABELS[data.state] ?? "voice mode";
        }
      } catch {
        // Ignore malformed events
      }
    });

    // Start the pipeline
    command("start_voice_pipeline").catch((e: unknown) => {
      console.error("[voice] Failed to start pipeline:", e);
      voiceState = "error";
    });

    return () => {
      unlisten();
      // Stop the pipeline when leaving voice mode
      command("stop_voice_pipeline").catch((e: unknown) => {
        console.error("[voice] Failed to stop pipeline:", e);
      });
    };
  });
</script>

<div class="voice-mode">
  <span class="label">{voiceState}</span>
</div>

<style>
  .voice-mode {
    width: 100%;
    height: 100%;
    background: #000;
    display: flex;
    align-items: center;
    justify-content: center;
  }

  .label {
    font-size: 14px;
    color: rgba(255, 255, 255, 0.3);
    font-family: var(--font-mono);
    letter-spacing: 0.05em;
  }
</style>
