---
name: agent-browser
description: Use when the user wants browser automation against an existing Chrome or Chromium tab, especially to reuse a real logged-in session without reauthenticating. Triggers include requests to use the current browser, keep existing cookies or extensions, avoid login churn, or automate a live Chromium tab.
allowed-tools: Bash(npx agent-browser:*), Bash(agent-browser:*)
---

# Browser Automation with agent-browser

`agent-browser` is the Chromium skill. Prefer it when the user wants the agent to work against the browser they already have open and logged into.

The primary model is OpenClaw-style relay control:

1. Open the target site in Chrome or Chromium.
2. Attach the browser relay extension to the tab you want the agent to control.
3. Run `agent-browser` commands against that attached tab.
4. Detach when you are done.

If relay mode is unavailable, fall back to a managed dedicated browser profile.

## Mode Selection

### Relay mode

Use relay mode when:

- the user is already logged in
- the task depends on the user's real browser extensions, cookies, or in-progress state
- the user wants to avoid login churn

Relay mode means the agent controls a real Chrome or Chromium tab. Treat that tab as sensitive. Use the minimum scope needed and avoid unrelated tabs.

### Managed mode

Use managed mode when:

- the user wants isolation from their daily browser
- the relay is not installed or not attached
- the task needs a dedicated automation profile

Managed mode launches or reuses a dedicated browser profile instead of the user's live browsing tab.

## Core Workflow

Every `agent-browser` task follows this pattern:

1. Confirm the right tab is attached in relay mode, or start managed mode.
2. Snapshot the page: `agent-browser snapshot -i`
3. Interact with refs: `click`, `fill`, `select`, `check`
4. Re-snapshot after navigation or major DOM changes

```bash
# Attached tab is already open in Chrome/Chromium
agent-browser snapshot -i
# Output: @e1 [input type="email"], @e2 [input type="password"], @e3 [button] "Sign in"

agent-browser fill @e1 "user@example.com"
agent-browser fill @e2 "password123"
agent-browser click @e3
agent-browser wait --load networkidle
agent-browser snapshot -i
```

Do not close the browser in relay mode unless the user explicitly asks for that. In relay mode, prefer detach over `agent-browser close`.

## Quick Start

### Relay mode quick start

```bash
# 1. User opens the target site in Chrome/Chromium
# 2. User attaches the relay extension on the target tab

# 3. Inspect the attached tab
agent-browser snapshot -i

# 4. Interact
agent-browser click @e1
agent-browser wait --load networkidle
agent-browser screenshot current-tab.png
```

### Managed mode quick start

```bash
agent-browser --profile ~/.agent-browser/profiles/work open https://example.com
agent-browser wait --load networkidle
agent-browser snapshot -i
```

## Authentication

When a site requires login, prefer these approaches in this order:

### Option 1: Existing logged-in browser tab

This is the default. If the user already has the site open and authenticated in Chrome or Chromium, attach relay control to that tab and continue. This keeps the real session, cookies, local storage, browser extensions, and any in-progress state.

### Option 2: Managed dedicated profile

```bash
# First run: log in once
agent-browser --profile ~/.profiles/myapp open https://app.example.com/login

# Future runs: reuse the same dedicated profile
agent-browser --profile ~/.profiles/myapp open https://app.example.com/dashboard
```

Use this when relay mode is unavailable or the user wants isolation.

### Option 3: Saved or imported state

Use saved state only when relay mode and dedicated profiles are not a good fit.

```bash
# Import auth from an existing compatible browser session
agent-browser --auto-connect state save ./auth.json

# Reuse saved state
agent-browser --state ./auth.json open https://app.example.com/dashboard
```

State files contain session tokens in plaintext unless you add encryption. Keep them out of git and delete them when they are no longer needed.

See [references/authentication.md](references/authentication.md) for relay setup, fallback auth patterns, 2FA, and recovery steps.

## Essential Commands

```bash
# Navigation
agent-browser open <url>
agent-browser close

# Snapshot
agent-browser snapshot -i
agent-browser snapshot -s "#selector"

# Interaction
agent-browser click @e1
agent-browser click @e1 --new-tab
agent-browser fill @e2 "text"
agent-browser type @e2 "text"
agent-browser select @e1 "option"
agent-browser check @e1
agent-browser press Enter
agent-browser keyboard type "text"
agent-browser keyboard inserttext "text"
agent-browser scroll down 500
agent-browser scroll down 500 --selector "div.content"

# Information
agent-browser get text @e1
agent-browser get url
agent-browser get title
agent-browser get cdp-url

# Wait
agent-browser wait @e1
agent-browser wait --load networkidle
agent-browser wait --url "**/page"
agent-browser wait 2000
agent-browser wait --text "Welcome"
agent-browser wait --fn "!document.body.innerText.includes('Loading...')"
agent-browser wait "#spinner" --state hidden

# Capture
agent-browser screenshot
agent-browser screenshot --full
agent-browser screenshot --annotate
agent-browser pdf output.pdf
```

## Command Chaining

Commands can be chained with `&&` when you do not need to inspect intermediate output:

```bash
agent-browser open https://example.com && agent-browser wait --load networkidle && agent-browser screenshot page.png
```

Run commands separately when you need fresh refs from `snapshot -i`.

## Common Patterns

### Form submission on an attached tab

```bash
agent-browser snapshot -i
agent-browser fill @e1 "Jane Doe"
agent-browser fill @e2 "jane@example.com"
agent-browser click @e3
agent-browser wait --load networkidle
```

### Capture the current logged-in page

```bash
agent-browser get title
agent-browser screenshot --full page-full.png
agent-browser snapshot -i > page-structure.txt
```

### Managed isolated session

```bash
agent-browser --profile ~/.profiles/test-user open https://example.com
agent-browser wait --load networkidle
agent-browser snapshot -i
```

## Relay Safety

- Confirm the correct tab before interacting.
- Avoid unrelated tabs and accounts.
- Do not run `agent-browser close` in relay mode unless the user asked to close the browser or tab.
- Detach relay control after finishing.
- Switch to managed mode for sensitive or isolated workflows.

## References

- [references/authentication.md](references/authentication.md)
- [references/commands.md](references/commands.md)
- [references/session-management.md](references/session-management.md)
- [references/snapshot-refs.md](references/snapshot-refs.md)
