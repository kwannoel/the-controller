import { describe, it, expect } from "vitest";
import { renderMarkdown, escapeHtml } from "./markdown";

describe("escapeHtml", () => {
  it("escapes &, <, >", () => {
    expect(escapeHtml("a & b < c > d")).toBe("a &amp; b &lt; c &gt; d");
  });

  it("returns empty string for empty input", () => {
    expect(escapeHtml("")).toBe("");
  });
});

describe("renderMarkdown", () => {
  it("renders headers h1 through h6", () => {
    expect(renderMarkdown("# Hello")).toContain("<h1>Hello</h1>");
    expect(renderMarkdown("## Hello")).toContain("<h2>Hello</h2>");
    expect(renderMarkdown("### Hello")).toContain("<h3>Hello</h3>");
    expect(renderMarkdown("#### Hello")).toContain("<h4>Hello</h4>");
    expect(renderMarkdown("##### Hello")).toContain("<h5>Hello</h5>");
    expect(renderMarkdown("###### Hello")).toContain("<h6>Hello</h6>");
  });

  it("renders bold text", () => {
    expect(renderMarkdown("**bold**")).toContain("<strong>bold</strong>");
  });

  it("renders italic text", () => {
    expect(renderMarkdown("*italic*")).toContain("<em>italic</em>");
  });

  it("renders inline code", () => {
    expect(renderMarkdown("use `foo()` here")).toContain("<code>foo()</code>");
  });

  it("escapes HTML inside inline code", () => {
    const result = renderMarkdown("use `<script>` tag");
    expect(result).toContain("<code>&lt;script&gt;</code>");
    expect(result).not.toContain("<script>");
  });

  it("renders code blocks", () => {
    const md = "```\nconst x = 1;\n```";
    const result = renderMarkdown(md);
    expect(result).toContain("<pre><code>");
    expect(result).toContain("</code></pre>");
    expect(result).toContain("const x = 1;");
  });

  it("escapes HTML inside code blocks", () => {
    const md = "```\n<div>hello</div>\n```";
    const result = renderMarkdown(md);
    expect(result).toContain("&lt;div&gt;hello&lt;/div&gt;");
    expect(result).not.toContain("<div>");
  });

  it("renders links", () => {
    const result = renderMarkdown("[click here](https://example.com)");
    expect(result).toContain('<a href="https://example.com" target="_blank" rel="noopener">click here</a>');
  });

  it("renders mailto links", () => {
    const result = renderMarkdown("[email me](mailto:test@example.com)");
    expect(result).toContain('<a href="mailto:test@example.com" target="_blank" rel="noopener">email me</a>');
  });

  it("does not render javascript links", () => {
    const result = renderMarkdown("[click me](javascript:alert-xss)");
    expect(result).not.toContain("<a ");
    expect(result).toContain("<p>click me</p>");
    expect(result).not.toContain("javascript:");
  });

  it("does not render data links", () => {
    const result = renderMarkdown("[click me](data:text/html;base64,PHNjcmlwdD4=)");
    expect(result).not.toContain("<a ");
    expect(result).toContain("<p>click me</p>");
    expect(result).not.toContain("data:text/html");
  });

  it("escapes quotes in link URLs", () => {
    const input = '[click](http://example.com" onclick="alert(1))';
    const result = renderMarkdown(input);
    expect(result).not.toContain('onclick');
  });

  it("renders unordered lists", () => {
    const md = "- item 1\n- item 2\n- item 3";
    const result = renderMarkdown(md);
    expect(result).toContain("<ul>");
    expect(result).toContain("<li>item 1</li>");
    expect(result).toContain("<li>item 2</li>");
    expect(result).toContain("<li>item 3</li>");
    expect(result).toContain("</ul>");
  });

  it("closes list when switching to non-list content", () => {
    const md = "- item\n\nParagraph";
    const result = renderMarkdown(md);
    expect(result).toContain("</ul>");
    expect(result).toContain("<p>Paragraph</p>");
  });

  it("renders empty lines as <br>", () => {
    const result = renderMarkdown("line 1\n\nline 2");
    expect(result).toContain("<br>");
  });

  it("escapes HTML in regular text to prevent XSS", () => {
    const result = renderMarkdown("<script>alert('xss')</script>");
    expect(result).not.toContain("<script>");
    expect(result).toContain("&lt;script&gt;");
  });

  it("escapes HTML in header content", () => {
    const result = renderMarkdown("# <b>header</b>");
    expect(result).toContain("&lt;b&gt;header&lt;/b&gt;");
  });

  it("handles empty input", () => {
    const result = renderMarkdown("");
    expect(result).toBe("<br>");
  });

  it("does not apply inline formatting across link tag boundaries", () => {
    // A * in one link and a * in another — the old regex would match across <a> tags
    const md = "see [foo*](https://a.com) and [bar*](https://b.com) here";
    const result = renderMarkdown(md);
    // Both links should remain intact
    expect(result).toContain('<a href="https://a.com"');
    expect(result).toContain('<a href="https://b.com"');
    // No <em> should span across the two links
    expect(result).not.toContain("<em>");
  });

  it("handles mixed content", () => {
    const md = "# Title\n\nSome **bold** and *italic* text.\n\n- list item\n\n```\ncode\n```";
    const result = renderMarkdown(md);
    expect(result).toContain("<h1>Title</h1>");
    expect(result).toContain("<strong>bold</strong>");
    expect(result).toContain("<em>italic</em>");
    expect(result).toContain("<li>list item</li>");
    expect(result).toContain("<pre><code>");
  });
});
