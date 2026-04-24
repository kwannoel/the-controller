import { command } from "./backend";

export async function captureScreenshotDataUrl(): Promise<string> {
  const { default: html2canvas } = await import("html2canvas");
  const canvas = await html2canvas(document.body, {
    logging: false,
    useCORS: true,
    backgroundColor: null,
  });
  return canvas.toDataURL("image/png");
}

export async function captureScreenshotPath(): Promise<string> {
  const dataUrl = await captureScreenshotDataUrl();
  return await command<string>("save_screenshot", { dataUrl });
}

export async function copyImageBlobToClipboard(blob: Blob): Promise<void> {
  const type = blob.type || "image/png";
  await navigator.clipboard.write([
    new ClipboardItem({ [type]: blob }),
  ]);
}
