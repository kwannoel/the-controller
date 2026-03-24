#!/bin/bash
# Template: Firefox Form Automation Workflow
# Purpose: Fill and submit web forms with validation using Firefox
# Usage: ./form-automation.sh <form-url>
#
# Customize: Update the selectors based on your form's structure.
# Use the browser inspector or `npx playwright codegen -b firefox <url>`
# to discover the right selectors.

set -euo pipefail

FORM_URL="${1:?Usage: $0 <form-url>}"

echo "Form automation (Firefox): $FORM_URL"

node <<SCRIPT
const { firefox } = require('playwright');
(async () => {
  const browser = await firefox.launch();
  const page = await browser.newPage();

  // Step 1: Navigate to form
  await page.goto('${FORM_URL}');
  await page.waitForLoadState('networkidle');
  console.log('Page loaded:', await page.title());

  // Step 2: Discover form elements (uncomment to inspect)
  // const inputs = await page.$$eval('input, select, textarea, button', els =>
  //   els.map(el => ({
  //     tag: el.tagName,
  //     type: el.type,
  //     name: el.name,
  //     placeholder: el.placeholder,
  //     text: el.textContent?.trim().slice(0, 50),
  //   }))
  // );
  // console.log(JSON.stringify(inputs, null, 2));

  // Step 3: Fill form fields (customize these selectors)
  //
  // await page.fill('input[name="name"]', 'Test User');
  // await page.fill('input[name="email"]', 'test@example.com');
  // await page.selectOption('select[name="country"]', 'US');
  // await page.check('input[type="checkbox"]');
  // await page.click('button[type="submit"]');
  // await page.waitForLoadState('networkidle');

  // Step 4: Verify result
  console.log('Final URL:', page.url());
  await page.screenshot({ path: '/tmp/form-result-ff.png' });
  console.log('Screenshot saved: /tmp/form-result-ff.png');

  await browser.close();
})();
SCRIPT

echo "Done"
