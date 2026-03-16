/**
 * Custom key-event handler for xterm.js terminals.
 *
 * Returns `false` to block xterm from processing the event,
 * `true` to let xterm handle it normally.
 *
 * When `onImagePaste` is provided, Cmd-V / Ctrl-V is intercepted so the
 * caller can read the clipboard for image data. Without it, paste falls
 * through to xterm's native handling.
 *
 * `sendRawToPty` sends data bypassing tmux's outer terminal parser (for CSI u sequences).
 */
export function makeCustomKeyHandler(
  sendRawToPty: (data: string) => void,
  options?: { onImagePaste?: () => void },
) {
  return (event: KeyboardEvent): boolean => {
    // Block individual keystrokes during IME composition to prevent
    // duplicate input. Composition events (compositionstart/update/end)
    // are handled by separate listeners in xterm.js and are not affected.
    if (event.isComposing) {
      return false;
    }

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

    // Intercept paste (Cmd-V / Ctrl-V) when an image-paste callback is provided.
    // Block all event types to prevent xterm from also handling the paste,
    // but only fire the callback on keydown to avoid duplicates.
    if (
      options?.onImagePaste &&
      event.key === "v" &&
      (event.metaKey || event.ctrlKey)
    ) {
      if (event.type === "keydown") {
        options.onImagePaste();
      }
      return false;
    }

    return true;
  };
}
