#!/bin/bash
# Template: Attached Browser Session Check
# Purpose: Verify the agent is attached to the correct logged-in tab before automation
# Usage: ./authenticated-session.sh [expected-url-fragment]
#
# Assumes:
#   - The user already opened Chrome/Chromium
#   - The user already attached relay control on the intended tab
#
# This template does not close the browser.

set -euo pipefail

EXPECTED_URL_FRAGMENT="${1:-}"

echo "Checking attached browser session..."

URL="$(agent-browser get url)"
TITLE="$(agent-browser get title)"

echo "Current URL: $URL"
echo "Current title: $TITLE"

if [[ -n "$EXPECTED_URL_FRAGMENT" && "$URL" != *"$EXPECTED_URL_FRAGMENT"* ]]; then
    echo "Attached tab does not match expected URL fragment: $EXPECTED_URL_FRAGMENT"
    echo "Attach the correct tab, then run this script again."
    exit 1
fi

echo ""
echo "Interactive elements on the current page:"
agent-browser snapshot -i

echo ""
echo "Session looks ready."
echo "Continue with form fill, capture, or other automation on this attached tab."
