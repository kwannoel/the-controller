---
name: agent-browser
description: Browser automation CLI for AI agents. Use when the user needs to interact with websites, including navigating pages, filling forms, clicking buttons, taking screenshots, extracting data, testing web apps, or automating any browser task. Triggers include requests to "open a website", "fill out a form", "click a button", "take a screenshot", "scrape data from a page", "test this web app", "login to a site", "automate browser actions", or any task requiring programmatic web interaction.
allowed-tools: Bash(npx agent-browser:*), Bash(agent-browser:*), Bash(node -e:*), Bash(node <<:*)
---

# Browser Automation — Connect to Existing Browser

**Always connect to the user's existing browser** instead of launching a new automated instance. Fresh automated browsers get flagged by bot detection (Google login bans, CAPTCHAs, account locks). Connecting to a real browser inherits the user's profile, cookies, extensions, and fingerprint.

Two backends:
- **Chrome** — `agent-browser` CLI with `--auto-connect` (preferred for most tasks)
- **Firefox** — Playwright with the user's real Firefox profile

## Setup

Before automating, the user must start their browser with remote debugging enabled.

### Chrome

```bash
# macOS
"/Applications/Google Chrome.app/Contents/MacOS/Google Chrome" --remote-debugging-port=9222

# Linux
google-chrome --remote-debugging-port=9222
```

If Chrome is already running, restart it with the flag. To use a separate debug instance:

```bash
"/Applications/Google Chrome.app/Contents/MacOS/Google Chrome" --remote-debugging-port=9222 --user-data-dir="$HOME/.chrome-debug"
```

> **Security note:** `--remote-debugging-port` exposes full browser control on localhost. Any local process can connect and read cookies, execute JS, etc. Only use on trusted machines and close Chrome when done.

### Firefox

```bash
# macOS
/Applications/Firefox.app/Contents/MacOS/firefox --start-debugger-server 9222

# Linux
firefox --start-debugger-server 9222
```

Or enable permanently: `about:config` → set `devtools.debugger.remote-enabled` to `true` and `devtools.debugger.prompt-connection` to `false`.

### Verify Connection

```bash
# Chrome: should return JSON with browser tabs
curl -s http://localhost:9222/json/version

# Firefox: test with Playwright
node -e "const { firefox } = require('playwright'); (async () => { const b = await firefox.launch({ headless: false }); console.log('OK'); await b.close(); })();"
```

## Chrome Workflow (agent-browser CLI)

Install: `npm i -g agent-browser` or `brew install agent-browser`

### Core Workflow

Every automation follows this pattern — always use `--auto-connect` to attach to the user's running Chrome:

1. **Connect + Navigate**: `agent-browser --auto-connect open <url>`
2. **Snapshot**: `agent-browser snapshot -i` (get element refs like `@e1`, `@e2`)
3. **Interact**: Use refs to click, fill, select
4. **Re-snapshot**: After navigation or DOM changes, get fresh refs

```bash
agent-browser --auto-connect open https://example.com/form
agent-browser snapshot -i
# Output: @e1 [input type="email"], @e2 [input type="password"], @e3 [button] "Submit"

agent-browser fill @e1 "user@example.com"
agent-browser fill @e2 "password123"
agent-browser click @e3
agent-browser wait --load networkidle
agent-browser snapshot -i  # Check result
```

### Command Chaining

Commands can be chained with `&&`. The browser persists between commands via a background daemon:

```bash
# Connect + wait + snapshot in one call
agent-browser --auto-connect open https://example.com && agent-browser wait --load networkidle && agent-browser snapshot -i

# Chain multiple interactions
agent-browser fill @e1 "user@example.com" && agent-browser fill @e2 "password123" && agent-browser click @e3
```

**When to chain:** Use `&&` when you don't need to read intermediate output. Run commands separately when you need to parse output first (e.g., snapshot to discover refs, then interact).

### Connect to Specific Port

```bash
# If auto-connect doesn't find the right browser, specify the port
agent-browser --cdp 9222 open https://example.com
agent-browser --cdp 9222 snapshot -i
```

### Essential Commands

```bash
# Navigation
agent-browser open <url>              # Navigate (aliases: goto, navigate)
agent-browser close                   # Close browser session (not the user's browser)

# Snapshot
agent-browser snapshot -i             # Interactive elements with refs (recommended)
agent-browser snapshot -s "#selector" # Scope to CSS selector

# Interaction (use @refs from snapshot)
agent-browser click @e1               # Click element
agent-browser click @e1 --new-tab     # Click and open in new tab
agent-browser fill @e2 "text"         # Clear and type text
agent-browser type @e2 "text"         # Type without clearing
agent-browser select @e1 "option"     # Select dropdown option
agent-browser check @e1               # Check checkbox
agent-browser press Enter             # Press key
agent-browser keyboard type "text"    # Type at current focus (no selector)
agent-browser scroll down 500         # Scroll page
agent-browser scroll down 500 --selector "div.content"  # Scroll within container

# Get information
agent-browser get text @e1            # Get element text
agent-browser get url                 # Get current URL
agent-browser get title               # Get page title

# Wait
agent-browser wait @e1                # Wait for element
agent-browser wait --load networkidle # Wait for network idle
agent-browser wait --url "**/page"    # Wait for URL pattern
agent-browser wait 2000               # Wait milliseconds
agent-browser wait --text "Welcome"   # Wait for text to appear
agent-browser wait "#spinner" --state hidden  # Wait for element to disappear

# Downloads
agent-browser download @e1 ./file.pdf          # Click to trigger download
agent-browser wait --download ./output.zip     # Wait for download

# Capture
agent-browser screenshot              # Screenshot to temp dir
agent-browser screenshot --full       # Full page screenshot
agent-browser screenshot --annotate   # Annotated screenshot with numbered labels
agent-browser pdf output.pdf          # Save as PDF

# Clipboard
agent-browser clipboard read          # Read text from clipboard
agent-browser clipboard write "text"  # Write text to clipboard

# Dialogs (alert, confirm, prompt)
agent-browser dialog accept           # Accept dialog
agent-browser dialog dismiss          # Dismiss dialog
agent-browser dialog status           # Check if dialog is open
```

### Handling Authentication

Since you're connected to the user's real browser, they may already be logged in. Check first:

```bash
agent-browser --auto-connect open https://app.example.com/dashboard
agent-browser get url  # If not redirected to login, you're already authenticated
```

If you need to save/restore auth state across sessions:

**Option 1: Save state from connected browser (fastest)**

```bash
# Grab cookies + localStorage from the user's browser
agent-browser --auto-connect state save ./auth.json

# Reuse later
agent-browser --auto-connect state load ./auth.json
agent-browser open https://app.example.com/dashboard
```

**Option 2: Session name (auto-save/restore)**

```bash
agent-browser --auto-connect --session-name myapp open https://app.example.com
# State auto-saved on close, auto-restored next time
```

**Option 3: Persistent profile**

```bash
agent-browser --auto-connect --profile ~/.myapp open https://app.example.com
```

See [references/authentication.md](references/authentication.md) for OAuth, 2FA, cookie-based auth, and token refresh patterns.

### Batch Execution

```bash
echo '[
  ["open", "https://example.com"],
  ["snapshot", "-i"],
  ["click", "@e1"],
  ["screenshot", "result.png"]
]' | agent-browser --auto-connect batch --json
```

### Common Patterns

#### Form Submission

```bash
agent-browser --auto-connect open https://example.com/signup
agent-browser snapshot -i
agent-browser fill @e1 "Jane Doe"
agent-browser fill @e2 "jane@example.com"
agent-browser select @e3 "California"
agent-browser check @e4
agent-browser click @e5
agent-browser wait --load networkidle
```

#### Data Extraction

```bash
agent-browser --auto-connect open https://example.com/products
agent-browser snapshot -i
agent-browser get text @e5           # Get specific element text
agent-browser get text body > page.txt  # Get all page text

# JSON output for parsing
agent-browser snapshot -i --json
```

#### Working with Iframes

Iframe content is automatically inlined in snapshots. Refs inside iframes carry frame context:

```bash
agent-browser --auto-connect open https://example.com/checkout
agent-browser snapshot -i
# @e1 [heading] "Checkout"
# @e2 [Iframe] "payment-frame"
#   @e3 [input] "Card number"
#   @e4 [input] "Expiry"
#   @e5 [button] "Pay"

# Interact directly — no frame switch needed
agent-browser fill @e3 "4111111111111111"
agent-browser fill @e4 "12/28"
agent-browser click @e5
```

#### Parallel Sessions

```bash
agent-browser --auto-connect --session site1 open https://site-a.com
agent-browser --auto-connect --session site2 open https://site-b.com

agent-browser --session site1 snapshot -i
agent-browser --session site2 snapshot -i

agent-browser session list
```

### Viewport & Responsive Testing

```bash
agent-browser set viewport 1920 1080
agent-browser screenshot desktop.png

agent-browser set viewport 375 812
agent-browser screenshot mobile.png

# Retina: same CSS layout at 2x pixel density
agent-browser set viewport 1920 1080 2
agent-browser screenshot retina.png

# Device emulation (viewport + user agent)
agent-browser set device "iPhone 14"
agent-browser screenshot device.png
```

### Visual Browser (Debugging)

```bash
agent-browser --auto-connect --headed open https://example.com
agent-browser highlight @e1          # Highlight element
agent-browser inspect                # Open Chrome DevTools
agent-browser record start demo.webm
```

### JavaScript Evaluation

Use `eval` to run JavaScript in the browser context. Use `--stdin` to avoid shell quoting issues:

```bash
# Simple expressions
agent-browser eval 'document.title'

# Complex JS: use --stdin with heredoc
agent-browser eval --stdin <<'EVALEOF'
JSON.stringify(
  Array.from(document.querySelectorAll("img"))
    .filter(i => !i.alt)
    .map(i => ({ src: i.src.split("/").pop(), width: i.width }))
)
EVALEOF
```

### Diffing (Verifying Changes)

```bash
agent-browser snapshot -i          # Baseline
agent-browser click @e2            # Action
agent-browser diff snapshot        # See what changed
```

### Semantic Locators (Alternative to Refs)

When refs are unavailable or unreliable:

```bash
agent-browser find text "Sign In" click
agent-browser find label "Email" fill "user@test.com"
agent-browser find role button click --name "Submit"
agent-browser find placeholder "Search" type "query"
agent-browser find testid "submit-btn" click
```

### Session Cleanup

Always close your session when done (this does NOT close the user's browser — it releases the automation connection):

```bash
agent-browser close                    # Close default session
agent-browser --session site1 close    # Close specific session
```

## Firefox Workflow (Playwright)

For Firefox, use Playwright with the user's real profile. This preserves cookies, extensions, and browser fingerprint.

### Find Your Firefox Profile Path

```bash
# macOS
ls ~/Library/Application\ Support/Firefox/Profiles/

# Linux
ls ~/.mozilla/firefox/

# The profile directory looks like: abc123xy.default-release
```

### Core Pattern — Persistent Context

```bash
node <<'SCRIPT'
const { firefox } = require('playwright');
(async () => {
  // IMPORTANT: Close Firefox first — profile directories are locked while Firefox is running
  const profilePath = process.env.FIREFOX_PROFILE || `${process.env.HOME}/Library/Application Support/Firefox/Profiles/YOUR_PROFILE.default-release`;

  const context = await firefox.launchPersistentContext(profilePath, {
    headless: false,  // Use headed mode to see what's happening
  });

  const page = context.pages()[0] || await context.newPage();
  await page.goto('https://example.com');
  await page.waitForLoadState('domcontentloaded');

  console.log('Title:', await page.title());
  console.log('URL:', page.url());

  // Take screenshot
  await page.screenshot({ path: '/tmp/firefox-screenshot.png' });

  await context.close();
})();
SCRIPT
```

> **Note:** Firefox profiles are locked while Firefox is running. Close Firefox before using `launchPersistentContext`, or copy the profile to a temp directory first. Use `domcontentloaded` instead of `networkidle` — heavy SPAs like Gmail never reach network idle.

### Copy Profile to Avoid Lock

```bash
node <<'SCRIPT'
const { firefox } = require('playwright');
const { execSync } = require('child_process');
const os = require('os');
const path = require('path');

(async () => {
  const sourceProfile = process.env.FIREFOX_PROFILE || `${process.env.HOME}/Library/Application Support/Firefox/Profiles/YOUR_PROFILE.default-release`;
  const tempProfile = path.join(os.tmpdir(), 'firefox-automation-profile');

  // Copy profile (preserves cookies, extensions, etc.)
  execSync(`rm -rf "${tempProfile}" && cp -R "${sourceProfile}" "${tempProfile}"`);
  // Remove lock files and compatibility markers (Playwright's Firefox is a different version)
  execSync(`rm -f "${tempProfile}/lock" "${tempProfile}/.parentlock" "${tempProfile}/parent.lock" "${tempProfile}/compatibility.ini"`);

  const context = await firefox.launchPersistentContext(tempProfile, {
    headless: false,
  });

  const page = context.pages()[0] || await context.newPage();
  await page.goto('https://example.com');
  console.log('Title:', await page.title());

  await context.close();
})();
SCRIPT
```

### Form Submission (Firefox)

```bash
node <<'SCRIPT'
const { firefox } = require('playwright');
(async () => {
  const context = await firefox.launchPersistentContext(process.env.FIREFOX_PROFILE, {
    headless: false,
  });
  const page = context.pages()[0] || await context.newPage();

  await page.goto('https://example.com/signup');
  await page.waitForLoadState('domcontentloaded');

  await page.fill('input[name="name"]', 'Jane Doe');
  await page.fill('input[name="email"]', 'jane@example.com');
  await page.selectOption('select[name="state"]', 'California');
  await page.check('input[type="checkbox"]');
  await page.click('button[type="submit"]');
  await page.waitForLoadState('domcontentloaded');

  console.log('URL:', page.url());
  await page.screenshot({ path: '/tmp/form-result.png' });

  await context.close();
})();
SCRIPT
```

### Save/Restore Auth State (Firefox)

```bash
# Login and save state
node <<'SCRIPT'
const { firefox } = require('playwright');
(async () => {
  const browser = await firefox.launch({ headless: false });
  const context = await browser.newContext();
  const page = await context.newPage();

  await page.goto('https://app.example.com/login');
  await page.fill('input[name="email"]', process.env.APP_USERNAME);
  await page.fill('input[name="password"]', process.env.APP_PASSWORD);
  await page.click('button[type="submit"]');
  await page.waitForURL('**/dashboard');

  await context.storageState({ path: './firefox-auth.json' });
  console.log('Auth state saved');
  await browser.close();
})();
SCRIPT

# Reuse auth state
node -e "
const { firefox } = require('playwright');
(async () => {
  const browser = await firefox.launch({ headless: false });
  const context = await browser.newContext({ storageState: './firefox-auth.json' });
  const page = await context.newPage();
  await page.goto('https://app.example.com/dashboard');
  console.log('Title:', await page.title());
  await browser.close();
})();
"
```

### Data Extraction (Firefox)

```bash
node <<'SCRIPT'
const { firefox } = require('playwright');
(async () => {
  const context = await firefox.launchPersistentContext(process.env.FIREFOX_PROFILE, {
    headless: false,
  });
  const page = context.pages()[0] || await context.newPage();

  await page.goto('https://example.com/products');
  await page.waitForLoadState('domcontentloaded');

  const items = await page.$$eval('.product', els =>
    els.map(el => ({
      name: el.querySelector('h2')?.textContent,
      price: el.querySelector('.price')?.textContent,
    }))
  );
  console.log(JSON.stringify(items, null, 2));

  await context.close();
})();
SCRIPT
```

### Locator Strategies (Firefox)

Playwright uses locators instead of `@ref` element references:

```javascript
// By role (recommended)
page.getByRole('button', { name: 'Submit' })
page.getByRole('textbox', { name: 'Email' })
page.getByRole('link', { name: 'Home' })

// By label
page.getByLabel('Email address')

// By placeholder
page.getByPlaceholder('Enter your email')

// By text
page.getByText('Welcome')

// By test ID
page.getByTestId('submit-btn')

// CSS selector (fallback)
page.locator('div.content > p')

// Chaining
page.locator('.card').filter({ hasText: 'Premium' }).getByRole('button')
```

## Which Backend to Use

| Situation | Use |
|-----------|-----|
| Quick navigation, screenshots, form filling | Chrome (`agent-browser --auto-connect`) |
| Already logged into Chrome | Chrome (`--auto-connect` inherits session) |
| Need Firefox specifically | Firefox (Playwright persistent context) |
| Complex multi-step automation | Chrome (CLI chaining is simpler) |
| Need programmatic control (loops, conditionals) | Firefox (full Node.js scripting) |

## Security

All security features are opt-in.

### Content Boundaries (Recommended for AI Agents)

```bash
export AGENT_BROWSER_CONTENT_BOUNDARIES=1
agent-browser --auto-connect snapshot
```

### Domain Allowlist

```bash
export AGENT_BROWSER_ALLOWED_DOMAINS="example.com,*.example.com"
```

### Action Policy

```bash
export AGENT_BROWSER_ACTION_POLICY=./policy.json
```

### Output Limits

```bash
export AGENT_BROWSER_MAX_OUTPUT=50000
```

## Configuration File

Create `agent-browser.json` in the project root for persistent settings:

```json
{
  "autoConnect": true,
  "headed": true,
  "proxy": "http://localhost:8080"
}
```

Priority (lowest to highest): `~/.agent-browser/config.json` < `./agent-browser.json` < env vars < CLI flags.

## Timeouts and Slow Pages

Default timeout is 25 seconds. Override with `AGENT_BROWSER_DEFAULT_TIMEOUT` (milliseconds). For slow pages, use explicit waits:

```bash
agent-browser wait --load networkidle
agent-browser wait "#content"
agent-browser wait --url "**/dashboard"
agent-browser wait --fn "document.readyState === 'complete'"
```

## Ref Lifecycle (Important)

Refs (`@e1`, `@e2`, etc.) are invalidated when the page changes. Always re-snapshot after:
- Clicking links/buttons that navigate
- Form submissions
- Dynamic content loading (dropdowns, modals)

## Deep-Dive Documentation

| Reference | When to Use |
|-----------|-------------|
| [references/commands.md](references/commands.md) | Full command reference with all options |
| [references/snapshot-refs.md](references/snapshot-refs.md) | Ref lifecycle, invalidation rules |
| [references/session-management.md](references/session-management.md) | Parallel sessions, state persistence |
| [references/authentication.md](references/authentication.md) | Login flows, OAuth, 2FA, state reuse |
| [references/video-recording.md](references/video-recording.md) | Recording workflows |
| [references/profiling.md](references/profiling.md) | Chrome DevTools profiling |
| [references/proxy-support.md](references/proxy-support.md) | Proxy configuration |

## Ready-to-Use Templates

| Template | Description |
|----------|-------------|
| [templates/form-automation.sh](templates/form-automation.sh) | Form filling with validation |
| [templates/authenticated-session.sh](templates/authenticated-session.sh) | Login once, reuse state |
| [templates/capture-workflow.sh](templates/capture-workflow.sh) | Content extraction with screenshots |
