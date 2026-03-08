# Command Registry Design

## Problem

Commands are defined in three separate places that can drift:
1. `stores.ts` — `HotkeyAction` type union
2. `HotkeyManager.svelte` — `handleHotkey()` switch mapping keys to actions
3. `HotkeyHelp.svelte` — hardcoded help sections

Adding a command requires updating all three. Nothing enforces this.

## Solution

Single command registry (`src/lib/commands.ts`) that defines every command with metadata. Help is derived from it. TypeScript exhaustive checking ensures new commands get handlers.

## CommandDef Structure

```ts
export type CommandId = "navigate-next" | "navigate-prev" | ...;

export interface CommandDef {
  id: CommandId;
  key: string;                 // trigger key ("j", "Cmd+S")
  section: "Navigation" | "Sessions" | "Projects" | "Panels";
  description: string;         // help text
  helpKey?: string;             // display override ("j / k" for paired commands)
  hidden?: boolean;             // don't show in help (paired secondary like "k")
  handledExternally?: boolean;  // handled outside main switch (Cmd+S, Cmd+K, Escape)
}
```

## Help Derivation

`HotkeyHelp.svelte` imports commands, filters out `hidden` entries, groups by `section`, renders. No hardcoded sections.

## Handler Enforcement

`HotkeyManager.svelte` builds a `Map<string, CommandDef>` from the registry. The `handleHotkey` switch operates on `cmd.id` (a `CommandId` union) instead of raw key strings. A `never` default branch makes unhandled commands a compile error. Commands marked `handledExternally` are excluded from the map.

## What Stays the Same

- `HotkeyAction` type and `dispatchHotkeyAction` — unchanged
- Component subscribers (App.svelte, TerminalManager, etc.) — unchanged
- Escape double-tap logic, Cmd+S/Cmd+K handling — stays in `onKeydown`, registered with `handledExternally: true`

## Files Changed

- **New:** `src/lib/commands.ts` — registry + types + `getHelpSections()` helper
- **Modified:** `src/lib/HotkeyHelp.svelte` — imports sections from registry
- **Modified:** `src/lib/HotkeyManager.svelte` — switch on `CommandId` instead of raw keys
- **Unchanged:** `stores.ts` — `HotkeyAction` stays as-is
