import {
  Decoration,
  DecorationSet,
  EditorView,
  ViewPlugin,
  ViewUpdate,
} from "@codemirror/view";
import { syntaxTree } from "@codemirror/language";
import { RangeSetBuilder } from "@codemirror/state";

/** CSS classes applied by mark decorations. */
const headingMark = {
  1: Decoration.mark({ class: "cm-md-h1" }),
  2: Decoration.mark({ class: "cm-md-h2" }),
  3: Decoration.mark({ class: "cm-md-h3" }),
  4: Decoration.mark({ class: "cm-md-h4" }),
  5: Decoration.mark({ class: "cm-md-h5" }),
  6: Decoration.mark({ class: "cm-md-h6" }),
} as Record<number, Decoration>;

const headerMarkerHide = Decoration.replace({});

function cursorLineRanges(view: EditorView): Set<number> {
  const lines = new Set<number>();
  for (const range of view.state.selection.ranges) {
    const startLine = view.state.doc.lineAt(range.from).number;
    const endLine = view.state.doc.lineAt(range.to).number;
    for (let l = startLine; l <= endLine; l++) {
      lines.add(l);
    }
  }
  return lines;
}

function buildDecorations(view: EditorView): DecorationSet {
  const builder = new RangeSetBuilder<Decoration>();
  const cursorLines = cursorLineRanges(view);
  const tree = syntaxTree(view.state);

  const decorations: { from: number; to: number; deco: Decoration }[] = [];

  tree.iterate({
    enter(node) {
      const lineStart = view.state.doc.lineAt(node.from).number;
      const lineEnd = view.state.doc.lineAt(node.to).number;

      let onCursorLine = false;
      for (let l = lineStart; l <= lineEnd; l++) {
        if (cursorLines.has(l)) {
          onCursorLine = true;
          break;
        }
      }
      if (onCursorLine) return;

      const name = node.name;

      const headingMatch = name.match(/^ATXHeading(\d)$/);
      if (headingMatch) {
        const level = parseInt(headingMatch[1]);
        decorations.push({
          from: node.from,
          to: node.to,
          deco: headingMark[level],
        });
      }

      if (name === "HeaderMark") {
        const hideEnd = Math.min(node.to + 1, view.state.doc.length);
        decorations.push({
          from: node.from,
          to: hideEnd,
          deco: headerMarkerHide,
        });
      }
    },
  });

  decorations.sort((a, b) => a.from - b.from || a.to - b.to);
  for (const { from, to, deco } of decorations) {
    builder.add(from, to, deco);
  }

  return builder.finish();
}

const livePreviewPlugin = ViewPlugin.fromClass(
  class {
    decorations: DecorationSet;
    constructor(view: EditorView) {
      this.decorations = buildDecorations(view);
    }
    update(update: ViewUpdate) {
      if (update.docChanged || update.selectionSet || update.viewportChanged) {
        this.decorations = buildDecorations(update.view);
      }
    }
  },
  { decorations: (v) => v.decorations },
);

export function markdownLivePreview() {
  return livePreviewPlugin;
}
