import { writable } from "svelte/store";

interface ToastMessage {
  id: number;
  text: string;
  type: "error" | "info";
}

export const toasts = writable<ToastMessage[]>([]);
let counter = 0;

export function showToast(text: string, type: "error" | "info" = "info") {
  const id = counter++;
  toasts.update(t => [...t, { id, text, type }]);
  setTimeout(() => {
    toasts.update(t => t.filter(msg => msg.id !== id));
  }, 5000);
}
