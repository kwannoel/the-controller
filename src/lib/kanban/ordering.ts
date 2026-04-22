import type { GithubIssue } from "../stores";
import type { Column } from "./columns";

export type OrderMap = Record<string, number[]>;

export function orderKey(repoPath: string, column: Column): string {
  return `${repoPath}:${column}`;
}

export function applyOrdering(
  issues: GithubIssue[],
  column: Column,
  repoPath: string,
  orderMap: OrderMap,
): GithubIssue[] {
  const key = orderKey(repoPath, column);
  const saved = orderMap[key];
  if (!saved || saved.length === 0) return [...issues];

  const byNumber = new Map(issues.map((i) => [i.number, i]));
  const ordered: GithubIssue[] = [];
  const placed = new Set<number>();

  for (const n of saved) {
    const issue = byNumber.get(n);
    if (issue && !placed.has(n)) {
      ordered.push(issue);
      placed.add(n);
    }
  }

  for (const issue of issues) {
    if (!placed.has(issue.number)) ordered.push(issue);
  }

  return ordered;
}

export function moveIssue(
  orderMap: OrderMap,
  fromKey: string,
  toKey: string,
  issueNumber: number,
  toIndex: number,
): OrderMap {
  const next: OrderMap = { ...orderMap };
  const fromList = [...(next[fromKey] ?? [])].filter(
    (n) => n !== issueNumber,
  );
  next[fromKey] = fromList;

  const toListBase =
    fromKey === toKey
      ? fromList
      : [...(next[toKey] ?? [])].filter((n) => n !== issueNumber);
  const clampedIndex = Math.max(0, Math.min(toIndex, toListBase.length));
  const toList = [
    ...toListBase.slice(0, clampedIndex),
    issueNumber,
    ...toListBase.slice(clampedIndex),
  ];
  next[toKey] = toList;
  if (fromKey === toKey) next[fromKey] = toList;

  return next;
}
