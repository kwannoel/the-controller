<script lang="ts">
  import { onMount, onDestroy } from "svelte";
  import { Terminal } from "@xterm/xterm";
  import { FitAddon } from "@xterm/addon-fit";
  import { invoke } from "@tauri-apps/api/core";
  import { listen, type UnlistenFn } from "@tauri-apps/api/event";
  import "@xterm/xterm/css/xterm.css";

  interface Props {
    sessionId: string;
  }

  let { sessionId }: Props = $props();

  let containerEl: HTMLDivElement | undefined = $state();
  let term: Terminal | undefined;
  let fitAddon: FitAddon | undefined;
  let resizeObserver: ResizeObserver | undefined;
  let mutationObserver: MutationObserver | undefined;
  let unlistenOutput: UnlistenFn | undefined;
  let unlistenStatus: UnlistenFn | undefined;

  onMount(() => {
    if (!containerEl) return;

    term = new Terminal({
      cursorBlink: true,
      fontSize: 13,
      fontFamily: "'JetBrains Mono', 'Fira Code', monospace",
      theme: {
        background: "#11111b",
        foreground: "#cdd6f4",
        cursor: "#f5e0dc",
        selectionBackground: "#45475a",
      },
    });

    fitAddon = new FitAddon();
    term.loadAddon(fitAddon);
    term.open(containerEl);
    fitAddon.fit();

    // Handle keys that xterm.js doesn't natively support
    term.attachCustomKeyEventHandler((event) => {
      if (event.type !== "keydown") return true;

      // Cmd-V (macOS) / Ctrl-V (Linux/Windows): paste from clipboard
      if (event.key === "v" && (event.metaKey || event.ctrlKey)) {
        navigator.clipboard.readText().then((text) => {
          if (text) {
            invoke("write_to_pty", { sessionId, data: text });
          }
        });
        return false;
      }

      // Shift-Enter: send CSI u sequence so Claude Code can distinguish it from Enter
      if (event.key === "Enter" && event.shiftKey) {
        invoke("write_to_pty", { sessionId, data: "\x1b[13;2u" });
        return false;
      }

      return true;
    });

    // Connect user input to PTY
    term.onData((data: string) => {
      invoke("write_to_pty", { sessionId, data }).catch((err) => {
        console.error("Failed to write to PTY:", err);
      });
    });

    // Listen for PTY output (base64-encoded)
    listen<string>(`pty-output:${sessionId}`, (event) => {
      if (term) {
        const bytes = Uint8Array.from(atob(event.payload), (c) =>
          c.charCodeAt(0),
        );
        term.write(bytes);
      }
    }).then((fn) => {
      unlistenOutput = fn;
    });

    // Listen for session status changes
    listen<string>(`session-status-changed:${sessionId}`, () => {
      if (term) {
        term.writeln("\r\n\x1b[90m[Session ended]\x1b[0m");
      }
    }).then((fn) => {
      unlistenStatus = fn;
    });

    // Handle resize
    resizeObserver = new ResizeObserver(() => {
      if (fitAddon && term) {
        fitAddon.fit();
        invoke("resize_pty", {
          sessionId,
          rows: term.rows,
          cols: term.cols,
        }).catch((err) => {
          console.error("Failed to resize PTY:", err);
        });
      }
    });
    resizeObserver.observe(containerEl);

    // Refit when becoming visible (display: none -> block doesn't trigger ResizeObserver)
    mutationObserver = new MutationObserver(() => {
      if (containerEl && containerEl.offsetParent !== null && fitAddon) {
        fitAddon.fit();
      }
    });
    if (containerEl?.parentElement) {
      mutationObserver.observe(containerEl.parentElement, {
        attributes: true,
        attributeFilter: ["class"],
      });
    }
  });

  export function focus() {
    term?.focus();
  }

  onDestroy(() => {
    unlistenOutput?.();
    unlistenStatus?.();
    resizeObserver?.disconnect();
    mutationObserver?.disconnect();
    term?.dispose();
  });
</script>

<div class="terminal-container" bind:this={containerEl}></div>

<style>
  .terminal-container {
    width: 100%;
    height: 100%;
    padding: 4px;
    box-sizing: border-box;
  }

  .terminal-container :global(.xterm) {
    width: 100%;
    height: 100%;
  }
</style>
