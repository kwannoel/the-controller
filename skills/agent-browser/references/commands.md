# Command Reference

`agent-browser` commands operate on:

- the attached live Chrome or Chromium tab in relay mode, or
- a dedicated automation browser in managed mode

## Core loop

```bash
agent-browser snapshot -i
agent-browser click @e1
agent-browser wait --load networkidle
agent-browser snapshot -i
```

Take a fresh snapshot after navigation or large DOM changes. Refs do not stay stable across page changes.

## Navigation

```bash
agent-browser open <url>
agent-browser get url
agent-browser get title
```

In relay mode, `open` navigates the attached tab. Only use it when you intend to change the live page.

## Interaction

```bash
agent-browser click @e1
agent-browser click @e1 --new-tab
agent-browser fill @e2 "text"
agent-browser type @e2 "text"
agent-browser select @e3 "option"
agent-browser check @e4
agent-browser press Enter
agent-browser keyboard type "text"
```

## Waiting

```bash
agent-browser wait @e1
agent-browser wait --load networkidle
agent-browser wait --url "**/dashboard"
agent-browser wait --text "Welcome"
agent-browser wait "#spinner" --state hidden
```

Prefer waits over fixed sleeps.

## Capture

```bash
agent-browser screenshot
agent-browser screenshot --full
agent-browser screenshot --annotate
agent-browser pdf output.pdf
```

## Close behavior

```bash
agent-browser close
```

In managed mode, `close` ends the automation browser session. In relay mode, avoid it unless the user explicitly wants the tab or browser closed.
