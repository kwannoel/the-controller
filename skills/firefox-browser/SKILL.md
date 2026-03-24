---
name: firefox-browser
description: Firefox browser automation for AI agents using Playwright. Use when the user needs Firefox-specific browser automation, including navigating pages, filling forms, clicking buttons, taking screenshots, extracting data, testing web apps, or any task requiring Firefox. Triggers include requests to "use Firefox", "open in Firefox", "test in Firefox", "automate Firefox", or any browser task where Firefox is specified or preferred.
allowed-tools: Bash(npx playwright:*), Bash(node -e:*), Bash(node <<:*)
---

# Firefox Browser Automation with Playwright

**Always use the user's real Firefox profile** instead of launching a fresh automated instance. Fresh automated browsers get flagged by bot detection (Google login bans, CAPTCHAs, account locks). Using the real profile inherits cookies, extensions, and fingerprint.

Uses Playwright's Firefox engine. Install with `npx playwright install firefox`.

## Setup: Find Your Firefox Profile

```bash
# macOS
ls ~/Library/Application\ Support/Firefox/Profiles/
# Look for: xxxxxxxx.default-release

# Linux
ls ~/.mozilla/firefox/
```

Set the `FIREFOX_PROFILE` env var for convenience:

```bash
# macOS
export FIREFOX_PROFILE="$HOME/Library/Application Support/Firefox/Profiles/YOUR_PROFILE.default-release"

# Linux
export FIREFOX_PROFILE="$HOME/.mozilla/firefox/YOUR_PROFILE.default-release"
```

## Core Pattern — Use Real Profile

Copy the user's profile to a temp directory (avoids lock conflicts with running Firefox) and launch with it:

```bash
node <<'SCRIPT'
const { firefox } = require('playwright');
const { execSync } = require('child_process');
const os = require('os');
const path = require('path');

(async () => {
  const sourceProfile = process.env.FIREFOX_PROFILE;
  const tempProfile = path.join(os.tmpdir(), 'firefox-automation-profile');

  // Copy profile (preserves cookies, extensions, history)
  execSync(`rm -rf "${tempProfile}" && cp -R "${sourceProfile}" "${tempProfile}"`);
  // Remove lock files and compatibility markers (Playwright's Firefox is a different version)
  execSync(`rm -f "${tempProfile}/lock" "${tempProfile}/.parentlock" "${tempProfile}/parent.lock" "${tempProfile}/compatibility.ini"`);

  const context = await firefox.launchPersistentContext(tempProfile, {
    headless: false,
  });

  const page = context.pages()[0] || await context.newPage();
  await page.goto('https://example.com');
  await page.waitForLoadState('domcontentloaded');

  console.log('Title:', await page.title());
  console.log('URL:', page.url());
  await page.screenshot({ path: '/tmp/firefox-screenshot.png' });

  await context.close();
})();
SCRIPT
```

**Key details:**
- Close or leave Firefox running — the profile copy avoids lock conflicts
- Remove `compatibility.ini` — Playwright's bundled Firefox is a different version than the system Firefox
- Use `domcontentloaded` instead of `networkidle` — heavy SPAs (Gmail, etc.) never reach network idle
- The user may already be logged in to sites via their cookies

## Reusing a Copied Profile

If the temp profile already exists from a previous run, reuse it (skip the copy):

```bash
node <<'SCRIPT'
const { firefox } = require('playwright');
const os = require('os');
const path = require('path');
const fs = require('fs');

(async () => {
  const tempProfile = path.join(os.tmpdir(), 'firefox-automation-profile');

  if (!fs.existsSync(tempProfile)) {
    console.error('No cached profile found. Run the full copy pattern first.');
    process.exit(1);
  }

  // Clean stale locks from previous run
  const { execSync } = require('child_process');
  execSync(`rm -f "${tempProfile}/lock" "${tempProfile}/.parentlock" "${tempProfile}/parent.lock"`);

  const context = await firefox.launchPersistentContext(tempProfile, {
    headless: false,
  });

  const page = context.pages()[0] || await context.newPage();
  await page.goto('https://example.com');
  await page.waitForLoadState('domcontentloaded');

  console.log('Title:', await page.title());
  await context.close();
})();
SCRIPT
```

## CLI Commands (Simple Tasks)

For one-shot tasks that don't need authentication, CLI commands still work:

```bash
# Screenshot
npx playwright screenshot --browser firefox https://example.com screenshot.png
npx playwright screenshot --browser firefox --full-page https://example.com full.png
npx playwright screenshot --browser firefox --wait-for-selector "#content" https://example.com content.png

# PDF
npx playwright pdf --browser firefox https://example.com output.pdf

# With options
npx playwright screenshot --browser firefox --device "iPhone 14" https://example.com mobile.png
npx playwright screenshot --browser firefox --color-scheme dark https://example.com dark.png
npx playwright screenshot --browser firefox --viewport-size "1920,1080" https://example.com desktop.png

# With saved auth state
npx playwright screenshot --browser firefox --load-storage auth.json https://example.com/dashboard dash.png
```

## Common Patterns

### Form Submission

```bash
node <<'SCRIPT'
const { firefox } = require('playwright');
const { execSync } = require('child_process');
const os = require('os');
const path = require('path');

(async () => {
  const sourceProfile = process.env.FIREFOX_PROFILE;
  const tempProfile = path.join(os.tmpdir(), 'firefox-automation-profile');

  execSync(`rm -rf "${tempProfile}" && cp -R "${sourceProfile}" "${tempProfile}"`);
  execSync(`rm -f "${tempProfile}/lock" "${tempProfile}/.parentlock" "${tempProfile}/parent.lock" "${tempProfile}/compatibility.ini"`);

  const context = await firefox.launchPersistentContext(tempProfile, {
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

### Data Extraction

```bash
node <<'SCRIPT'
const { firefox } = require('playwright');
const { execSync } = require('child_process');
const os = require('os');
const path = require('path');

(async () => {
  const sourceProfile = process.env.FIREFOX_PROFILE;
  const tempProfile = path.join(os.tmpdir(), 'firefox-automation-profile');

  execSync(`rm -rf "${tempProfile}" && cp -R "${sourceProfile}" "${tempProfile}"`);
  execSync(`rm -f "${tempProfile}/lock" "${tempProfile}/.parentlock" "${tempProfile}/parent.lock" "${tempProfile}/compatibility.ini"`);

  const context = await firefox.launchPersistentContext(tempProfile, {
    headless: false,
  });
  const page = context.pages()[0] || await context.newPage();

  await page.goto('https://example.com/products');
  await page.waitForLoadState('domcontentloaded');

  const text = await page.textContent('body');
  console.log(text);

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

### Authentication with State Persistence

Since you're using the real profile, the user is likely already logged in. If you need to save/restore state explicitly:

```bash
# Step 1: Login and save state
node <<'SCRIPT'
const { firefox } = require('playwright');
const { execSync } = require('child_process');
const os = require('os');
const path = require('path');

(async () => {
  const sourceProfile = process.env.FIREFOX_PROFILE;
  const tempProfile = path.join(os.tmpdir(), 'firefox-automation-profile');

  execSync(`rm -rf "${tempProfile}" && cp -R "${sourceProfile}" "${tempProfile}"`);
  execSync(`rm -f "${tempProfile}/lock" "${tempProfile}/.parentlock" "${tempProfile}/parent.lock" "${tempProfile}/compatibility.ini"`);

  const context = await firefox.launchPersistentContext(tempProfile, {
    headless: false,
  });
  const page = context.pages()[0] || await context.newPage();

  await page.goto('https://app.example.com/login');
  await page.waitForLoadState('domcontentloaded');

  await page.fill('input[name="email"]', process.env.APP_USERNAME);
  await page.fill('input[name="password"]', process.env.APP_PASSWORD);
  await page.click('button[type="submit"]');
  await page.waitForURL('**/dashboard');

  await context.storageState({ path: './auth.json' });
  console.log('Auth state saved to ./auth.json');

  await context.close();
})();
SCRIPT

# Step 2: Reuse auth state in future runs
node -e "
const { firefox } = require('playwright');
(async () => {
  const browser = await firefox.launch({ headless: false });
  const context = await browser.newContext({ storageState: './auth.json' });
  const page = await context.newPage();
  await page.goto('https://app.example.com/dashboard');
  console.log('Title:', await page.title());
  await browser.close();
})();
"
```

### Screenshots and Capture

```bash
node <<'SCRIPT'
const { firefox } = require('playwright');
const { execSync } = require('child_process');
const os = require('os');
const path = require('path');

(async () => {
  const sourceProfile = process.env.FIREFOX_PROFILE;
  const tempProfile = path.join(os.tmpdir(), 'firefox-automation-profile');

  execSync(`rm -rf "${tempProfile}" && cp -R "${sourceProfile}" "${tempProfile}"`);
  execSync(`rm -f "${tempProfile}/lock" "${tempProfile}/.parentlock" "${tempProfile}/parent.lock" "${tempProfile}/compatibility.ini"`);

  const context = await firefox.launchPersistentContext(tempProfile, {
    headless: false,
  });
  const page = context.pages()[0] || await context.newPage();

  await page.goto('https://example.com');
  await page.waitForLoadState('domcontentloaded');

  // Viewport screenshot
  await page.screenshot({ path: 'viewport.png' });

  // Full page
  await page.screenshot({ path: 'full.png', fullPage: true });

  // Specific element
  const el = page.locator('header');
  await el.screenshot({ path: 'header.png' });

  // PDF
  await page.pdf({ path: 'page.pdf', format: 'A4' });

  await context.close();
})();
SCRIPT
```

### Waiting Strategies

```javascript
// Wait for DOM content loaded (recommended default)
await page.waitForLoadState('domcontentloaded');

// Wait for selector
await page.waitForSelector('#content');

// Wait for text
await page.waitForSelector('text=Welcome');

// Wait for URL pattern
await page.waitForURL('**/dashboard');

// Wait for element to disappear
await page.waitForSelector('#spinner', { state: 'hidden' });

// Wait for JS condition
await page.waitForFunction(() => document.readyState === 'complete');

// Wait for network idle (avoid for heavy SPAs)
await page.waitForLoadState('networkidle');

// Fixed wait (last resort)
await page.waitForTimeout(2000);
```

### Viewport and Device Emulation

```bash
node <<'SCRIPT'
const { firefox, devices } = require('playwright');
(async () => {
  const browser = await firefox.launch({ headless: false });

  // Custom viewport
  const page = await browser.newPage({ viewport: { width: 1920, height: 1080 } });
  await page.goto('https://example.com');
  await page.screenshot({ path: 'desktop.png' });

  // Mobile device emulation
  const iPhone = devices['iPhone 14'];
  const mobile = await browser.newContext({ ...iPhone });
  const mobilePage = await mobile.newPage();
  await mobilePage.goto('https://example.com');
  await mobilePage.screenshot({ path: 'mobile.png' });

  await browser.close();
})();
SCRIPT
```

### Network Interception

```bash
node <<'SCRIPT'
const { firefox } = require('playwright');
(async () => {
  const browser = await firefox.launch({ headless: false });
  const page = await browser.newPage();

  // Log all requests
  page.on('request', req => console.log('>>', req.method(), req.url()));
  page.on('response', res => console.log('<<', res.status(), res.url()));

  // Block specific requests
  await page.route('**/*.{png,jpg,jpeg,gif}', route => route.abort());

  // Mock API responses
  await page.route('**/api/data', route =>
    route.fulfill({ contentType: 'application/json', body: JSON.stringify({ mock: true }) })
  );

  await page.goto('https://example.com');
  await page.waitForLoadState('domcontentloaded');

  await browser.close();
})();
SCRIPT
```

### Multiple Tabs

```bash
node <<'SCRIPT'
const { firefox } = require('playwright');
const { execSync } = require('child_process');
const os = require('os');
const path = require('path');

(async () => {
  const sourceProfile = process.env.FIREFOX_PROFILE;
  const tempProfile = path.join(os.tmpdir(), 'firefox-automation-profile');

  execSync(`rm -rf "${tempProfile}" && cp -R "${sourceProfile}" "${tempProfile}"`);
  execSync(`rm -f "${tempProfile}/lock" "${tempProfile}/.parentlock" "${tempProfile}/parent.lock" "${tempProfile}/compatibility.ini"`);

  const context = await firefox.launchPersistentContext(tempProfile, {
    headless: false,
  });

  const page1 = context.pages()[0] || await context.newPage();
  await page1.goto('https://example.com');

  const page2 = await context.newPage();
  await page2.goto('https://example.org');

  console.log('Tab 1:', await page1.title());
  console.log('Tab 2:', await page2.title());

  await context.close();
})();
SCRIPT
```

### Dark Mode

```bash
# CLI
npx playwright screenshot --browser firefox --color-scheme dark https://example.com dark.png

# Script (with real profile)
node <<'SCRIPT'
const { firefox } = require('playwright');
const { execSync } = require('child_process');
const os = require('os');
const path = require('path');

(async () => {
  const sourceProfile = process.env.FIREFOX_PROFILE;
  const tempProfile = path.join(os.tmpdir(), 'firefox-automation-profile');

  execSync(`rm -rf "${tempProfile}" && cp -R "${sourceProfile}" "${tempProfile}"`);
  execSync(`rm -f "${tempProfile}/lock" "${tempProfile}/.parentlock" "${tempProfile}/parent.lock" "${tempProfile}/compatibility.ini"`);

  const context = await firefox.launchPersistentContext(tempProfile, {
    headless: false,
    colorScheme: 'dark',
  });
  const page = context.pages()[0] || await context.newPage();
  await page.goto('https://example.com');
  await page.screenshot({ path: 'dark.png' });
  await context.close();
})();
SCRIPT
```

### Proxy

```bash
node <<'SCRIPT'
const { firefox } = require('playwright');
const { execSync } = require('child_process');
const os = require('os');
const path = require('path');

(async () => {
  const sourceProfile = process.env.FIREFOX_PROFILE;
  const tempProfile = path.join(os.tmpdir(), 'firefox-automation-profile');

  execSync(`rm -rf "${tempProfile}" && cp -R "${sourceProfile}" "${tempProfile}"`);
  execSync(`rm -f "${tempProfile}/lock" "${tempProfile}/.parentlock" "${tempProfile}/parent.lock" "${tempProfile}/compatibility.ini"`);

  const context = await firefox.launchPersistentContext(tempProfile, {
    headless: false,
    proxy: { server: 'http://localhost:8080' },
  });
  const page = context.pages()[0] || await context.newPage();
  await page.goto('https://example.com');
  console.log(await page.title());
  await context.close();
})();
SCRIPT
```

### Dialogs (alert / confirm / prompt)

```bash
node <<'SCRIPT'
const { firefox } = require('playwright');
(async () => {
  const browser = await firefox.launch({ headless: false });
  const page = await browser.newPage();

  page.on('dialog', async dialog => {
    console.log(`Dialog: ${dialog.type()} - ${dialog.message()}`);
    await dialog.accept();
  });

  await page.goto('https://example.com');
  await browser.close();
})();
SCRIPT
```

### HAR Recording

```bash
node <<'SCRIPT'
const { firefox } = require('playwright');
const { execSync } = require('child_process');
const os = require('os');
const path = require('path');

(async () => {
  const sourceProfile = process.env.FIREFOX_PROFILE;
  const tempProfile = path.join(os.tmpdir(), 'firefox-automation-profile');

  execSync(`rm -rf "${tempProfile}" && cp -R "${sourceProfile}" "${tempProfile}"`);
  execSync(`rm -f "${tempProfile}/lock" "${tempProfile}/.parentlock" "${tempProfile}/parent.lock" "${tempProfile}/compatibility.ini"`);

  const context = await firefox.launchPersistentContext(tempProfile, {
    headless: false,
    recordHar: { path: 'network.har' },
  });
  const page = context.pages()[0] || await context.newPage();
  await page.goto('https://example.com');
  await page.waitForLoadState('domcontentloaded');

  await context.close();
  console.log('HAR saved to network.har');
})();
SCRIPT
```

### Video Recording

```bash
node <<'SCRIPT'
const { firefox } = require('playwright');
const { execSync } = require('child_process');
const os = require('os');
const path = require('path');

(async () => {
  const sourceProfile = process.env.FIREFOX_PROFILE;
  const tempProfile = path.join(os.tmpdir(), 'firefox-automation-profile');

  execSync(`rm -rf "${tempProfile}" && cp -R "${sourceProfile}" "${tempProfile}"`);
  execSync(`rm -f "${tempProfile}/lock" "${tempProfile}/.parentlock" "${tempProfile}/parent.lock" "${tempProfile}/compatibility.ini"`);

  const context = await firefox.launchPersistentContext(tempProfile, {
    headless: false,
    recordVideo: { dir: './videos/', size: { width: 1280, height: 720 } },
  });
  const page = context.pages()[0] || await context.newPage();
  await page.goto('https://example.com');
  await page.waitForTimeout(3000);

  await context.close();
  console.log('Video saved to ./videos/');
})();
SCRIPT
```

## Locator Strategies

Playwright uses locators instead of `@ref` element references. Prefer semantic locators:

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
page.locator('#main-form input[type="email"]')

// Chaining
page.locator('.card').filter({ hasText: 'Premium' }).getByRole('button')
```

## Key Differences from agent-browser

Use `agent-browser` when the user says some version of "use my current browser" or needs an existing logged-in Chromium tab. Use `firefox-browser` when the task specifically needs Firefox or a managed Playwright session.

| Feature | agent-browser | firefox-browser |
|---------|---------------|-----------------|
| Primary use | Existing logged-in Chrome or Chromium tab via relay | Managed Firefox automation with Playwright |
| Browser reuse | Best option for "use my current browser" | Use persistent profiles or saved state, not live-tab relay |
| Interaction | CLI commands with `@refs` | Node.js scripts with Playwright locators |
| Session model | Attached live tab or dedicated Chromium profile | Playwright browser or persistent Firefox profile |
| Simple tasks | `agent-browser screenshot` on the current tab | `npx playwright screenshot -b firefox` |
| Complex tasks | Snapshot and interact on the attached tab | Inline Node.js scripts |

## Setup

```bash
# Install Firefox for Playwright
npx playwright install firefox

# Verify
npx playwright screenshot --browser firefox https://example.com /tmp/test.png
```
