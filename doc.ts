/**
 * Converts HTML to Markdown using pure Regex.
 * Zero dependencies. Works in Node, Deno, and Browser.
 */

export function convertHtmlToMarkdown(html: string): string {
    let md = html;

    // --- 1. Cleanup & Metadata ---
    // Remove DOCTYPE, html, body tags (keep content)
    md = md.replace(/<!DOCTYPE[^>]*>/gi, "");
    md = md.replace(/<\/?html[^>]*>/gi, "");
    md = md.replace(/<\/?body[^>]*>/gi, "");

    // Remove invisible sections completely (Head, Script, Style)
    md = md.replace(/<head[\s\S]*?<\/head>/gi, "");
    md = md.replace(/<script[\s\S]*?<\/script>/gi, "");
    md = md.replace(/<style[\s\S]*?<\/style>/gi, "");
    md = md.replace(/<!--[\s\S]*?-->/g, ""); // Comments

    // --- 2. Block Elements ---

    // Headers (h1-h6)
    // We use a replacer function to determine the number of hashes
    md = md.replace(
        /<h([1-6])[^>]*>([\s\S]*?)<\/h\1>/gi,
        (match, level, content) => {
            const hashes = "#".repeat(parseInt(level));
            return `\n\n${hashes} ${removeTags(content).trim()}\n\n`;
        },
    );

    // Code Blocks (<pre><code>)
    // Try to catch class="language-xyz"
    md = md.replace(
        /<pre><code[^>]*class=["'](?:language-)?(\w+)["'][^>]*>([\s\S]*?)<\/code><\/pre>/gi,
        "\n```$1\n$2\n```\n",
    );
    // Catch generic code blocks
    md = md.replace(
        /<pre><code[^>]*>([\s\S]*?)<\/code><\/pre>/gi,
        "\n```\n$1\n```\n",
    );
    // Catch bare pre
    md = md.replace(/<pre[^>]*>([\s\S]*?)<\/pre>/gi, "\n```\n$1\n```\n");

    // Paragraphs
    md = md.replace(/<p[^>]*>([\s\S]*?)<\/p>/gi, "\n$1\n\n");

    // Blockquotes
    md = md.replace(
        /<blockquote[^>]*>([\s\S]*?)<\/blockquote>/gi,
        "\n> $1\n\n",
    );

    // Horizontal Rules
    md = md.replace(/<hr\s*\/?>/gi, "\n---\n");

    // Line Breaks
    md = md.replace(/<br\s*\/?>/gi, "  \n");

    // --- 3. Lists ---
    // Definition Lists (dl, dt, dd) - mapped to bullet points
    md = md.replace(/<\/?dl[^>]*>/gi, ""); // Remove outer dl wrapper
    md = md.replace(/<dt[^>]*>([\s\S]*?)<\/dt>/gi, "\n* **$1**");
    md = md.replace(/<dd[^>]*>([\s\S]*?)<\/dd>/gi, ": $1\n");

    // Unordered/Ordered Lists
    // Note: Regex cannot easily handle nested indentation, but this preserves the list structure linearly.
    md = md.replace(/<\/?ul[^>]*>/gi, "");
    md = md.replace(/<\/?ol[^>]*>/gi, "");
    md = md.replace(/<li[^>]*>([\s\S]*?)<\/li>/gi, "- $1\n");

    // --- 4. Inline Elements ---

    // Images
    md = md.replace(
        /<img[^>]*src=["']([^"']*)["'][^>]*alt=["']([^"']*)["'][^>]*\/?>/gi,
        "![$2]($1)",
    );
    // Fallback for images without alt or different order (simple catch)
    md = md.replace(/<img[^>]*src=["']([^"']*)["'][^>]*\/?>/gi, "![]($1)");

    // Links
    md = md.replace(
        /<a[^>]*href=["']([^"']*)["'][^>]*>([\s\S]*?)<\/a>/gi,
        "[$2]($1)",
    );

    // Bold / Strong
    md = md.replace(/<(b|strong)[^>]*>([\s\S]*?)<\/\1>/gi, "**$2**");

    // Italic / Em
    md = md.replace(/<(i|em)[^>]*>([\s\S]*?)<\/\1>/gi, "*$2*");

    // Inline Code
    md = md.replace(/<code[^>]*>([\s\S]*?)<\/code>/gi, "`$1`");

    // --- 5. Final Cleanup ---

    // Decode common entities
    md = md.replace(/&nbsp;/g, " ");
    md = md.replace(/&amp;/g, "&");
    md = md.replace(/&lt;/g, "<");
    md = md.replace(/&gt;/g, ">");
    md = md.replace(/&quot;/g, '"');
    md = md.replace(/&#39;/g, "'");

    // Strip any remaining HTML tags (like div, span, main, section) that we didn't transform
    md = removeTags(md);

    // Collapse multiple newlines into max 2
    md = md.replace(/\n{3,}/g, "\n\n");

    return md.trim();
}

// Helper to remove remaining tags but keep content
function removeTags(str: string): string {
    return str.replace(/<\/?[^>]+(>|$)/g, "");
}

const md = convertHtmlToMarkdown(await Deno.readTextFile("doc.html"));
await Deno.writeTextFile("doc.md", md);
