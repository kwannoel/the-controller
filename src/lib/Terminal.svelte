<script lang="ts">
  import { onMount, onDestroy } from "svelte";
  import { Terminal } from "@xterm/xterm";
  import { FitAddon } from "@xterm/addon-fit";
  import { invoke } from "@tauri-apps/api/core";
  import { listen, type UnlistenFn } from "@tauri-apps/api/event";
  import { makeCustomKeyHandler } from "./terminal-keys";
  import { clipboardHasImage } from "./clipboard";
  import { activeSessionId, projects, type Project } from "./stores";
  import "@xterm/xterm/css/xterm.css";

  interface Props {
    sessionId: string;
  }

  let { sessionId }: Props = $props();

  let containerEl: HTMLDivElement | undefined = $state();
  let term: Terminal | undefined;
  let fitAddon: FitAddon | undefined;
  let termOpened = false; // tracks whether term.open() produced valid measurements
  let resizeObserver: ResizeObserver | undefined;
  let mutationObserver: MutationObserver | undefined;
  let unlistenOutput: UnlistenFn | undefined;
  let unlistenStatus: UnlistenFn | undefined;
  let unlistenDragDrop: UnlistenFn | undefined;

  // Gate: suppress onData forwarding during initialization to prevent
  // xterm.js auto-responses to terminal queries (DA, DSR) from being
  // sent to the PTY as input. See GitHub issue #49.
  let inputReady = false;

  // Capture the first prompt typed by the user (text before first Enter).
  // Skip if session already has a prompt (e.g., from a GitHub issue).
  let promptBuffer = "";
  let promptCaptured = (() => {
    let list: Project[] = [];
    projects.subscribe((v) => { list = v; })();
    const session = list.flatMap((p) => p.sessions).find((s) => s.id === sessionId);
    return session?.initial_prompt != null || session?.github_issue != null;
  })();

  async function handleImagePaste(
    writeToPty: (data: string) => Promise<unknown>,
  ) {
    const hasImage = await clipboardHasImage();
    if (hasImage) {
      // Send empty bracket paste to trigger Claude Code's clipboard image reader
      await writeToPty("\x1b[200~\x1b[201~");
    } else {
      // No image — read text from clipboard and send as bracket paste
      try {
        const text = await navigator.clipboard.readText();
        if (text) {
          // Capture pasted text into the prompt buffer so the summary pane
          // shows the full prompt (paste bypasses term.onData).
          if (!promptCaptured) {
            promptBuffer += text;
          }
          await writeToPty("\x1b[200~" + text + "\x1b[201~");
        }
      } catch {
        // Clipboard read failed — nothing to paste
      }
    }
  }

  function saveInitialPrompt(prompt: string) {
    let projectList: Project[] = [];
    projects.subscribe((v) => { projectList = v; })();
    const match = projectList.flatMap((p) =>
      p.sessions.map((s) => ({ session: s, projectId: p.id }))
    ).find((x) => x.session.id === sessionId);
    if (!match || match.session.initial_prompt != null) return;

    invoke("set_initial_prompt", {
      projectId: match.projectId,
      sessionId,
      prompt,
    }).then(() => {
      // Refresh the store so SummaryPane picks up the change
      invoke<Project[]>("list_projects").then((result) => {
        projects.set(result);
      });
    }).catch((err) => {
      console.error("Failed to save initial prompt:", err);
    });
  }

  const IMAGE_EXTENSIONS = new Set([
    ".png", ".jpg", ".jpeg", ".gif", ".webp", ".bmp",
  ]);

  function isImageFile(path: string): boolean {
    const ext = path.slice(path.lastIndexOf(".")).toLowerCase();
    return IMAGE_EXTENSIONS.has(ext);
  }

  onMount(() => {
    if (!containerEl) return;

    term = new Terminal({
      cursorBlink: true,
      fontSize: 13,
      fontFamily: "'JetBrains Mono', 'Fira Code', monospace",
      scrollback: 10000,
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

    // Only fit if the container is actually visible — xterm.js can't measure
    // character cells in a display:none ancestor, which produces bogus cols.
    if (containerEl.offsetParent !== null) {
      fitAddon.fit();
      termOpened = true;
    }

    const writeToPty = (data: string) =>
      invoke("write_to_pty", { sessionId, data });

    // Handle keys that xterm.js doesn't natively support
    term.attachCustomKeyEventHandler(
      makeCustomKeyHandler(
        (data) => {
          if (!inputReady) return;
          invoke("send_raw_to_pty", { sessionId, data });
        },
        {
          onImagePaste: () => {
            if (!inputReady) return;
            handleImagePaste(writeToPty);
          },
        },
      ),
    );

    // Connect user input to PTY (gated until initialization settles)
    term.onData((data: string) => {
      if (!inputReady) return;

      // Capture the first prompt: buffer keystrokes until Enter
      if (!promptCaptured) {
        for (const ch of data) {
          if (ch === "\r" || ch === "\n") {
            const trimmed = promptBuffer.trim();
            if (trimmed.length > 0) {
              promptCaptured = true;
              saveInitialPrompt(trimmed);
            }
            promptBuffer = "";
            break;
          } else if (ch === "\x7f" || ch === "\b") {
            // Backspace
            promptBuffer = promptBuffer.slice(0, -1);
          } else if (ch >= " ") {
            // Printable character
            promptBuffer += ch;
          }
        }
      }

      writeToPty(data).catch((err) => {
        console.error("Failed to write to PTY:", err);
      });
    });

    // Listen for PTY output (base64-encoded).
    // After the listener is set up, allow a brief window for tmux's
    // initial terminal queries and xterm auto-responses to settle,
    // then enable input forwarding.
    listen<string>(`pty-output:${sessionId}`, (event) => {
      if (term) {
        const bytes = Uint8Array.from(atob(event.payload), (c) =>
          c.charCodeAt(0),
        );
        term.write(bytes);
      }
    }).then((fn) => {
      unlistenOutput = fn;
      setTimeout(() => {
        inputReady = true;
      }, 100);
    });

    // Listen for session status changes
    listen<string>(`session-status-changed:${sessionId}`, () => {
      if (term) {
        term.writeln("\r\n\x1b[90m[Session ended]\x1b[0m");
      }
    }).then((fn) => {
      unlistenStatus = fn;
    });

    // Listen for drag-and-drop file events (from Finder).
    // Gate on active session — this is a window-level event so all
    // mounted Terminal instances receive it; only the active one should act.
    listen<{ paths: string[] }>("tauri://drag-drop", async (event) => {
      let active: string | null = null;
      activeSessionId.subscribe((v) => { active = v; })();
      if (active !== sessionId) return;

      const imagePath = event.payload.paths.find(isImageFile);
      if (imagePath) {
        try {
          await invoke("copy_image_file_to_clipboard", { path: imagePath });
          await writeToPty("\x1b[200~\x1b[201~");
        } catch (err) {
          console.error("Failed to handle dropped image:", err);
        }
      }
    }).then((fn) => {
      unlistenDragDrop = fn;
    });

    // Handle resize
    resizeObserver = new ResizeObserver(() => {
      if (fitAddon && term && containerEl) {
        // Skip resize when container is hidden (display:none ancestor)
        if (containerEl.offsetParent === null) return;

        // If xterm was opened while hidden, cell measurements are invalid.
        // Force a full canvas repaint first so FitAddon gets correct metrics.
        if (!termOpened) {
          term.refresh(0, term.rows - 1);
          termOpened = true;
        }

        fitAddon.fit();

        // Guard against bogus dimensions from bad cell measurements
        if (term.cols < 10) return;

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

    // Refit when becoming visible (display: none -> flex doesn't always trigger ResizeObserver).
    // Watch .terminal-wrapper (grandparent) which is the element that gets the `visible` class.
    mutationObserver = new MutationObserver(() => {
      if (containerEl && containerEl.offsetParent !== null && fitAddon && term) {
        // If xterm was opened while hidden, cell measurements are invalid.
        // Force a full canvas repaint first so FitAddon gets correct metrics.
        if (!termOpened) {
          term.refresh(0, term.rows - 1);
          termOpened = true;
        }

        fitAddon.fit();

        // Guard against bogus dimensions
        if (term.cols < 10) return;

        // Force full repaint — canvas content may be stale after display:none
        term.refresh(0, term.rows - 1);
        // Notify PTY of dimensions so the program gets SIGWINCH and redraws its TUI
        invoke("resize_pty", {
          sessionId,
          rows: term.rows,
          cols: term.cols,
        }).catch((err: unknown) => {
          console.error("Failed to resize PTY:", err);
        });
      }
    });
    // Observe .terminal-wrapper (grandparent of .terminal-container) for class changes.
    // Previously this watched .terminal-inner (parent) which never gets class changes.
    const wrapperEl = containerEl?.parentElement?.parentElement;
    if (wrapperEl) {
      mutationObserver.observe(wrapperEl, {
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
    unlistenDragDrop?.();
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
