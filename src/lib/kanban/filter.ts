import type { GithubIssue } from "../stores";
import { isStatusLabel } from "./columns";

export interface Filters {
  assignees: Set<string>;
  labels: Set<string>;
  milestone: string | null;
}

export function applyFilters(
  issues: GithubIssue[],
  filters: Filters,
): GithubIssue[] {
  const wantLabels = new Set(
    [...filters.labels].filter((l) => !isStatusLabel(l)),
  );
  const wantAssignees = filters.assignees;
  const wantMilestone = filters.milestone;

  return issues.filter((issue) => {
    if (wantAssignees.size > 0) {
      const logins = (issue.assignees ?? []).map((a) => a.login);
      if (!logins.some((l) => wantAssignees.has(l))) return false;
    }

    if (wantLabels.size > 0) {
      const names = new Set(issue.labels.map((l) => l.name));
      for (const want of wantLabels) {
        if (!names.has(want)) return false;
      }
    }

    if (wantMilestone !== null) {
      if ((issue.milestone?.title ?? null) !== wantMilestone) return false;
    }

    return true;
  });
}
