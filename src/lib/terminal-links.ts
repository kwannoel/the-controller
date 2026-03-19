/**
 * Utilities for Ctrl/Cmd+Click link opening in xterm.js terminals.
 *
 * When Claude Code has mouse tracking enabled (alternate screen mode),
 * xterm.js forwards clicks to the application instead of activating
 * WebLinksAddon links. These helpers extract URLs from the terminal
 * buffer so a modifier-click handler can open them.
 */

/** Find a URL at the given column position in a line of text. */
export function findUrlAtPosition(text: string, col: number): string | null {
  const urlRegex = /https?:\/\/[^\s<>"')\]]+/g;
  let match;
  while ((match = urlRegex.exec(text)) !== null) {
    if (col >= match.index && col < match.index + match[0].length) {
      return match[0];
    }
  }
  return null;
}

/** Extract line text from an xterm.js buffer line. */
export function extractLineText(line: {
  length: number;
  getCell(x: number): { getChars(): string } | undefined;
}): string {
  let text = "";
  for (let i = 0; i < line.length; i++) {
    const ch = line.getCell(i)?.getChars();
    text += ch || " ";
  }
  return text;
}
