import { describe, expect, it } from "vitest";
import { EditorState } from "@codemirror/state";
import { EditorView } from "@codemirror/view";
import { markdown } from "@codemirror/lang-markdown";
import { markdownLivePreview } from "./markdownLivePreview";

function createView(doc: string, cursorPos?: number): EditorView {
  const state = EditorState.create({
    doc,
    extensions: [markdown(), markdownLivePreview()],
    selection: cursorPos !== undefined ? { anchor: cursorPos } : undefined,
  });
  const parent = document.createElement("div");
  return new EditorView({ state, parent });
}

describe("markdownLivePreview", () => {
  describe("headings", () => {
    it("applies heading class to ATXHeading lines when cursor is elsewhere", () => {
      const view = createView("# Hello World\n\nsome text", 20);
      const lines = view.dom.querySelectorAll(".cm-line");
      expect(lines[0].querySelector(".cm-md-h1")).not.toBeNull();
    });

    it("does not hide heading markers when cursor is on the heading line", () => {
      const view = createView("# Hello World\n\nsome text", 3);
      const lines = view.dom.querySelectorAll(".cm-line");
      expect(lines[0].querySelector(".cm-md-h1")).toBeNull();
    });

    it("applies different classes for h1 through h3", () => {
      const doc = "# H1\n\n## H2\n\n### H3\n\ntext";
      const view = createView(doc, doc.length - 1);
      expect(view.dom.querySelector(".cm-md-h1")).not.toBeNull();
      expect(view.dom.querySelector(".cm-md-h2")).not.toBeNull();
      expect(view.dom.querySelector(".cm-md-h3")).not.toBeNull();
    });
  });
});
