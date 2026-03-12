import {
  Decoration,
  type DecorationSet,
  EditorView,
  ViewPlugin,
  type ViewUpdate,
  WidgetType,
} from "@codemirror/view";
import { syntaxTree } from "@codemirror/language";
import { RangeSetBuilder, type EditorState, Facet } from "@codemirror/state";

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

const strongMark = Decoration.mark({ class: "cm-md-strong" });
const emphasisMark = Decoration.mark({ class: "cm-md-em" });
const inlineCodeMark = Decoration.mark({ class: "cm-md-code" });
const syntaxHide = Decoration.replace({});
const linkMark = Decoration.mark({ class: "cm-md-link" });

class BulletWidget extends WidgetType {
  toDOM() {
    const span = document.createElement("span");
    span.className = "cm-md-list-bullet";
    span.textContent = "\u2022 ";
    return span;
  }
}

const bulletWidget = Decoration.replace({ widget: new BulletWidget() });
const codeBlockLine = Decoration.line({ class: "cm-md-codeblock-line" });

export type ImageSrcResolver = (path: string) => string | null;

const imageResolverFacet = Facet.define<ImageSrcResolver, ImageSrcResolver>({
  combine: (values) => values[values.length - 1] ?? (() => null),
});

class ImageWidget extends WidgetType {
  constructor(readonly src: string, readonly alt: string) {
    super();
  }
  eq(other: ImageWidget) {
    return this.src === other.src && this.alt === other.alt;
  }
  toDOM() {
    const wrapper = document.createElement("div");
    wrapper.className = "cm-md-image";
    const img = document.createElement("img");
    img.src = this.src;
    img.alt = this.alt;
    img.style.maxWidth = "100%";
    img.style.height = "auto";
    img.style.borderRadius = "4px";
    img.style.display = "block";
    img.style.margin = "4px 0";
    img.draggable = false;
    wrapper.appendChild(img);
    return wrapper;
  }
}

function resolveImageUrl(rawUrl: string, state: EditorState): string | null {
  if (rawUrl.startsWith("http://") || rawUrl.startsWith("https://")) {
    return rawUrl;
  }
  const resolver = state.facet(imageResolverFacet);
  return resolver(rawUrl);
}

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

      if (name === "Image") {
        const text = view.state.doc.sliceString(node.from, node.to);
        const match = text.match(/^!\[([^\]]*)\]\(([^)]+)\)$/);
        if (match) {
          const alt = match[1];
          const rawUrl = match[2];
          const src = resolveImageUrl(rawUrl, view.state);
          if (src) {
            decorations.push({
              from: node.from,
              to: node.to,
              deco: Decoration.replace({ widget: new ImageWidget(src, alt) }),
            });
            return;
          }
        }
      }

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

      if (name === "StrongEmphasis") {
        decorations.push({ from: node.from, to: node.to, deco: strongMark });
      }
      if (name === "Emphasis") {
        decorations.push({ from: node.from, to: node.to, deco: emphasisMark });
      }
      if (name === "EmphasisMark") {
        decorations.push({ from: node.from, to: node.to, deco: syntaxHide });
      }
      if (name === "InlineCode") {
        decorations.push({ from: node.from, to: node.to, deco: inlineCodeMark });
      }
      if (name === "CodeMark") {
        decorations.push({ from: node.from, to: node.to, deco: syntaxHide });
      }

      if (name === "Link") {
        decorations.push({ from: node.from, to: node.to, deco: linkMark });
      }
      if (name === "LinkMark") {
        decorations.push({ from: node.from, to: node.to, deco: syntaxHide });
      }
      if (name === "URL") {
        decorations.push({ from: node.from, to: node.to, deco: syntaxHide });
      }

      if (name === "ListMark") {
        const hideEnd = Math.min(node.to + 1, view.state.doc.length);
        decorations.push({ from: node.from, to: hideEnd, deco: bulletWidget });
      }

      if (name === "FencedCode") {
        const startLine = view.state.doc.lineAt(node.from).number;
        const endLine = view.state.doc.lineAt(node.to).number;
        for (let l = startLine; l <= endLine; l++) {
          const line = view.state.doc.line(l);
          decorations.push({ from: line.from, to: line.from, deco: codeBlockLine });
        }
      }
      if (name === "CodeInfo") {
        decorations.push({ from: node.from, to: node.to, deco: syntaxHide });
      }
    },
  });

  decorations.sort(
    (a, b) => a.from - b.from || a.deco.startSide - b.deco.startSide || a.to - b.to,
  );
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

export interface LivePreviewOptions {
  resolveImageSrc?: ImageSrcResolver;
}

export function markdownLivePreview(options?: LivePreviewOptions) {
  const extensions = [livePreviewPlugin];
  if (options?.resolveImageSrc) {
    extensions.unshift(imageResolverFacet.of(options.resolveImageSrc));
  }
  return extensions;
}
