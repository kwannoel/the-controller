/**
 * Custom key-event handler for xterm.js terminals.
 *
 * Returns `false` to block xterm from processing the event,
 * `true` to let xterm handle it normally.
 *
 * Paste (Cmd-V / Ctrl-V) is intentionally NOT handled here — xterm.js
 * natively processes browser paste events and emits them via `onData`,
 * so a custom handler would cause double-paste.
 *
 * `sendRawToPty` sends data bypassing tmux's outer terminal parser (for CSI u sequences).
 */
export function makeCustomKeyHandler(
  sendRawToPty: (data: string) => void,
) {
  return (event: KeyboardEvent): boolean => {
    // Shift+Enter must be blocked on ALL event types (keydown, keypress, keyup)
    // to prevent xterm from also processing it as a regular Enter (\r).
    // We only send the CSI u sequence on keydown to avoid duplicates.
    // Uses send_raw_to_pty which bypasses tmux's outer terminal parser via
    // `tmux send-keys -H`, since tmux doesn't recognise CSI u from the outer PTY.
    if (event.key === "Enter" && event.shiftKey) {
      if (event.type === "keydown") {
        sendRawToPty("\x1b[13;2u");
      }
      return false;
    }

    return true;
  };
}
