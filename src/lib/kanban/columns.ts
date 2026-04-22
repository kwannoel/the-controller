import type { GithubIssue } from "../stores";

export type Column =
  | "backlog"
  | "todo"
  | "in-progress"
  | "in-review"
  | "done";

export const COLUMNS: readonly Column[] = [
  "backlog",
  "todo",
  "in-progress",
  "in-review",
  "done",
] as const;

export const COLUMN_TITLES: Record<Column, string> = {
  backlog: "Backlog",
  todo: "To Do",
  "in-progress": "In Progress",
  "in-review": "In Review",
  done: "Done",
};

export const LABEL_BY_COLUMN: Record<Column, string> = {
  backlog: "status:backlog",
  todo: "status:todo",
  "in-progress": "status:in-progress",
  "in-review": "status:in-review",
  done: "status:done",
};

export const COLUMN_BY_LABEL: Record<string, Column> = Object.fromEntries(
  (Object.entries(LABEL_BY_COLUMN) as [Column, string][]).map(([c, l]) => [
    l,
    c,
  ]),
);

export const EMPTY_STATE: Record<Column, string> = {
  backlog: "No backlog items",
  todo: "Nothing queued",
  "in-progress": "Nothing in progress",
  "in-review": "Nothing awaiting review",
  done: "Nothing done yet",
};

export const LABEL_COLOR: Record<Column, string> = {
  backlog: "9399B2",
  todo: "89B4FA",
  "in-progress": "F9E2AF",
  "in-review": "CBA6F7",
  done: "A6E3A1",
};

export function columnForIssue(issue: GithubIssue): Column {
  let best: Column = "backlog";
  let bestIdx = -1;
  for (const { name } of issue.labels) {
    const c = COLUMN_BY_LABEL[name];
    if (!c) continue;
    const idx = COLUMNS.indexOf(c);
    if (idx > bestIdx) {
      best = c;
      bestIdx = idx;
    }
  }
  return best;
}

export function isStatusLabel(name: string): boolean {
  return name.startsWith("status:");
}
