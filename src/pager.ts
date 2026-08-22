// Renders the prev/next pager at the bottom of a rendered doc, ordered by
// the current nav tree. Pure DOM, no api imports, so it can be tested
// standalone.

export interface PagerLink {
  docId: string;
  title: string;
}

export function appendPager(
  container: HTMLElement,
  prev: PagerLink | null,
  next: PagerLink | null,
  onSelect: (docId: string) => void,
): void {
  container.querySelector(".doc-pager")?.remove();
  if (!prev && !next) return;

  const pager = document.createElement("nav");
  pager.className = "doc-pager";
  pager.setAttribute("aria-label", "文档翻页");

  if (prev) {
    const button = document.createElement("button");
    button.type = "button";
    button.textContent = `← 上一篇:${prev.title}`;
    button.addEventListener("click", () => onSelect(prev.docId));
    pager.appendChild(button);
  }
  if (next) {
    const button = document.createElement("button");
    button.type = "button";
    button.textContent = `下一篇:${next.title} →`;
    button.addEventListener("click", () => onSelect(next.docId));
    pager.appendChild(button);
  }
  container.appendChild(pager);
}
