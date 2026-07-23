// Renders the wiki-style cross-source search results page: grouped by hit,
// each with a source/language badge and highlighted body snippets.
import type { SearchHit, Snippet } from "./api";
import { languageLabel } from "./lang";

export function renderSearchResults(
  container: HTMLElement,
  query: string,
  hits: SearchHit[],
  sourceNames: Map<string, string>,
  onSelect: (hit: SearchHit) => void,
): void {
  container.replaceChildren();

  const summary = document.createElement("p");
  summary.className = "search-summary";
  summary.textContent =
    hits.length > 0
      ? `找到 ${hits.length} 条与「${query}」相关的结果`
      : `没有找到与「${query}」相关的结果`;
  container.appendChild(summary);

  const list = document.createElement("div");
  list.className = "search-hit-list";

  for (const hit of hits) {
    const card = document.createElement("button");
    card.type = "button";
    card.className = "search-hit";
    card.addEventListener("click", () => onSelect(hit));

    const header = document.createElement("div");
    header.className = "search-hit-header";

    const title = document.createElement("span");
    title.className = "search-hit-title";
    title.textContent = hit.title;

    const meta = document.createElement("span");
    meta.className = "search-hit-meta";
    meta.textContent = `${sourceNames.get(hit.sourceId) ?? hit.sourceId} · ${languageLabel(hit.lang)}`;

    header.append(title, meta);
    card.appendChild(header);

    for (const snippet of hit.snippets) {
      const p = document.createElement("p");
      p.className = "search-hit-snippet";
      p.innerHTML = highlightSnippet(snippet);
      card.appendChild(p);
    }

    list.appendChild(card);
  }
  container.appendChild(list);
}

function highlightSnippet(snippet: Snippet): string {
  let html = "";
  let cursor = 0;
  for (const [start, end] of snippet.highlightRanges) {
    html += escapeHtml(snippet.text.slice(cursor, start));
    html += `<mark>${escapeHtml(snippet.text.slice(start, end))}</mark>`;
    cursor = end;
  }
  html += escapeHtml(snippet.text.slice(cursor));
  return html;
}

function escapeHtml(value: string): string {
  const div = document.createElement("div");
  div.textContent = value;
  return div.innerHTML;
}
