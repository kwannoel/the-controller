const isTauri = typeof window !== "undefined" && !!(window as any).__TAURI_INTERNALS__;

export async function openUrl(url: string): Promise<void> {
  if (isTauri) {
    const { openUrl } = await import("@tauri-apps/plugin-opener");
    return openUrl(url);
  }
  window.open(url, "_blank", "noopener,noreferrer");
}

export { isTauri };
