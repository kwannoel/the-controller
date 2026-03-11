# Controller Chat Redesign

## Feeling

Quiet companion — the chat should feel like a calm presence that recedes when you don't need it and is instantly there when you look.

## Research

1. **Cursor's chat panel** — Right sidebar, toggleable (Cmd+I). Messages use subtle background shifts, not heavy borders. Lesson: minimal, toggleable, blend with the editor.
2. **Zed's assistant panel** — Dense, code-editor native. Compact messages, subtle role differentiation. Lesson: density and consistency with the editor's visual language.
3. **The Controller's own sidebar** — `#1e1e2e` background, `px` units, `12px 16px` padding, `14px` headers, `#313244` borders. Lesson: match exactly for cohesion.

## Design

### Toggle

- `g` key toggles visibility via `controllerChatVisible` store (default `true`)
- Registered as `toggle-controller-chat` command in Panels section
- Conditional render in `App.svelte`, same pattern as sidebar

### Layout

- Right sidebar, `280px` width (closer to sidebar's `250px`)
- Background `#1e1e2e` (matching sidebar)

### Header

- Matches sidebar header: `14px` font, `600` weight, `12px 16px` padding, `border-bottom: 1px solid #313244`
- Shows focus context only (project name / note filename)
- No "Controller Chat" label — panel position communicates role
- No focus: "No focus" in `#6c7086`

### Transcript

- Left-border accent for role differentiation (no rounded bubbles):
  - User: `3px` left border `#89b4fa` (blue)
  - Assistant: `3px` left border `#a6e3a1` (green)
  - Tool: `3px` left border `#f9e2af` (yellow)
- Padding: `8px 12px` per message
- No role text labels — border color communicates it
- Text: `13px`, `#cdd6f4`, `white-space: pre-wrap`
- Gap: `4px` between messages

### Composer

- Textarea: `#11111b` background, `1px solid #45475a` border, `4px` border-radius
- `rows="2"`, placeholder: `"Ask the controller..."`
- Submit on Enter, newline on Shift+Enter
- No visible send button
- Working state: subtle `#6c7086` "working..." text below textarea

### Empty State

- Centered: "No messages yet" in `#6c7086` at `13px`

### States

- **Empty:** centered hint text
- **Populated:** scrolling transcript, auto-scroll to bottom on new messages
- **Working:** textarea disabled, "working..." indicator
- **Hidden:** component not rendered, `g` brings it back

## Critique

- **Eye movement:** Focus context in header anchors the eye, transcript flows down, composer at bottom.
- **Negative space:** `8px 12px` padding tightens messages without cramping. `4px` gap groups them.
- **Visual weight:** Left-border accents lighter than current bordered bubbles.
- **Contrast as communication:** Three accent colors (blue/green/yellow) map to user/assistant/tool without labels.
- **Glance test:** Colored stripes = roles, text area at bottom = where to type.
- **Consistency as trust:** Matching sidebar tokens makes chat feel native.
- **Removed:** uppercase label, role text per message, blue pill send button, rounded message bubbles.
