<script lang="ts">
  import { onMount } from "svelte";
  import { command, listen } from "$lib/backend";

  let voiceState = $state<string>("voice mode");
  let showDebug = $state(false);
  let showTranscript = $state(false);
  let debugLog = $state<string[]>([]);
  let transcript = $state<{ role: string; text: string }[]>([]);

  const MAX_DEBUG_LINES = 200;
  const MAX_TRANSCRIPT_ENTRIES = 200;

  const STATE_LABELS: Record<string, string> = {
    listening: "listening...",
    thinking: "thinking...",
    speaking: "speaking...",
  };

  export function toggleDebug() {
    showDebug = !showDebug;
  }

  export function toggleTranscript() {
    showTranscript = !showTranscript;
  }

  let debugEl: HTMLElement | undefined = $state();
  let transcriptEl: HTMLElement | undefined = $state();

  $effect(() => {
    if (debugEl && debugLog.length) {
      debugEl.scrollTop = debugEl.scrollHeight;
    }
  });

  $effect(() => {
    if (transcriptEl && transcript.length) {
      transcriptEl.scrollTop = transcriptEl.scrollHeight;
    }
  });

  onMount(() => {
    const unlistenState = listen<string>("voice-state-changed", (payload) => {
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

    const unlistenDebug = listen<string>("voice-debug", (payload) => {
      try {
        const data = JSON.parse(payload);
        const line = `[${data.ts}] ${data.msg}`;
        debugLog = [...debugLog.slice(-(MAX_DEBUG_LINES - 1)), line];
      } catch {
        // Ignore
      }
    });

    const unlistenTranscript = listen<string>("voice-transcript", (payload) => {
      try {
        const data = JSON.parse(payload);
        transcript = [...transcript, { role: data.role, text: data.text }].slice(-MAX_TRANSCRIPT_ENTRIES);
      } catch {
        // Ignore
      }
    });

    command("start_voice_pipeline").catch((e: unknown) => {
      console.error("[voice] Failed to start pipeline:", e);
      voiceState = "error";
    });

    return () => {
      unlistenState();
      unlistenDebug();
      unlistenTranscript();
      command("stop_voice_pipeline").catch((e: unknown) => {
        console.error("[voice] Failed to stop pipeline:", e);
      });
    };
  });
</script>

<div class="voice-mode">
  {#if showDebug}
    <div class="panel debug-panel" bind:this={debugEl}>
      <div class="panel-header">debug</div>
      <div class="panel-content">
        {#each debugLog as line}
          <div class="log-line">{line}</div>
        {/each}
      </div>
    </div>
  {/if}

  <div class="center">
    <span class="label">{voiceState}</span>
  </div>

  {#if showTranscript}
    <div class="panel transcript-panel" bind:this={transcriptEl}>
      <div class="panel-header">transcript</div>
      <div class="panel-content">
        {#each transcript as entry}
          <div class="transcript-entry" class:user={entry.role === "user"} class:assistant={entry.role === "assistant"}>
            <span class="role">{entry.role === "user" ? "You" : "AI"}:</span>
            <span class="text">{entry.text}</span>
          </div>
        {/each}
      </div>
    </div>
  {/if}
</div>

<style>
  .voice-mode {
    width: 100%;
    height: 100%;
    background: #000;
    display: flex;
    overflow: hidden;
  }

  .center {
    flex: 1;
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

  .panel {
    width: 300px;
    display: flex;
    flex-direction: column;
    overflow-y: auto;
  }

  .debug-panel {
    border-right: 1px solid rgba(255, 255, 255, 0.1);
  }

  .transcript-panel {
    border-left: 1px solid rgba(255, 255, 255, 0.1);
  }

  .panel-header {
    font-size: 11px;
    color: rgba(255, 255, 255, 0.2);
    font-family: var(--font-mono);
    padding: 8px 12px;
    letter-spacing: 0.1em;
    text-transform: uppercase;
  }

  .panel-content {
    flex: 1;
    padding: 0 12px 12px;
    overflow-y: auto;
  }

  .log-line {
    font-size: 11px;
    color: rgba(255, 255, 255, 0.4);
    font-family: var(--font-mono);
    line-height: 1.6;
    white-space: nowrap;
  }

  .transcript-entry {
    font-size: 13px;
    color: rgba(255, 255, 255, 0.5);
    font-family: var(--font-mono);
    margin-bottom: 12px;
    line-height: 1.5;
  }

  .transcript-entry.user .role {
    color: rgba(137, 220, 235, 0.7);
  }

  .transcript-entry.assistant .role {
    color: rgba(203, 166, 247, 0.7);
  }

  .role {
    font-weight: 600;
    margin-right: 6px;
  }

  .text {
    color: rgba(255, 255, 255, 0.6);
  }
</style>
