import { writable, get } from "svelte/store";

interface Keystroke {
  id: number;
  label: string;
}

export const keystrokeVisualizerEnabled = writable<boolean>(false);
export const keystrokes = writable<Keystroke[]>([]);

let counter = 0;
const MAX_VISIBLE = 5;
const FADE_MS = 2000;

export function toggleKeystrokeVisualizer() {
  keystrokeVisualizerEnabled.update((v) => {
    if (v) keystrokes.set([]);
    return !v;
  });
}

export function pushKeystroke(label: string) {
  if (!get(keystrokeVisualizerEnabled)) return;
  const id = counter++;
  keystrokes.update((list) => {
    const next = [...list, { id, label }];
    return next.length > MAX_VISIBLE
      ? next.slice(next.length - MAX_VISIBLE)
      : next;
  });
  setTimeout(() => {
    keystrokes.update((list) => list.filter((k) => k.id !== id));
  }, FADE_MS);
}
