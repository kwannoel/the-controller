#!/bin/bash
# Template: Firefox Authenticated Session Workflow
# Purpose: Login once, save state, reuse for subsequent runs using Firefox
# Usage: ./authenticated-session.sh <login-url> [state-file]
#
# Environment variables:
#   APP_USERNAME - Login username/email
#   APP_PASSWORD - Login password
#
# Two modes:
#   1. Discovery mode (default): Shows form structure so you can identify selectors
#   2. Login mode: Performs actual login after you update the selectors
#
# Setup steps:
#   1. Run once to see form structure (discovery mode)
#   2. Update selectors in LOGIN FLOW section below
#   3. Set APP_USERNAME and APP_PASSWORD
#   4. Delete the DISCOVERY section

set -euo pipefail

LOGIN_URL="${1:?Usage: $0 <login-url> [state-file]}"
STATE_FILE="${2:-./auth-ff.json}"

echo "Authentication workflow (Firefox): $LOGIN_URL"

# ================================================================
# SAVED STATE: Skip login if valid saved state exists
# ================================================================
if [[ -f "$STATE_FILE" ]]; then
    echo "Loading saved state from $STATE_FILE..."
    RESTORED=$(node <<SCRIPT
const { firefox } = require('playwright');
(async () => {
  try {
    const browser = await firefox.launch();
    const context = await browser.newContext({ storageState: '${STATE_FILE}' });
    const page = await context.newPage();
    await page.goto('${LOGIN_URL}');
    await page.waitForLoadState('networkidle');
    const url = page.url();
    const stillOnLogin = url.includes('login') || url.includes('signin');
    console.log(stillOnLogin ? 'expired' : 'restored');
    if (!stillOnLogin) {
      console.log('Title:', await page.title());
    }
    await browser.close();
  } catch (e) {
    console.log('expired');
  }
})();
SCRIPT
    )
    if echo "$RESTORED" | grep -q "^restored"; then
        echo "Session restored successfully"
        echo "$RESTORED"
        exit 0
    fi
    echo "Session expired, performing fresh login..."
    rm -f "$STATE_FILE"
fi

# ================================================================
# DISCOVERY MODE: Shows form structure (delete after setup)
# ================================================================
echo "Opening login page..."
node <<SCRIPT
const { firefox } = require('playwright');
(async () => {
  const browser = await firefox.launch();
  const page = await browser.newPage();
  await page.goto('${LOGIN_URL}');
  await page.waitForLoadState('networkidle');

  console.log('');
  console.log('Login form structure:');
  console.log('---');
  const elements = await page.\$\$eval('input, button, a[href], select', els =>
    els.map((el, i) => {
      const tag = el.tagName.toLowerCase();
      const type = el.type || '';
      const name = el.name || '';
      const placeholder = el.placeholder || '';
      const text = el.textContent?.trim().slice(0, 60) || '';
      const id = el.id || '';
      return ['[' + i + ']', tag, type, name ? 'name=' + name : '', id ? 'id=' + id : '', placeholder ? 'placeholder="' + placeholder + '"' : '', text ? '"' + text + '"' : ''].filter(Boolean).join(' ');
    })
  );
  elements.forEach(e => console.log(' ', e));
  console.log('---');
  console.log('');
  console.log('Next steps:');
  console.log('  1. Note the selectors for username, password, and submit');
  console.log('  2. Update the LOGIN FLOW section with your selectors');
  console.log('  3. Set: export APP_USERNAME="..." APP_PASSWORD="..."');
  console.log('  4. Delete the DISCOVERY MODE section');

  await browser.close();
})();
SCRIPT

exit 0

# ================================================================
# LOGIN FLOW: Uncomment and customize after discovery
# ================================================================
# : "${APP_USERNAME:?Set APP_USERNAME environment variable}"
# : "${APP_PASSWORD:?Set APP_PASSWORD environment variable}"
#
# node <<LOGINSCRIPT
# const { firefox } = require('playwright');
# (async () => {
#   const browser = await firefox.launch();
#   const context = await browser.newContext();
#   const page = await context.newPage();
#
#   await page.goto('${LOGIN_URL}');
#   await page.waitForLoadState('networkidle');
#
#   // Fill credentials (update selectors to match your form)
#   await page.fill('input[name="email"]', process.env.APP_USERNAME);
#   await page.fill('input[name="password"]', process.env.APP_PASSWORD);
#   await page.click('button[type="submit"]');
#   await page.waitForLoadState('networkidle');
#
#   // Verify login succeeded
#   const url = page.url();
#   if (url.includes('login') || url.includes('signin')) {
#     console.log('Login failed - still on login page');
#     await page.screenshot({ path: '/tmp/login-failed-ff.png' });
#     await browser.close();
#     process.exit(1);
#   }
#
#   // Save state for future runs
#   await context.storageState({ path: '${STATE_FILE}' });
#   console.log('Auth state saved to ${STATE_FILE}');
#   console.log('Login successful');
#   console.log('Title:', await page.title());
#
#   await browser.close();
# })();
# LOGINSCRIPT
