// Renders the main content pane: loading / error / rendered-markdown states.
import { assignHeadingIds } from "./doc-links";
import { renderMarkdown } from "./markdown";

export function showLoading(container: HTMLElement): void {
  container.innerHTML = `<p class="content-status">加载中…</p>`;
}

export function showPlaceholder(container: HTMLElement): void {
  container.innerHTML = `<p class="content-status">从左侧选择一篇文档开始阅读。</p>`;
}

export function showError(container: HTMLElement, message: string, prefix = "加载失败"): void {
  container.innerHTML = `<p class="content-status content-error">${prefix}：${escapeHtml(message)}</p>`;
}

export interface DocBanner {
  text: string;
  buttonLabel: string;
  onClick: () => void;
}

export function showDoc(container: HTMLElement, markdown: string, banners: DocBanner[] = []): void {
  container.replaceChildren();

  for (const banner of banners) {
    const el = document.createElement("div");
    el.className = "update-banner";
    const text = document.createElement("span");
    text.textContent = banner.text;
    const button = document.createElement("button");
    button.type = "button";
    button.textContent = banner.buttonLabel;
    button.addEventListener("click", banner.onClick);
    el.append(text, button);
    container.appendChild(el);
  }

  const article = document.createElement("article");
  article.className = "markdown-body";
  article.innerHTML = renderMarkdown(markdown);
  assignHeadingIds(article); // gives in-page #anchor links something to land on
  container.appendChild(article);
}

function escapeHtml(value: string): string {
  const div = document.createElement("div");
  div.textContent = value;
  return div.innerHTML;
}
