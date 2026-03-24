#!/bin/bash
# Template: Attached Tab Form Automation
# Purpose: Inspect and submit a form in the current attached browser tab
# Usage: ./form-automation.sh
#
# Assumes:
#   - The target form is already open in the attached Chrome/Chromium tab
#   - The user attached relay control to the correct tab
#
# This template does not close the browser.

set -euo pipefail

echo "Inspecting attached form tab..."
echo "URL: $(agent-browser get url)"
echo "Title: $(agent-browser get title)"

echo ""
echo "Form structure:"
agent-browser snapshot -i

cat <<'EOF'

Update the refs below based on the snapshot output, then uncomment:

  agent-browser fill @e1 "Test User"
  agent-browser fill @e2 "test@example.com"
  agent-browser click @e3
  agent-browser wait --load networkidle
  agent-browser snapshot -i

EOF

# Example flow:
# agent-browser fill @e1 "Test User"
# agent-browser fill @e2 "test@example.com"
# agent-browser click @e3
# agent-browser wait --load networkidle

echo "Screenshotting current state..."
agent-browser screenshot /tmp/form-result.png
echo "Saved: /tmp/form-result.png"
