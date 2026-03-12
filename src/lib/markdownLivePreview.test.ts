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

    it("does not apply heading class when cursor is on the heading line", () => {
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

  describe("inline formatting", () => {
    it("applies bold class and hides markers when cursor is elsewhere", () => {
      const view = createView("**bold text**\n\nother", 18);
      expect(view.dom.querySelector(".cm-md-strong")).not.toBeNull();
    });

    it("applies italic class when cursor is elsewhere", () => {
      const view = createView("*italic text*\n\nother", 18);
      expect(view.dom.querySelector(".cm-md-em")).not.toBeNull();
    });

    it("applies inline code class when cursor is elsewhere", () => {
      const view = createView("`code`\n\nother", 10);
      expect(view.dom.querySelector(".cm-md-code")).not.toBeNull();
    });

    it("does not apply formatting when cursor is on the line", () => {
      const view = createView("**bold text**\n\nother", 3);
      expect(view.dom.querySelector(".cm-md-strong")).toBeNull();
    });
  });

  describe("links", () => {
    it("styles link text when cursor is elsewhere", () => {
      const view = createView("[click here](https://example.com)\n\nother", 38);
      expect(view.dom.querySelector(".cm-md-link")).not.toBeNull();
    });

    it("shows raw markdown when cursor is on the link line", () => {
      const view = createView("[click here](https://example.com)\n\nother", 5);
      expect(view.dom.querySelector(".cm-md-link")).toBeNull();
    });
  });

  describe("lists", () => {
    it("replaces list marker with bullet when cursor is elsewhere", () => {
      const view = createView("- item one\n- item two\n\nother", 25);
      expect(view.dom.querySelector(".cm-md-list-bullet")).not.toBeNull();
    });

    it("shows raw markdown when cursor is on a list line", () => {
      const view = createView("- item one\n\nother", 3);
      expect(view.dom.querySelector(".cm-md-list-bullet")).toBeNull();
    });
  });

  describe("code blocks", () => {
    it("applies code block line class when cursor is elsewhere", () => {
      const doc = "text\n\n```js\nconst x = 1;\n```\n\nother";
      const view = createView(doc, doc.length - 1);
      expect(view.dom.querySelector(".cm-md-codeblock-line")).not.toBeNull();
    });
  });
});
