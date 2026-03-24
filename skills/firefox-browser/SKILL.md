---
name: firefox-browser
description: Firefox browser automation for AI agents using Playwright. Use when the user needs Firefox-specific browser automation, including navigating pages, filling forms, clicking buttons, taking screenshots, extracting data, testing web apps, or any task requiring Firefox. Triggers include requests to "use Firefox", "open in Firefox", "test in Firefox", "automate Firefox", or any browser task where Firefox is specified or preferred.
allowed-tools: Bash(npx playwright:*), Bash(node -e:*), Bash(node <<:*)
---

# Firefox Browser Automation with Playwright

Uses Playwright's Firefox engine for browser automation. Install Firefox with `npx playwright install firefox`.

Two modes of operation:

1. **CLI commands** — one-shot tasks (screenshots, PDFs)
2. **Inline scripts** — interactive automation (forms, clicks, scraping)

## CLI Commands (Simple Tasks)

```bash
# Screenshot
npx playwright screenshot --browser firefox https://example.com screenshot.png
npx playwright screenshot --browser firefox --full-page https://example.com full.png
npx playwright screenshot --browser firefox --wait-for-timeout 3000 https://example.com delayed.png
npx playwright screenshot --browser firefox --wait-for-selector "#content" https://example.com content.png

# PDF
npx playwright pdf --browser firefox https://example.com output.pdf

# With options
npx playwright screenshot --browser firefox --device "iPhone 14" https://example.com mobile.png
npx playwright screenshot --browser firefox --color-scheme dark https://example.com dark.png
npx playwright screenshot --browser firefox --viewport-size "1920,1080" https://example.com desktop.png

# With auth state
npx playwright screenshot --browser firefox --load-storage auth.json https://example.com/dashboard dash.png

# Save HAR (network log)
npx playwright screenshot --browser firefox --save-har network.har https://example.com shot.png
```

## Inline Scripts (Interactive Automation)

For anything beyond screenshots/PDFs, write inline Playwright scripts. The browser persists for the duration of the script.

### Core Pattern

```bash
node -e "
const { firefox } = require('playwright');
(async () => {
  const browser = await firefox.launch();
  const page = await browser.newPage();
  await page.goto('https://example.com');
  console.log(await page.title());
  await browser.close();
})();
"
```

For complex scripts, use a heredoc to avoid quoting issues:

```bash
node <<'SCRIPT'
const { firefox } = require('playwright');
(async () => {
  const browser = await firefox.launch();
  const page = await browser.newPage();
  await page.goto('https://example.com');

  // Your automation here
  console.log(await page.title());

  await browser.close();
})();
SCRIPT
```

### Headed Mode (Visible Browser)

```bash
node -e "
const { firefox } = require('playwright');
(async () => {
  const browser = await firefox.launch({ headless: false });
  const page = await browser.newPage();
  await page.goto('https://example.com');
  await page.waitForTimeout(5000);
  await browser.close();
})();
"
```

## Common Patterns

### Form Submission

```bash
node <<'SCRIPT'
const { firefox } = require('playwright');
(async () => {
  const browser = await firefox.launch();
  const page = await browser.newPage();
  await page.goto('https://example.com/signup');
  await page.waitForLoadState('networkidle');

  // Fill form fields
  await page.fill('input[name="name"]', 'Jane Doe');
  await page.fill('input[name="email"]', 'jane@example.com');
  await page.selectOption('select[name="state"]', 'California');
  await page.check('input[type="checkbox"]');

  // Submit
  await page.click('button[type="submit"]');
  await page.waitForLoadState('networkidle');

  // Verify result
  console.log('URL:', page.url());
  console.log('Title:', await page.title());
  await page.screenshot({ path: '/tmp/form-result.png' });

  await browser.close();
})();
SCRIPT
```

### Data Extraction

```bash
node <<'SCRIPT'
const { firefox } = require('playwright');
(async () => {
  const browser = await firefox.launch();
  const page = await browser.newPage();
  await page.goto('https://example.com/products');
  await page.waitForLoadState('networkidle');

  // Get all text
  const text = await page.textContent('body');
  console.log(text);

  // Get specific elements
  const items = await page.$$eval('.product', els =>
    els.map(el => ({
      name: el.querySelector('h2')?.textContent,
      price: el.querySelector('.price')?.textContent,
    }))
  );
  console.log(JSON.stringify(items, null, 2));

  await browser.close();
})();
SCRIPT
```

### Authentication with State Persistence

```bash
# Step 1: Login and save state
node <<'SCRIPT'
const { firefox } = require('playwright');
(async () => {
  const browser = await firefox.launch();
  const context = await browser.newContext();
  const page = await context.newPage();

  await page.goto('https://app.example.com/login');
  await page.waitForLoadState('networkidle');

  await page.fill('input[name="email"]', process.env.APP_USERNAME);
  await page.fill('input[name="password"]', process.env.APP_PASSWORD);
  await page.click('button[type="submit"]');
  await page.waitForURL('**/dashboard');

  // Save auth state
  await context.storageState({ path: './auth.json' });
  console.log('Auth state saved to ./auth.json');

  await browser.close();
})();
SCRIPT

# Step 2: Reuse auth state in future runs
node -e "
const { firefox } = require('playwright');
(async () => {
  const browser = await firefox.launch();
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
(async () => {
  const browser = await firefox.launch();
  const page = await browser.newPage();
  await page.goto('https://example.com');
  await page.waitForLoadState('networkidle');

  // Viewport screenshot
  await page.screenshot({ path: 'viewport.png' });

  // Full page
  await page.screenshot({ path: 'full.png', fullPage: true });

  // Specific element
  const el = page.locator('header');
  await el.screenshot({ path: 'header.png' });

  // PDF
  await page.pdf({ path: 'page.pdf', format: 'A4' });

  await browser.close();
})();
SCRIPT
```

### Waiting Strategies

```bash
node <<'SCRIPT'
const { firefox } = require('playwright');
(async () => {
  const browser = await firefox.launch();
  const page = await browser.newPage();
  await page.goto('https://example.com');

  // Wait for network idle
  await page.waitForLoadState('networkidle');

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

  // Fixed wait (last resort)
  await page.waitForTimeout(2000);

  await browser.close();
})();
SCRIPT
```

### Viewport and Device Emulation

```bash
node <<'SCRIPT'
const { firefox, devices } = require('playwright');
(async () => {
  const browser = await firefox.launch();

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
  const browser = await firefox.launch();
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
  await page.waitForLoadState('networkidle');

  await browser.close();
})();
SCRIPT
```

### Multiple Tabs

```bash
node <<'SCRIPT'
const { firefox } = require('playwright');
(async () => {
  const browser = await firefox.launch();
  const context = await browser.newContext();

  const page1 = await context.newPage();
  await page1.goto('https://example.com');

  const page2 = await context.newPage();
  await page2.goto('https://example.org');

  console.log('Tab 1:', await page1.title());
  console.log('Tab 2:', await page2.title());

  await browser.close();
})();
SCRIPT
```

### Dark Mode

```bash
# CLI
npx playwright screenshot --browser firefox --color-scheme dark https://example.com dark.png

# Script
node -e "
const { firefox } = require('playwright');
(async () => {
  const browser = await firefox.launch();
  const context = await browser.newContext({ colorScheme: 'dark' });
  const page = await context.newPage();
  await page.goto('https://example.com');
  await page.screenshot({ path: 'dark.png' });
  await browser.close();
})();
"
```

### Proxy

```bash
node -e "
const { firefox } = require('playwright');
(async () => {
  const browser = await firefox.launch({
    proxy: { server: 'http://localhost:8080' }
  });
  const page = await browser.newPage();
  await page.goto('https://example.com');
  console.log(await page.title());
  await browser.close();
})();
"
```

### Dialogs (alert / confirm / prompt)

```bash
node <<'SCRIPT'
const { firefox } = require('playwright');
(async () => {
  const browser = await firefox.launch();
  const page = await browser.newPage();

  // Handle dialogs before they appear
  page.on('dialog', async dialog => {
    console.log(`Dialog: ${dialog.type()} - ${dialog.message()}`);
    await dialog.accept();  // or dialog.dismiss()
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
(async () => {
  const browser = await firefox.launch();
  const context = await browser.newContext({
    recordHar: { path: 'network.har' }
  });
  const page = await context.newPage();
  await page.goto('https://example.com');
  await page.waitForLoadState('networkidle');

  // Close context to flush HAR
  await context.close();
  console.log('HAR saved to network.har');

  await browser.close();
})();
SCRIPT
```

### Video Recording

```bash
node <<'SCRIPT'
const { firefox } = require('playwright');
(async () => {
  const browser = await firefox.launch();
  const context = await browser.newContext({
    recordVideo: { dir: './videos/', size: { width: 1280, height: 720 } }
  });
  const page = await context.newPage();
  await page.goto('https://example.com');
  await page.waitForTimeout(3000);

  // Close context to save video
  await context.close();
  console.log('Video saved to ./videos/');

  await browser.close();
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
