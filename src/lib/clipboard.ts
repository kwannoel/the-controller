import { readImage } from "@tauri-apps/plugin-clipboard-manager";

/**
 * Check if the system clipboard contains an image.
 * Returns true if an image is present, false otherwise.
 */
export async function clipboardHasImage(): Promise<boolean> {
  try {
    await readImage();
    return true;
  } catch {
    return false;
  }
}
