# Retheme Design — Cold Dominant Power

## Feeling

"Cold dominant power." The interface commands the space. Not warm, not inviting — precise, authoritative, and absolute. Like a control room at 2 AM where everything works exactly as expected.

## Research

### Godel Terminal (primary reference)

**What it is:** Multi-panel AI terminal with up to 6 independent workspaces.

**Design lessons taken:**

1. **Gradient-black depth.** Radial gradients from `#141414` → `#0b0b0b` → `#070707` instead of flat colored surfaces. Dimensionality without borders. In a B&W palette, luminance is the only spatial tool — Godel proves it works.

2. **Near-invisible borders.** `rgba(255,255,255, 0.08–0.12)` — white at 8-12% opacity. Borders exist as light-catching edges, not drawn lines. Surfaces suggest themselves through light rather than pigment.

3. **Restraint as identity.** Godel uses exactly one accent color (mint green). We go further — zero accent color. White is the accent. Color exists only as functional status signals.

**What doesn't fit:** Marketing typography (Sofia Sans Semi Condensed) is too editorial. Heavy rounded corners (12-36px) would feel toy-like in a dense orchestration UI. 24-column grid is web-page thinking, not app thinking.

### Warp Terminal (secondary reference)

**Lesson taken:** Translucent overlays with blur instead of opaque stacked panels. Modals and overlays feel like frosted glass over the void — depth through physics, not color.

### Ghostty (tertiary reference)

**Lesson taken:** True black (`#000000`) as the deepest background. Translucency and blur (85% opacity, 16px blur) for panel differentiation. "Sane defaults" philosophy — beautiful out of the box.

## Design

### Palette — CSS Custom Properties

Everything centralized in `:root` in `app.css`. No more hardcoded hex across 30+ component files.

#### Backgrounds (luminance ladder)

| Token | Value | Use |
|-------|-------|-----|
| `--bg-void` | `#000000` | Terminal, deepest layer |
| `--bg-base` | `#0a0a0a` | Main content area |
| `--bg-surface` | `#141414` | Sidebar, panels |
| `--bg-elevated` | `#1c1c1c` | Modals, popovers |
| `--bg-hover` | `#242424` | Hover state |
| `--bg-active` | `#2e2e2e` | Selected/active state |

#### Borders (Godel-style rgba white)

| Token | Value | Use |
|-------|-------|-----|
| `--border-subtle` | `rgba(255, 255, 255, 0.06)` | Panel separators |
| `--border-default` | `rgba(255, 255, 255, 0.10)` | Inputs, card edges |

#### Text (luminance as hierarchy)

| Token | Value | Use |
|-------|-------|-----|
| `--text-emphasis` | `#ffffff` | Headings, active items, focus |
| `--text-primary` | `#e0e0e0` | Body text |
| `--text-secondary` | `#777777` | Labels, metadata |
| `--text-tertiary` | `#4a4a4a` | Placeholders, disabled |

#### Status (semantic color — the ONLY chromatic elements)

| Token | Value | Use |
|-------|-------|-----|
| `--status-idle` | `#4a9e6e` | Desaturated cool green |
| `--status-working` | `#b8a43c` | Muted gold, kept dim |
| `--status-error` | `#c44040` | Desaturated cool red |
| `--status-exited` | `#555555` | Neutral gray |

#### Focus

| Token | Value | Use |
|-------|-------|-----|
| `--focus-ring` | `#ffffff` | Pure white, 2px solid |

### Layout

No layout changes. Sidebar (250px fixed) + terminal area (flex: 1). The retheme is purely visual.

### Surfaces & Depth

- **No solid borders between panels.** Sidebar/terminal separation via luminance difference (`#141414` vs `#000000`) + a `--border-subtle` line.
- **Modal overlay:** `backdrop-filter: blur(16px)` + `rgba(0,0,0, 0.7)`. Modal body: `--bg-elevated` with `--border-default`. Frosted void.
- **Elevation shadows:** `0 8px 32px rgba(0,0,0, 0.6)` on modals.
- **Toast notifications:** `--bg-elevated`, `--border-default`, left border white (info) or status color (error). Shadow `0 4px 16px rgba(0,0,0, 0.5)`.

### Information Hierarchy

1. **First:** White headings and active items (`--text-emphasis`) — navigation and current state
2. **Second:** Status dots — the only chromatic elements, they pop against monochrome
3. **Third:** Body text (`--text-primary`) — readable but doesn't compete
4. **Fourth:** Terminal ANSI output — the most colorful area, where the work is

### Interactions

- **Hover:** Background steps up one luminance level (e.g., `--bg-surface` → `--bg-hover`)
- **Focus:** `outline: 2px solid var(--focus-ring); outline-offset: -2px; border-radius: 4px`
- **Active/selected:** `--bg-active` background + `--text-emphasis` text
- **Buttons:** Default: white outline on dark. Primary: white fill, black text. Destructive: `--status-error` fill, white text.

### Visual Treatment

- **Typography:** Geist Sans for UI text (geometric, cold, designed for dev tools). Geist Mono for terminal/code (matches Geist Sans, tighter than JetBrains Mono). Both free via `geist` npm package. Font stack: `'Geist', sans-serif` and `'Geist Mono', monospace`.
- **Font weight as hierarchy:** 400 body → 500 labels → 600 headings
- **Border radius:** 6px inputs/buttons, 8px modals, 4px badges. Slightly sharper than Godel — colder.
- **All borders use rgba white tokens**, never solid gray hex.

### Component Specifics

**Sidebar:**
- Background: `--bg-surface`
- Project headers: `--text-emphasis`, weight 600
- Session items: `--text-primary`, weight 400
- Active session: `--bg-active` + `--text-emphasis`
- Status dots: semantic colors — the only color in the sidebar

**Terminal area:**
- Background: `--bg-void` (pure black)
- Cursor: white block
- Summary pane: `--bg-base` with `--border-subtle` bottom border

**Modals:**
- Frosted glass overlay (blur + dark alpha)
- Body: `--bg-elevated`, `--border-default`, `border-radius: 8px`
- Inputs: `--bg-hover` background, `--border-default` border
- Input focus: `border-color: var(--text-emphasis)`

**Badges/pills:**
- Background: `--bg-hover`
- Text: `--text-secondary`
- Border radius: 4px

### States

- **Empty state:** `--text-tertiary` text on `--bg-void`. The void is comfortable.
- **Error:** Status dot in `--status-error`. Toast with red left border. Text stays monochrome.
- **Loading:** No decorative animation. Existing status mechanisms.
- **Populated:** Dense but structured. Luminance and weight differentiate, not color.

## Critique

### Lenses Applied

**Eye movement:** White headings draw the eye first (navigation). Status dots in color pop next (system state). Body text recedes. Terminal ANSI output is the richest visual area — correct, that's the work product.

**Negative space:** Pure black terminal anchors the composition. The luminance gap from `#000000` to `#141414` sidebar is perceptible but not harsh — spatial separation without a hard edge.

**Visual weight:** White text headings heaviest → colored status dots → body text → tertiary text nearly invisible. Matches information hierarchy exactly.

**Contrast as communication:** Color = status. Always. No exceptions. If it has color, it's system state. If it's white, it's structure. Zero ambiguity.

**Density vs. cognitive load:** B&W reduces cognitive load vs. current multi-hue Catppuccin. Fewer hue channels to process. Dense sidebar works because differentiation is structural (indent, dots, weight) not chromatic.

**Glance test:** Half-second view: black screen, light sidebar with white headings, colored dots. Instantly: session manager, some active (green), some working (gold). Monochrome makes status colors scream.

**Consistency as trust:** Same border treatment everywhere (rgba white). Same hover behavior (luminance +1). Same focus ring (white). Predictable.

### What Was Removed

1. ~~`--border-strong` (rgba 0.16)~~ — Three border levels is over-engineering. Two levels (subtle, default) are sufficient.
2. ~~Separate `--accent` token~~ — It was `#ffffff`, identical to `--text-emphasis`. Merged. White is the accent.
3. ~~Bright status colors~~ → Desaturated all three. Original values were too warm for "cold dominant."

## Scope

- **Pure visual transformation** — no logic, no semantics, no behavior changes
- **Centralize all color values** into CSS custom properties in `app.css`
- **Replace hardcoded hex** across all component `<style>` blocks with `var(--token)` references
- **Add `backdrop-filter: blur`** to modal overlays
- **Update xterm.js theme** object to match new palette
