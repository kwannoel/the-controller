#!/bin/bash
# Template: Attached Tab Capture Workflow
# Purpose: Extract content from the current attached browser tab
# Usage: ./capture-workflow.sh [output-dir]
#
# Assumes the user already attached relay control on the intended tab.
# This template does not close the browser.

set -euo pipefail

OUTPUT_DIR="${1:-.}"

mkdir -p "$OUTPUT_DIR"

echo "Capturing attached tab..."

TITLE="$(agent-browser get title)"
URL="$(agent-browser get url)"

echo "Title: $TITLE"
echo "URL: $URL"

agent-browser screenshot --full "$OUTPUT_DIR/page-full.png"
echo "Saved: $OUTPUT_DIR/page-full.png"

agent-browser snapshot -i > "$OUTPUT_DIR/page-structure.txt"
echo "Saved: $OUTPUT_DIR/page-structure.txt"

echo "$TITLE" > "$OUTPUT_DIR/page-title.txt"
echo "$URL" > "$OUTPUT_DIR/page-url.txt"

echo ""
echo "Capture complete:"
ls -la "$OUTPUT_DIR"
