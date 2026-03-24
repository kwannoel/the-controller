#!/bin/bash
# Template: Firefox Content Capture Workflow
# Purpose: Extract content from web pages (text, screenshots, PDF) using Firefox
# Usage: ./capture-workflow.sh <url> [output-dir]
#
# Outputs:
#   - page-full.png: Full page screenshot
#   - page-text.txt: All text content
#   - page.pdf: PDF version

set -euo pipefail

TARGET_URL="${1:?Usage: $0 <url> [output-dir]}"
OUTPUT_DIR="${2:-.}"

echo "Capturing (Firefox): $TARGET_URL"
mkdir -p "$OUTPUT_DIR"

node <<SCRIPT
const { firefox } = require('playwright');
(async () => {
  const browser = await firefox.launch();
  const page = await browser.newPage();
  await page.goto('${TARGET_URL}');
  await page.waitForLoadState('networkidle');

  const title = await page.title();
  console.log('Title:', title);
  console.log('URL:', page.url());

  // Full page screenshot
  await page.screenshot({ path: '${OUTPUT_DIR}/page-full.png', fullPage: true });
  console.log('Saved: ${OUTPUT_DIR}/page-full.png');

  // Extract text
  const fs = require('fs');
  const text = await page.textContent('body');
  fs.writeFileSync('${OUTPUT_DIR}/page-text.txt', text || '');
  console.log('Saved: ${OUTPUT_DIR}/page-text.txt');

  // PDF
  await page.pdf({ path: '${OUTPUT_DIR}/page.pdf', format: 'A4' });
  console.log('Saved: ${OUTPUT_DIR}/page.pdf');

  await browser.close();
})();
SCRIPT

echo ""
echo "Capture complete:"
ls -la "$OUTPUT_DIR"
