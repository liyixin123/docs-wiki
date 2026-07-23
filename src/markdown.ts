// Local markdown rendering (bundled via the `marked` npm package, so it
// works fully offline — no runtime fetch to a CDN).
import { marked } from "marked";

marked.setOptions({
  gfm: true,
  breaks: false,
});

export function renderMarkdown(source: string): string {
  return marked.parse(source, { async: false }) as string;
}
