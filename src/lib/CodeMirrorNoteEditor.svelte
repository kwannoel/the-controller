<script lang="ts">
  import { untrack } from "svelte";
  import { EditorState } from "@codemirror/state";
  import { EditorView, drawSelection } from "@codemirror/view";
  import { markdown } from "@codemirror/lang-markdown";
  import { Vim, getCM, vim } from "@replit/codemirror-vim";

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
    onChange?: (value: string) => void;
    onEscape?: (mode: VimMode | string) => void;
    onModeChange?: (mode: VimMode | string) => void;
    onAiChat?: (request: AiChatRequest) => void;
  }

  let { value, focused = false, entryKey, onChange, onEscape, onModeChange, onAiChat }: Props = $props();

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
        EditorView.lineWrapping,
        EditorView.domEventHandlers({
          keydown: (event) => {
            if (event.key === "Escape") {
              onEscape?.(currentMode);
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
      // coordsAtPos returns null for off-screen positions. When the entire
      // note is selected (ggVG), `from` (pos 0) is scrolled out of view
      // while `to` (cursor end) is visible — fall back to `to` coords.
      const coords = view.coordsAtPos(from) ?? view.coordsAtPos(to);
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
    font-family: var(--font-mono);
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
     "smeared" highlight alongside (or persisting after) the custom overlay. */
  .note-code-editor :global(.cm-content::selection),
  .note-code-editor :global(.cm-content *::selection) {
    background-color: transparent !important;
  }
</style>
