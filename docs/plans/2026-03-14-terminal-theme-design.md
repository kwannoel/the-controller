# Terminal Theme Loading From Kitty-Style Config

## Problem

The terminal currently hardcodes its xterm.js theme in the frontend. Users cannot align the app with their existing terminal color scheme, and the app needs a stable theme file location under `~/.the-controller/`.

## Decision

Load terminal colors from `~/.the-controller/current-theme.conf` using a kitty-compatible key/value format. The file is optional. If it is missing, unreadable, or contains no usable theme values, the terminal falls back to the current built-in theme.

## Scope

- Add backend support for loading a terminal theme from `~/.the-controller/current-theme.conf`
- Parse kitty-style color assignments and map them to xterm.js theme fields
- Apply the loaded theme when creating xterm terminals
- Keep the app base directory at `~/.the-controller/`

Out of scope:

- UI for editing or previewing themes
- Adding theme settings to `config.json`
- Changing onboarding or `projects_root`

## Architecture

### Config Directory

The app storage base directory remains `~/.the-controller/`. Theme loading uses that same base directory and does not introduce a separate config path abstraction.

### Theme Loading

Add a Rust module that:

- Defines a serializable `TerminalTheme` DTO shaped for xterm.js consumption
- Exposes a `default_terminal_theme()` helper matching the current hardcoded theme
- Reads `<base_dir>/current-theme.conf`
- Parses kitty-style `key value` lines, ignoring blank lines and comments
- Converts recognized keys into `TerminalTheme`

The Tauri command should be async and wrap file I/O in `spawn_blocking`.

### Supported Keys

Initial support should cover the values xterm.js can use directly:

- `background` -> `background`
- `foreground` -> `foreground`
- `cursor` -> `cursor`
- `selection_background` -> `selectionBackground`
- `selection_foreground` -> `selectionForeground`
- `cursor_text_color` -> `cursorAccent`
- `color0` through `color15` -> ANSI palette fields

Unknown keys are ignored. If a recognized key has an invalid color value, parsing should fail closed to the default theme rather than applying a partially broken theme.

### Frontend Integration

`Terminal.svelte` should stop hardcoding the theme inline. Instead it should:

1. Start from the shared default theme
2. Call the backend command during `onMount`
3. Use the returned theme when constructing `new Terminal({ theme })`
4. Fall back silently to the default theme on command failure

The rest of the terminal lifecycle remains unchanged.

## Error Handling

- Missing theme file: return default theme
- Invalid file contents: return default theme
- Command failure: frontend logs the error and uses default theme

The terminal must still open even if theme loading fails.

## Testing

Rust tests should cover:

- Default config directory path points to `~/.the-controller`
- Missing theme file returns the default theme
- Valid kitty-style config maps to expected xterm theme fields
- Invalid values fall back to the default theme

Frontend tests should cover:

- `Terminal.svelte` requests the backend theme before constructing xterm
- The returned theme is passed into the xterm constructor
- A command failure still results in terminal creation with the default theme
