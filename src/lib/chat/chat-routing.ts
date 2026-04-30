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
    let start = text.slice(token.start, token.end) === tokenText ? token.start : -1;
    if (start === -1) {
      start = text.indexOf(tokenText, Math.max(0, Math.min(token.start, text.length)));
    }
    if (start === -1) {
      start = text.indexOf(tokenText);
    }
    while (start !== -1 && usedStarts.has(start)) {
      start = text.indexOf(tokenText, start + tokenText.length);
    }
    if (start === -1) return [];
    usedStarts.add(start);
    return [{ ...token, start, end: start + tokenText.length }];
  });
}
