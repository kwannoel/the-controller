<script lang="ts">
  import { untrack } from "svelte";
  import { EditorState } from "@codemirror/state";
  import { EditorView, drawSelection } from "@codemirror/view";
  import { markdown } from "@codemirror/lang-markdown";
  import { Vim, getCM, vim } from "@replit/codemirror-vim";
  import { markdownLivePreview } from "./markdownLivePreview";

  // WKWebView (Tauri on macOS) may report Shift+letter keydown events with
  // a lowercase `key` and `shiftKey: true` instead of the uppercase `key` the
  // vim plugin expects.  Patch `vimKeyFromEvent` so 'g' + shiftKey → 'G'.
  const _origVimKeyFromEvent = Vim.vimKeyFromEvent;
  Vim.vimKeyFromEvent = function (e: any, vim: any) {
    if (
      e.shiftKey && !e.ctrlKey && !e.altKey && !e.metaKey &&
      typeof e.key === "string" && e.key.length === 1 &&
      e.key >= "a" && e.key <= "z"
    ) {
      return _origVimKeyFromEvent.call(
        this,
        { key: e.key.toUpperCase(), shiftKey: e.shiftKey, ctrlKey: e.ctrlKey, altKey: e.altKey, metaKey: e.metaKey, code: e.code } as unknown as KeyboardEvent,
        vim,
      );
    }
    return _origVimKeyFromEvent.call(this, e, vim);
  };

  export type VimMode = "normal" | "insert" | "visual" | "replace";

  export interface AiChatRequest {
    selectedText: string;
    from: number;
    to: number;
    coords: { left: number; top: number; bottom: number };
  }

  interface Props {
    value: string;
    focused?: boolean;
    entryKey?: string;
    folder?: string;
    resolveImageSrc?: (path: string) => string | null;
    onChange?: (value: string) => void;
    onEscape?: (mode: VimMode | string) => void;
    onModeChange?: (mode: VimMode | string) => void;
    onAiChat?: (request: AiChatRequest) => void;
    onImageSaved?: (relativePath: string) => void;
  }

  let { value, focused = false, entryKey, folder, resolveImageSrc, onChange, onEscape, onModeChange, onAiChat, onImageSaved }: Props = $props();

  const ALLOWED_IMAGE_TYPES = ["image/png", "image/jpeg", "image/gif", "image/webp"];

  function extensionFromMime(mime: string): string | null {
    const map: Record<string, string> = {
      "image/png": "png",
      "image/jpeg": "jpg",
      "image/gif": "gif",
      "image/webp": "webp",
    };
    return map[mime] ?? null;
  }

  async function handleImageFile(file: File, insertPos: number) {
    if (!folder) return;
    const ext = extensionFromMime(file.type);
    if (!ext) return;

    const { command } = await import("$lib/backend");
    const arrayBuf = await file.arrayBuffer();
    const bytes = Array.from(new Uint8Array(arrayBuf));

    const relativePath = await command<string>("save_note_image", {
      folder,
      imageBytes: bytes,
      extension: ext,
    });

    if (!view) return;

    const mdText = `![](${relativePath})`;
    view.dispatch({
      changes: { from: insertPos, to: insertPos, insert: mdText },
    });
    onChange?.(view.state.doc.toString());
    onImageSaved?.(relativePath);
  }

  let hostEl: HTMLDivElement | undefined;
  let view: EditorView | null = null;
  let currentMode: VimMode | string = "normal";

  function buildState(doc: string) {
    return EditorState.create({
      doc,
      extensions: [
        vim(),
        drawSelection(),
        markdown(),
        markdownLivePreview({ resolveImageSrc: untrack(() => resolveImageSrc) }),
        EditorView.lineWrapping,
        EditorView.domEventHandlers({
          keydown: (event) => {
            if (event.key === "Escape") {
              onEscape?.(currentMode);
            }
            return false;
          },
          paste: (event, editorView) => {
            const files = event.clipboardData?.files;
            if (!files || files.length === 0) return false;
            for (const file of files) {
              if (ALLOWED_IMAGE_TYPES.includes(file.type)) {
                event.preventDefault();
                const pos = editorView.state.selection.main.head;
                handleImageFile(file, pos);
                return true;
              }
            }
            return false;
          },
          drop: (event, editorView) => {
            const files = event.dataTransfer?.files;
            if (!files || files.length === 0) return false;
            for (const file of files) {
              if (ALLOWED_IMAGE_TYPES.includes(file.type)) {
                event.preventDefault();
                const dropPos = editorView.posAtCoords({
                  x: event.clientX,
                  y: event.clientY,
                });
                const pos = dropPos ?? editorView.state.selection.main.head;
                handleImageFile(file, pos);
                return true;
              }
            }
            return false;
          },
        }),
        EditorView.updateListener.of((update) => {
          if (update.docChanged) {
            onChange?.(update.state.doc.toString());
          }
        }),
      ],
    });
  }

  $effect(() => {
    if (!hostEl || view) return;

    // Use untrack to prevent this effect from depending on `value` and callback
    // props. The value sync effect (below) handles value changes separately.
    // Without untrack, every keystroke in insert mode would re-run this effect,
    // destroying and recreating the view (and losing vim state + focus).
    const initialValue = untrack(() => value);
    view = new EditorView({
      state: buildState(initialValue),
      parent: hostEl,
    });

    currentMode = "normal";
    untrack(() => onModeChange?.(currentMode));

    const cm = getCM(view);
    const handleModeChange = (event: { mode?: string }) => {
      const prevMode = currentMode;
      currentMode = event.mode ?? "normal";
      onModeChange?.(currentMode);

      // When exiting visual mode, WebKit/WKWebView can leave a stale native
      // selection highlight ("smeared" artifact) even though drawSelection()
      // renders its own overlay. Collapsing the native selection forces a
      // repaint and removes the artifact.
      if (prevMode === "visual" && currentMode === "normal") {
        window.getSelection()?.collapseToEnd();
      }
    };

    cm?.on("vim-mode-change", handleModeChange);

    Vim.defineAction("aiChat", (_cm: any) => {
      if (!view) return;
      const sel = view.state.selection.main;
      if (sel.empty) return;
      const from = sel.from;
      const to = sel.to;
      const selectedText = view.state.doc.sliceString(from, to);
      // coordsAtPos returns null for some off-screen positions, but for
      // positions scrolled above the viewport it may return very negative
      // coordinates instead.  When the entire note is selected (vG / ggVG),
      // `from` (pos 0) is scrolled out of view while `to` (cursor end) is
      // visible — fall back to `to` coords in either case.
      const fromCoords = view.coordsAtPos(from);
      const coords = (fromCoords && fromCoords.bottom >= 0) ? fromCoords : view.coordsAtPos(to);
      if (!coords) return;
      onAiChat?.({
        selectedText,
        from,
        to,
        coords: { left: coords.left, top: coords.top, bottom: coords.bottom },
      });
    });
    Vim.mapCommand("ga", "action", "aiChat", undefined, { context: "visual" });

    return () => {
      cm?.off("vim-mode-change", handleModeChange);
      view?.destroy();
      view = null;
    };
  });

  $effect(() => {
    if (!view) return;

    const currentValue = view.state.doc.toString();
    if (currentValue === value) return;

    view.dispatch({
      changes: { from: 0, to: currentValue.length, insert: value },
    });
  });

  let lastEntryKey: string | undefined;
  $effect(() => {
    if (view && focused) {
      view.focus();
      if (entryKey && entryKey !== lastEntryKey) {
        lastEntryKey = entryKey;
        const cm = getCM(view);
        if (cm) Vim.handleKey(cm, entryKey, "mapping");
      }
    } else {
      lastEntryKey = undefined;
    }
  });
</script>

<div class="note-code-editor" data-testid="note-code-editor" bind:this={hostEl}></div>

<style>
  .note-code-editor {
    flex: 1;
    min-width: 0;
    min-height: 0;
    background: var(--bg-void);
  }

  .note-code-editor :global(.cm-editor) {
    height: 100%;
    background: var(--bg-void);
    color: var(--text-primary);
  }

  .note-code-editor :global(.cm-scroller) {
    font-family: var(--font-sans, sans-serif);
    line-height: 1.6;
  }

  .note-code-editor :global(.cm-content) {
    padding: 16px;
    color: var(--text-primary);
    caret-color: var(--text-primary);
  }

  .note-code-editor :global(.cm-focused) {
    outline: none;
  }

  .note-code-editor :global(.cm-cursor) {
    border-left-color: var(--text-primary);
  }

  .note-code-editor :global(.cm-selectionBackground) {
    background: rgba(255, 255, 255, 0.15);
  }

  /* Hide native browser selection — drawSelection() renders its own overlay.
     CodeMirror's built-in hideNativeSelection CSS doesn't reliably suppress
     the native ::selection in WebKit/WKWebView (Tauri), causing a visible
     "smeared" highlight alongside (or persisting after) the custom overlay.
     The :focus variants are needed to override CodeMirror's own
     `.cm-content :focus::selection { background-color: highlight }` rule. */
  .note-code-editor :global(.cm-content::selection),
  .note-code-editor :global(.cm-content *::selection),
  .note-code-editor :global(.cm-content :focus::selection),
  .note-code-editor :global(.cm-content :focus *::selection) {
    background-color: rgba(255, 255, 255, 0.15) !important;
    color: var(--text-selection) !important;
    -webkit-text-fill-color: var(--text-selection) !important;
  }

  /* Live preview heading styles */
  .note-code-editor :global(.cm-md-h1) {
    font-size: 24px;
    font-weight: 700;
    font-family: var(--font-sans, sans-serif);
  }

  .note-code-editor :global(.cm-md-h2) {
    font-size: 20px;
    font-weight: 600;
    font-family: var(--font-sans, sans-serif);
  }

  .note-code-editor :global(.cm-md-h3) {
    font-size: 16px;
    font-weight: 600;
    font-family: var(--font-sans, sans-serif);
  }

  .note-code-editor :global(.cm-md-h4),
  .note-code-editor :global(.cm-md-h5),
  .note-code-editor :global(.cm-md-h6) {
    font-size: 14px;
    font-weight: 600;
    font-family: var(--font-sans, sans-serif);
  }

  /* Inline formatting */
  .note-code-editor :global(.cm-md-strong) {
    font-weight: 700;
  }

  .note-code-editor :global(.cm-md-em) {
    font-style: italic;
  }

  .note-code-editor :global(.cm-md-code) {
    background: var(--bg-surface);
    padding: 2px 5px;
    border-radius: 3px;
    font-family: var(--font-mono);
    font-size: 13px;
  }

  /* Links */
  .note-code-editor :global(.cm-md-link) {
    color: var(--text-emphasis);
    text-decoration: none;
  }

  .note-code-editor :global(.cm-md-link:hover) {
    text-decoration: underline;
  }

  /* List bullets */
  .note-code-editor :global(.cm-md-list-bullet) {
    color: var(--text-secondary);
  }

  /* Fenced code blocks */
  .note-code-editor :global(.cm-md-codeblock-line) {
    background: var(--bg-surface);
  }

  /* Images */
  .note-code-editor :global(.cm-md-image) {
    padding: 4px 0;
  }

  .note-code-editor :global(.cm-md-image img) {
    max-width: 100%;
    height: auto;
    border-radius: 4px;
    display: block;
  }
</style>
