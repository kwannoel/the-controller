<script lang="ts">
  import { onMount, onDestroy } from "svelte";
  import { get } from "svelte/store";
  import { Terminal } from "@xterm/xterm";
  import { FitAddon } from "@xterm/addon-fit";
  import { WebLinksAddon } from "@xterm/addon-web-links";
  import { openUrl } from "$lib/platform";
  import { command, listen } from "$lib/backend";
  import { refreshProjectsFromBackend } from "./project-listing";
  import { makeCustomKeyHandler } from "./terminal-keys";
  import { createScrollTracker } from "./terminal-scroll";
  import { clipboardHasImage } from "./clipboard";
  import { activeSessionId, projects, type Project } from "./stores";
  import "@xterm/xterm/css/xterm.css";

  type TerminalTheme = {
    background: string;
    foreground: string;
    cursor: string;
    selectionBackground: string;
    selectionForeground?: string;
    cursorAccent?: string;
    black?: string;
    red?: string;
    green?: string;
    yellow?: string;
    blue?: string;
    magenta?: string;
    cyan?: string;
    white?: string;
    brightBlack?: string;
    brightRed?: string;
    brightGreen?: string;
    brightYellow?: string;
    brightBlue?: string;
    brightMagenta?: string;
    brightCyan?: string;
    brightWhite?: string;
  };

  const DEFAULT_TERMINAL_THEME: TerminalTheme = {
    background: "#000000",
    foreground: "#e0e0e0",
    cursor: "#ffffff",
    selectionBackground: "#2e2e2e",
  };

  interface Props {
    sessionId: string;
    kind?: string;
  }

  let { sessionId, kind = "claude" }: Props = $props();

  let containerEl: HTMLDivElement | undefined = $state();
  let term: Terminal | undefined;
  let fitAddon: FitAddon | undefined;
  let termOpened = false; // tracks whether term.open() produced valid measurements
  let resizeObserver: ResizeObserver | undefined;
  let mutationObserver: MutationObserver | undefined;
  let unlistenOutput: (() => void) | undefined;
  let unlistenStatus: (() => void) | undefined;
  let unlistenDragDrop: (() => void) | undefined;

  // Gate: suppress onData forwarding during initialization to prevent
  // xterm.js auto-responses to terminal queries (DA, DSR) from being
  // sent to the PTY as input. See GitHub issue #49.
  let inputReady = false;

  // Scroll-position tracker: prevents resize/visibility changes from
  // disrupting the user's scroll position while browsing history.
  const scrollTracker = createScrollTracker();

  // Whether connect_session has been called for this terminal.
  let connected = false;

  // Capture the first prompt typed by the user (text before first Enter).
  // Skip if session already has a prompt (e.g., from a GitHub issue).
  let promptBuffer = "";
  let promptCaptured = (() => {
    const session = get(projects).flatMap((p) => p.sessions).find((s) => s.id === sessionId);
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
    const match = get(projects).flatMap((p) =>
      p.sessions.map((s) => ({ session: s, projectId: p.id }))
    ).find((x) => x.session.id === sessionId);
    if (!match || match.session.initial_prompt != null) return;

    command("set_initial_prompt", {
      projectId: match.projectId,
      sessionId,
      prompt,
    }).then(() => refreshProjectsFromBackend()).catch((err) => {
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

  async function resolveTerminalTheme(): Promise<TerminalTheme> {
    try {
      return await command<TerminalTheme>("load_terminal_theme");
    } catch (err) {
      console.error("Failed to load terminal theme:", err);
      return DEFAULT_TERMINAL_THEME;
    }
  }

  onMount(async () => {
    if (!containerEl) return;
    const theme = await resolveTerminalTheme();

    term = new Terminal({
      cursorBlink: true,
      fontSize: 13,
      fontFamily: "'JetBrains Mono', 'Fira Code', monospace",
      scrollback: 10000,
      theme,
    });

    fitAddon = new FitAddon();
    term.loadAddon(fitAddon);
    term.loadAddon(new WebLinksAddon((_event, uri) => {
      openUrl(uri).catch((err) => {
        console.error("Failed to open URL:", err);
      });
    }));
    term.open(containerEl);

    // Track scroll position so we can avoid forcibly scrolling to bottom when
    // the user is reading history.  xterm.js fires onScroll whenever the
    // viewport position changes (including programmatic scrolls).
    term.onScroll(() => {
      if (!term) return;
      scrollTracker.handleScroll(term, containerEl);
    });

    const writeToPty = (data: string) =>
      command("write_to_pty", { sessionId, data });

    // Handle keys that xterm.js doesn't natively support.
    // Only intercept paste for Claude sessions (image paste is Claude Code-specific).
    // For other session types (e.g. Codex), let xterm handle paste natively.
    const keyHandlerOptions = kind === "claude"
      ? {
          onImagePaste: () => {
            if (!inputReady) return;
            handleImagePaste(writeToPty);
          },
        }
      : undefined;

    term.attachCustomKeyEventHandler(
      makeCustomKeyHandler(
        (data) => {
          if (!inputReady) return;
          command("send_raw_to_pty", { sessionId, data });
        },
        keyHandlerOptions,
      ),
    );

    // In alternate screen mode, xterm.js converts wheel events to arrow keys
    // (because there's no scrollback). Claude Code interprets those arrows as
    // input history navigation instead of scrolling. Fix: send SGR mouse wheel
    // escape sequences directly to the inner process via send_raw_to_pty, which
    // bypasses tmux's terminal parser. Claude Code's crossterm has mouse capture
    // enabled and will handle these as scroll events.
    term.attachCustomWheelEventHandler((ev: WheelEvent) => {
      if (term!.buffer.active.type === 'alternate') {
        ev.preventDefault();
        if (ev.deltaY !== 0) {
          // SGR mouse encoding: \x1b[<button;col;rowM
          // Button 64 = scroll up, 65 = scroll down
          const button = ev.deltaY < 0 ? 64 : 65;
          const seq = `\x1b[<${button};1;1M`;
          command("send_raw_to_pty", { sessionId, data: seq }).catch(() => {});
        }
        return false;
      }
      return true;
    });

    // Connect user input to PTY (gated until initialization settles)
    term.onData((data: string) => {
      if (!inputReady) return;

      // Capture the first prompt: buffer keystrokes until Enter
      if (!promptCaptured) {
        // Skip escape sequences (arrow keys, function keys, etc.)
        // They arrive as multi-char strings like \x1b[A — not typed text
        if (data.startsWith("\x1b")) {
          // Still forward to PTY below, just don't buffer
        } else {
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
      }

      writeToPty(data).catch((err) => {
        console.error("Failed to write to PTY:", err);
      });
    });

    // Register PTY output listener BEFORE connecting the session to avoid a
    // race where early output (including the alternate-screen escape sequence)
    // is emitted before the handler exists, causing xterm.js to miss the
    // screen-buffer switch and break trackpad scrolling.
    unlistenOutput = listen<string>(`pty-output:${sessionId}`, (payload) => {
      if (term) {
        const bytes = Uint8Array.from(atob(payload), (c) =>
          c.charCodeAt(0),
        );
        term.write(bytes);
      }
    });
    setTimeout(() => {
      inputReady = true;
    }, 100);

    // Only fit if the container is actually visible — xterm.js can't measure
    // character cells in a display:none ancestor, which produces bogus cols.
    if (containerEl.offsetParent !== null) {
      fitAddon.fit();
      termOpened = true;
      // Connect PTY at the measured size to avoid intermediate resizes
      // that cause extra newlines on restart.
      connected = true;
      command("connect_session", {
        sessionId,
        rows: term.rows,
        cols: term.cols,
      }).catch((err) => {
        console.error("Failed to connect session:", err);
      });
    }

    // Listen for session status changes
    unlistenStatus = listen<string>(`session-status-changed:${sessionId}`, () => {
      if (term) {
        term.writeln("\r\n\x1b[90m[Session ended]\x1b[0m");
      }
    });

    // Listen for drag-and-drop file events (from Finder).
    // Gate on active session — this is a window-level event so all
    // mounted Terminal instances receive it; only the active one should act.
    unlistenDragDrop = listen<{ paths: string[] }>("tauri://drag-drop", async (payload) => {
      if (get(activeSessionId) !== sessionId) return;

      const imagePath = payload.paths.find(isImageFile);
      if (imagePath) {
        try {
          await command("copy_image_file_to_clipboard", { path: imagePath });
          await writeToPty("\x1b[200~\x1b[201~");
        } catch (err) {
          console.error("Failed to handle dropped image:", err);
        }
      }
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

        scrollTracker.fitPreservingScroll(term, fitAddon);

        // Guard against bogus dimensions from bad cell measurements
        if (term.cols < 10) return;

        command("resize_pty", {
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

        scrollTracker.fitPreservingScroll(term, fitAddon);

        // Guard against bogus dimensions
        if (term.cols < 10) return;

        // Connect PTY if this terminal was hidden on mount
        if (!connected) {
          connected = true;
          command("connect_session", {
            sessionId,
            rows: term.rows,
            cols: term.cols,
          }).catch((err: unknown) => {
            console.error("Failed to connect session:", err);
          });
        }

        // Force full repaint — canvas content may be stale after display:none
        term.refresh(0, term.rows - 1);
        // Notify PTY of dimensions so the program gets SIGWINCH and redraws its TUI
        command("resize_pty", {
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
