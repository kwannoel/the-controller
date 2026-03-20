import { describe, expect, it } from "vitest";
import { EditorState } from "@codemirror/state";
import { EditorView } from "@codemirror/view";
import { Vim, getCM, vim } from "@replit/codemirror-vim";

function createEditor(doc: string) {
  const state = EditorState.create({
    doc,
    extensions: [vim()],
  });
  const view = new EditorView({ state, parent: document.body });
  return view;
}

describe("vim a command", () => {
  it("positions cursor after current character for insertion", () => {
    const view = createEditor("Hello");
    view.focus();

    // Normal mode, cursor at position 0 (on 'H')
    expect(view.state.selection.main.head).toBe(0);

    // Press 'a' via vim
    const cm = getCM(view)!;
    Vim.handleKey(cm, "a", "mapping");

    // Cursor should now be at position 1 (after 'H')
    const cursorPos = view.state.selection.main.head;
    expect(cursorPos).toBe(1);

    // Simulate typing 'X' at the cursor position
    view.dispatch({
      changes: { from: cursorPos, insert: "X" },
      selection: { anchor: cursorPos + 1 },
    });

    // Result should be "HXello" — X inserted after H
    expect(view.state.doc.toString()).toBe("HXello");

    view.destroy();
  });

  it("i positions cursor at current character", () => {
    const view = createEditor("Hello");
    view.focus();

    expect(view.state.selection.main.head).toBe(0);

    const cm = getCM(view)!;
    Vim.handleKey(cm, "i", "mapping");

    // Cursor should stay at position 0 (before 'H')
    const cursorPos = view.state.selection.main.head;
    expect(cursorPos).toBe(0);

    view.dispatch({
      changes: { from: cursorPos, insert: "X" },
      selection: { anchor: cursorPos + 1 },
    });

    // Result should be "XHello" — X inserted before H
    expect(view.state.doc.toString()).toBe("XHello");

    view.destroy();
  });

  it("a in middle of line positions cursor correctly", () => {
    const view = createEditor("Hello");
    view.focus();

    const cm = getCM(view)!;

    // Move to position 2 (on 'l')
    Vim.handleKey(cm, "l");
    Vim.handleKey(cm, "l");
    expect(view.state.selection.main.head).toBe(2);

    // Press 'a'
    Vim.handleKey(cm, "a", "mapping");

    // Cursor should be at position 3 (after first 'l')
    const cursorPos = view.state.selection.main.head;
    expect(cursorPos).toBe(3);

    view.dispatch({
      changes: { from: cursorPos, insert: "X" },
      selection: { anchor: cursorPos + 1 },
    });

    // "HelXlo"
    expect(view.state.doc.toString()).toBe("HelXlo");

    view.destroy();
  });

  it("a at end of line positions cursor after last char", () => {
    const view = createEditor("Hi");
    view.focus();

    const cm = getCM(view)!;

    // Move to position 1 (on 'i') — last character
    Vim.handleKey(cm, "l");
    expect(view.state.selection.main.head).toBe(1);

    // Press 'a'
    Vim.handleKey(cm, "a", "mapping");

    // Cursor should be at position 2 (after 'i')
    const cursorPos = view.state.selection.main.head;
    expect(cursorPos).toBe(2);

    view.dispatch({
      changes: { from: cursorPos, insert: "X" },
      selection: { anchor: cursorPos + 1 },
    });

    // "HiX"
    expect(view.state.doc.toString()).toBe("HiX");

    view.destroy();
  });

  it("a entry key from sidebar positions cursor correctly", () => {
    const view = createEditor("Hello");

    // Simulate the entry key flow:
    // 1. Focus the view
    view.focus();

    // 2. Call Vim.handleKey with "a" and "mapping" context (same as CodeMirrorNoteEditor)
    const cm = getCM(view)!;
    Vim.handleKey(cm, "a", "mapping");

    // Cursor should be at position 1 (after 'H')
    const cursorPos = view.state.selection.main.head;
    expect(cursorPos).toBe(1);

    // 3. User types text
    view.dispatch({
      changes: { from: cursorPos, insert: "X" },
      selection: { anchor: cursorPos + 1 },
    });

    expect(view.state.doc.toString()).toBe("HXello");

    view.destroy();
  });
});
