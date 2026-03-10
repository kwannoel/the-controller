<script lang="ts">
  import { EditorState } from "@codemirror/state";
  import { EditorView, drawSelection } from "@codemirror/view";
  import { markdown } from "@codemirror/lang-markdown";
  import { getCM, vim } from "@replit/codemirror-vim";

  export type VimMode = "normal" | "insert" | "visual" | "replace";

  interface Props {
    value: string;
    focused?: boolean;
    onChange?: (value: string) => void;
    onEscape?: (mode: VimMode | string) => void;
    onModeChange?: (mode: VimMode | string) => void;
  }

  let { value, focused = false, onChange, onEscape, onModeChange }: Props = $props();

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

    view = new EditorView({
      state: buildState(value),
      parent: hostEl,
    });

    currentMode = "normal";
    onModeChange?.(currentMode);

    const cm = getCM(view);
    const handleModeChange = (event: { mode?: string }) => {
      currentMode = event.mode ?? "normal";
      onModeChange?.(currentMode);
    };

    cm?.on("vim-mode-change", handleModeChange);

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

  $effect(() => {
    if (view && focused) {
      view.focus();
    }
  });
</script>

<div class="note-code-editor" data-testid="note-code-editor" bind:this={hostEl}></div>

<style>
  .note-code-editor {
    flex: 1;
    min-width: 0;
    min-height: 0;
    background: #11111b;
  }

  .note-code-editor :global(.cm-editor) {
    height: 100%;
    background: #11111b;
    color: #cdd6f4;
  }

  .note-code-editor :global(.cm-scroller) {
    font-family: monospace;
    line-height: 1.6;
  }

  .note-code-editor :global(.cm-content) {
    padding: 16px;
    caret-color: #cdd6f4;
  }

  .note-code-editor :global(.cm-focused) {
    outline: none;
  }

  .note-code-editor :global(.cm-cursor) {
    border-left-color: #cdd6f4;
  }

  .note-code-editor :global(.cm-selectionBackground) {
    background: rgba(137, 180, 250, 0.35);
  }
</style>
