# Actionable Maintainer Reports

## Goal

Transform the maintainer from a report-generating agent into an issue-filing agent. Each run analyzes the project, searches for existing `filed-by-maintainer` GitHub issues, and either files new issues or updates existing ones. Runs only produce output when something changed.

## Flow

```
Scheduler triggers run
  -> Spawn Claude session with health check + gh CLI access
    -> Claude analyzes codebase
    -> Claude searches existing filed-by-maintainer issues via gh issue list
    -> For each finding:
       - If matching issue exists: update body + add comment noting what changed
       - If no match: file new issue with labels
    -> Claude returns structured JSON summary of actions taken
  -> Rust parses summary into a run log
  -> Run log stored (timestamp, issues filed, issues updated, issues unchanged)
  -> If nothing changed since last run: no log emitted (diff-based silence)
```

## Labels (auto-created on first run)

| Label | Purpose |
|-------|---------|
| `filed-by-maintainer` | Identifies maintainer-filed issues |
| `priority: low` | Low priority finding |
| `priority: high` | High priority finding |
| `complexity: low` | Easy fix |
| `complexity: high` | Significant effort |

## GitHub Repo Resolution

1. Read `origin` remote URL from the project's local git config
2. If `MaintainerConfig.github_repo` is set, use that instead
3. If neither resolves, skip issue filing and log a warning

## Data Model Changes

**MaintainerConfig** gains:
- `github_repo: Option<String>` — optional `owner/repo` override

**MaintainerReport** becomes **MaintainerRunLog**:
- `id`, `project_id`, `timestamp` (same as before)
- `issues_filed: Vec<IssueSummary>` — new issues created
- `issues_updated: Vec<IssueSummary>` — existing issues updated with new context
- `issues_unchanged: u32` — count of issues that still exist with no change
- `summary: String` — one-line human-readable summary

**IssueSummary**:
- `issue_number: u32`
- `title: String`
- `url: String`
- `labels: Vec<String>`
- `action: "filed" | "updated"`

**Removed types**: `MaintainerFinding`, `FindingSeverity`, `FindingAction`, `ReportStatus` — replaced by GitHub issues as the source of truth.

## Claude Prompt Structure

The health check prompt instructs Claude to:
1. Analyze the repo for code quality, tests, architecture, docs
2. Use `gh issue list --label filed-by-maintainer --state open --json number,title,body,labels` to fetch existing issues
3. For each finding, determine if it matches an existing issue (semantic match, not just string equality)
4. File new or update existing issues using `gh issue create` / `gh issue edit` + `gh issue comment`
5. Auto-create missing labels via `gh label create` on first encounter
6. Return a JSON summary of all actions taken

## Dashboard Changes

- Run log list replaces report list — shows timestamp + summary ("Filed 2 new issues, updated 1")
- Clicking a run log shows the list of IssueSummary items with links to GitHub issues
- No more finding detail view — the detail lives in GitHub now

## Migration

On first run with the new code, delete all old-format `MaintainerReport` JSON files from `{projects_root}/{project_id}/maintainer-reports/`. These files use the old finding-based schema and are no longer relevant. The `clear_maintainer_reports()` storage method already exists and can be called once per project during migration.

## What Gets Removed

- `MaintainerFinding`, `FindingSeverity`, `FindingAction`, `ReportStatus` types
- Report detail view with finding blocks in AgentDashboard
- `parse_report_output()` logic for parsing findings (replaced by simpler action-summary parsing)
- Old `MaintainerReport` JSON files on disk (via migration)
