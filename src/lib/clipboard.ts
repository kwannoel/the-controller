import { isTauri } from "$lib/platform";

/**
 * Check if the system clipboard contains an image.
 * Returns true if an image is present, false otherwise.
 */
export async function clipboardHasImage(): Promise<boolean> {
  if (!isTauri) return false;
  try {
    const { readImage } = await import("@tauri-apps/plugin-clipboard-manager");
    await readImage();
    return true;
  } catch {
    return false;
  }
}
