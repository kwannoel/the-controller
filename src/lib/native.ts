export async function copyImageBlobToClipboard(blob: Blob): Promise<void> {
  const type = blob.type || "image/png";
  await navigator.clipboard.write([
    new ClipboardItem({ [type]: blob }),
  ]);
}
