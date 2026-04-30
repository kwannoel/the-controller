import type { AgentProfile, RouteToken, RouteTokenKind } from "../daemon/types";

export interface RouteTokenQuery {
  kind: RouteTokenKind;
  query: string;
  start: number;
  end: number;
}

export interface InsertedRouteToken {
  text: string;
  cursor: number;
  token: RouteToken;
}

const ROUTE_TOKEN_PATTERN = /(^|\s)([@%])([A-Za-z0-9_-]*)$/;
const ROUTE_HANDLE_CHAR = /[A-Za-z0-9_-]/;

export function extractRouteTokenQuery(text: string, cursor: number): RouteTokenQuery | null {
  const prefix = text.slice(0, cursor);
  const match = prefix.match(ROUTE_TOKEN_PATTERN);
  if (!match) return null;

  const marker = match[2];
  const query = match[3] ?? "";
  const start = cursor - marker.length - query.length;
  return {
    kind: marker === "@" ? "reusable" : "shadow",
    query,
    start,
    end: cursor,
  };
}

export function routeTokenForProfile(
  profile: Pick<AgentProfile, "handle">,
  kind: RouteTokenKind,
  start: number,
  end: number,
): RouteToken {
  return { kind, handle: profile.handle, start, end };
}

export function insertRouteToken(
  text: string,
  query: RouteTokenQuery,
  handle: string,
): InsertedRouteToken {
  const marker = query.kind === "reusable" ? "@" : "%";
  const inserted = `${marker}${handle}`;
  const nextText = `${text.slice(0, query.start)}${inserted}${text.slice(query.end)}`;
  const end = query.start + inserted.length;
  return {
    text: nextText,
    cursor: end,
    token: { kind: query.kind, handle, start: query.start, end },
  };
}

export function reconcileRouteTokens(text: string, tokens: RouteToken[]): RouteToken[] {
  const usedStarts = new Set<number>();
  return tokens.flatMap((token) => {
    const marker = token.kind === "reusable" ? "@" : "%";
    const tokenText = `${marker}${token.handle}`;
    let start = findRouteToken(text, tokenText, Math.max(0, Math.min(token.start, text.length)));
    if (start === -1 && token.start > 0) {
      start = findRouteToken(text, tokenText, 0);
    }
    while (start !== -1 && usedStarts.has(start)) {
      start = findRouteToken(text, tokenText, start + tokenText.length);
    }
    if (start === -1) return [];
    usedStarts.add(start);
    return [{ ...token, start, end: start + tokenText.length }];
  });
}

function findRouteToken(text: string, tokenText: string, fromIndex: number): number {
  let start = text.indexOf(tokenText, fromIndex);
  while (start !== -1) {
    const before = start === 0 ? "" : text[start - 1];
    const after = text[start + tokenText.length] ?? "";
    if ((start === 0 || /\s/.test(before)) && !ROUTE_HANDLE_CHAR.test(after)) {
      return start;
    }
    start = text.indexOf(tokenText, start + tokenText.length);
  }
  return -1;
}
