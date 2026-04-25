/**
 * Check if the system clipboard contains an image.
 * Returns true if an image is present, false otherwise.
 */
export async function clipboardHasImage(): Promise<boolean> {
  try {
    const items = await navigator.clipboard.read();
    return items.some((item) => item.types.some((t) => t.startsWith("image/")));
  } catch {
    return false;
  }
}
