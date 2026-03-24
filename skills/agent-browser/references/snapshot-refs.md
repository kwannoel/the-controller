# Snapshots and Refs

Use `agent-browser snapshot -i` to inspect the current page and get element refs like `@e1`, `@e2`, and `@e3`.

## Core pattern

```bash
agent-browser snapshot -i
agent-browser click @e1
agent-browser wait --load networkidle
agent-browser snapshot -i
```

Refs are not stable across navigation or major DOM changes. Re-snapshot after:

- clicking a link or submit button
- route changes in a SPA
- opening or closing a modal
- lazy-loaded content appearing

## Relay mode vs managed mode

The ref model is the same in both modes. The only difference is where the page lives:

- relay mode uses the attached live tab
- managed mode uses the automation browser

## Good practice

- use fresh refs
- avoid guessing old refs after the page changes
- combine `snapshot -i` with `get url` and `get title` when you need to confirm you are still on the right page
