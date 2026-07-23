// Local markdown rendering (bundled via the `marked` npm package, so it
// works fully offline — no runtime fetch to a CDN).
import { marked } from "marked";

marked.setOptions({
  gfm: true,
  breaks: false,
});

export function renderMarkdown(source: string): string {
  return marked.parse(stripDocSiteSyntax(source), { async: false }) as string;
}

/** Strip syntax that plain markdown can't represent but doc-site generators
 * (Docusaurus/MDX and friends) mix into their source files: a YAML
 * frontmatter block and ESM `import` lines. Plain docs (the bundled seed,
 * Pi, local-folder imports) contain neither, so for them this is a no-op. */
function stripDocSiteSyntax(source: string): string {
  // Drop a leading BOM (some editors prepend one) so the frontmatter anchor
  // at ^ can match.
  const text = source.charCodeAt(0) === 0xfeff ? source.slice(1) : source;
  return text
    .replace(/^---\r?\n[\s\S]*?\r?\n---\r?\n?/, "")
    // MDX component import lines, e.g. `import Tabs from '@theme/Tabs';`
    .replace(/^[ \t]*import\s.+?from\s+['"][^'"]+['"];?[ \t]*\r?$/gm, "");
}
