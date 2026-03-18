import { writable, derived } from "svelte/store";
import { command, listen } from "$lib/backend";
import { commands, applyOverrides } from "$lib/commands";
import { showToast } from "$lib/toast";

interface KeybindingsResult {
  overrides: Record<string, string>;
  warnings: string[];
  meta_key: string;
}

export const keybindingOverrides = writable<Record<string, string>>({});

/** Which physical modifier "Meta+" maps to: "cmd" (default) or "ctrl". */
export const metaKey = writable<"cmd" | "ctrl">("cmd");

export const resolvedCommands = derived(keybindingOverrides, ($overrides) =>
  applyOverrides(commands, $overrides),
);

function applyResult(result: KeybindingsResult) {
  keybindingOverrides.set(result.overrides);
  metaKey.set(result.meta_key === "ctrl" ? "ctrl" : "cmd");
  for (const w of result.warnings) {
    showToast(w, "error");
  }
}

export async function initKeybindings(): Promise<() => void> {
  const unlisten = await listen<string>("keybindings-changed", (raw) => {
    try {
      const result: KeybindingsResult = JSON.parse(raw);
      applyResult(result);
    } catch {
      // Keep current bindings on parse error
    }
  });

  try {
    const result = await command<KeybindingsResult>("load_keybindings");
    applyResult(result);
  } catch {
    // Silently use defaults if backend call fails
  }

  return unlisten;
}
