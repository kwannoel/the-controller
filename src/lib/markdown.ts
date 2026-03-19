/**
 * Simple markdown-to-HTML renderer with XSS protection.
 * Supports: headers, bold, italic, code blocks, inline code, links, unordered lists, empty lines.
 */

export function escapeHtml(text: string): string {
  return text.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;");
}

function escapeHtmlAttribute(text: string): string {
  return escapeHtml(text).replace(/"/g, "&quot;");
}

const ALLOWED_LINK_PROTOCOLS = new Set(["http:", "https:", "mailto:"]);

function sanitizeLinkUrl(url: string): string | null {
  const trimmedUrl = url.trim();
  if (trimmedUrl === "") {
    return null;
  }

  try {
    const parsedUrl = new URL(trimmedUrl);
    if (!ALLOWED_LINK_PROTOCOLS.has(parsedUrl.protocol)) {
      return null;
    }

    return trimmedUrl;
  } catch {
    return null;
  }
}

function renderLinks(text: string): string {
  const linkPattern = /\[([^\]]+)\]\(([^)]+)\)/g;
  let result = "";
  let lastIndex = 0;

  for (const match of text.matchAll(linkPattern)) {
    const matchIndex = match.index ?? 0;
    const [fullMatch, linkText, linkUrl] = match;

    result += escapeHtml(text.slice(lastIndex, matchIndex));

    const sanitizedUrl = sanitizeLinkUrl(linkUrl);
    const escapedLinkText = escapeHtml(linkText);
    if (sanitizedUrl) {
      result += `<a href="${escapeHtmlAttribute(sanitizedUrl)}" target="_blank" rel="noopener">${escapedLinkText}</a>`;
    } else {
      result += escapedLinkText;
    }

    lastIndex = matchIndex + fullMatch.length;
  }

  result += escapeHtml(text.slice(lastIndex));
  return result;
}

function processInline(line: string): string {
  // Inline code first (to prevent nested processing inside code spans)
  let result = "";
  let i = 0;
  while (i < line.length) {
    const tick = line.indexOf("`", i);
    if (tick === -1) {
      result += processInlineFormatting(line.slice(i));
      break;
    }
    const endTick = line.indexOf("`", tick + 1);
    if (endTick === -1) {
      result += processInlineFormatting(line.slice(i));
      break;
    }
    result += processInlineFormatting(line.slice(i, tick));
    result += `<code>${escapeHtml(line.slice(tick + 1, endTick))}</code>`;
    i = endTick + 1;
  }
  return result;
}

function applyBoldItalic(text: string): string {
  let result = text;
  // Bold: **text**
  result = result.replace(/\*\*(.+?)\*\*/g, "<strong>$1</strong>");
  // Italic: *text* (but not inside bold markers)
  result = result.replace(/\*(.+?)\*/g, "<em>$1</em>");
  return result;
}

function processInlineFormatting(text: string): string {
  const html = renderLinks(text);
  // Split by HTML tags so bold/italic regexes only operate on text segments,
  // never matching across tag boundaries (e.g. <a>...</a>).
  const parts = html.split(/(<[^>]+>)/);
  return parts.map((part) => (part.startsWith("<") ? part : applyBoldItalic(part))).join("");
}

export function renderMarkdown(md: string): string {
  const lines = md.split("\n");
  const output: string[] = [];
  let inCodeBlock = false;
  let inList = false;

  for (const line of lines) {
    // Code block toggle
    if (line.trimStart().startsWith("```")) {
      if (inCodeBlock) {
        output.push("</code></pre>");
        inCodeBlock = false;
      } else {
        if (inList) {
          output.push("</ul>");
          inList = false;
        }
        output.push("<pre><code>");
        inCodeBlock = true;
      }
      continue;
    }

    // Inside code block: escape and emit raw
    if (inCodeBlock) {
      output.push(escapeHtml(line));
      continue;
    }

    // Empty line
    if (line.trim() === "") {
      if (inList) {
        output.push("</ul>");
        inList = false;
      }
      output.push("<br>");
      continue;
    }

    // Headers
    const headerMatch = line.match(/^(#{1,6})\s+(.*)$/);
    if (headerMatch) {
      if (inList) {
        output.push("</ul>");
        inList = false;
      }
      const level = headerMatch[1].length;
      output.push(`<h${level}>${processInline(headerMatch[2])}</h${level}>`);
      continue;
    }

    // Unordered list items
    const listMatch = line.match(/^[-*]\s+(.*)$/);
    if (listMatch) {
      if (!inList) {
        output.push("<ul>");
        inList = true;
      }
      output.push(`<li>${processInline(listMatch[1])}</li>`);
      continue;
    }

    // Close list if we hit a non-list line
    if (inList) {
      output.push("</ul>");
      inList = false;
    }

    // Regular paragraph line
    output.push(`<p>${processInline(line)}</p>`);
  }

  // Close any open tags
  if (inList) {
    output.push("</ul>");
  }
  if (inCodeBlock) {
    output.push("</code></pre>");
  }

  return output.join("\n");
}
