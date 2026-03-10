#!/usr/bin/env bash

set -euo pipefail

if [[ $# -lt 1 || $# -gt 2 ]]; then
  echo "Usage: $0 <owner/repo> [--dry-run]" >&2
  exit 1
fi

REPO="$1"
DRY_RUN="${2:-}"
ASSIGNED_LABEL="assigned-to-auto-worker"
FINISHED_LABEL="finished-by-worker"
REPORT_MARKER="<!-- auto-worker-report -->"

if [[ "$DRY_RUN" != "" && "$DRY_RUN" != "--dry-run" ]]; then
  echo "Unknown option: $DRY_RUN" >&2
  exit 1
fi

gh label create "$ASSIGNED_LABEL" \
  --repo "$REPO" \
  --description "Issue has been handled by the auto-worker" \
  --color "94E2D5" \
  --force >/dev/null

ISSUES_JSON="$(gh issue list \
  --repo "$REPO" \
  --state all \
  --limit 200 \
  --json number,title,labels,comments)"

ISSUE_ROWS="$(
  printf '%s' "$ISSUES_JSON" | node -e '
const fs = require("fs");
const marker = process.argv[1];
const assigned = process.argv[2];
const finished = process.argv[3];
const issues = JSON.parse(fs.readFileSync(0, "utf8"));
for (const issue of issues) {
  const hasReport = (issue.comments || []).some((comment) =>
    typeof comment.body === "string" && comment.body.includes(marker)
  );
  if (!hasReport) continue;
  const labels = new Set((issue.labels || []).map((label) => label.name));
  const needsAssigned = !labels.has(assigned);
  const hasFinished = labels.has(finished);
  console.log([issue.number, issue.title, needsAssigned ? "1" : "0", hasFinished ? "1" : "0"].join("\t"));
}
' "$REPORT_MARKER" "$ASSIGNED_LABEL" "$FINISHED_LABEL"
)"

if [[ -z "$ISSUE_ROWS" ]]; then
  echo "No worker-report issues found in $REPO"
  exit 0
fi

while IFS= read -r row; do
  IFS=$'\t' read -r issue_number issue_title needs_assigned has_finished <<<"$row"
  echo "#$issue_number $issue_title"

  if [[ "$needs_assigned" == "0" && "$has_finished" == "0" ]]; then
    echo "  no changes"
    continue
  fi

  if [[ "$DRY_RUN" == "--dry-run" ]]; then
    [[ "$needs_assigned" == "1" ]] && echo "  would add $ASSIGNED_LABEL"
    [[ "$has_finished" == "1" ]] && echo "  would remove $FINISHED_LABEL"
    continue
  fi

  if [[ "$needs_assigned" == "1" ]]; then
    gh issue edit "$issue_number" --repo "$REPO" --add-label "$ASSIGNED_LABEL" >/dev/null
    echo "  added $ASSIGNED_LABEL"
  fi

  if [[ "$has_finished" == "1" ]]; then
    gh issue edit "$issue_number" --repo "$REPO" --remove-label "$FINISHED_LABEL" >/dev/null
    echo "  removed $FINISHED_LABEL"
  fi
done <<<"$ISSUE_ROWS"
