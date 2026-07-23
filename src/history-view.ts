// Renders the "history" panel: a flat, most-recent-first list of log
// entries (imports, update checks, applied updates, translations).
import type { LogEntry } from "./api";

const KIND_LABELS: Record<LogEntry["kind"], string> = {
  import: "导入",
  check: "检查更新",
  applyUpdate: "应用更新",
  translate: "翻译",
  translateError: "翻译失败",
};

export function renderHistory(
  container: HTMLElement,
  entries: LogEntry[],
  onShowDiff?: (entry: LogEntry) => void,
): void {
  container.replaceChildren();

  const heading = document.createElement("p");
  heading.className = "search-summary";
  heading.textContent = entries.length > 0 ? `共 ${entries.length} 条历史记录` : "暂无历史记录";
  container.appendChild(heading);

  const list = document.createElement("div");
  list.className = "history-list";

  for (const entry of entries) {
    const row = document.createElement("div");
    row.className = "history-row";
    if (entry.snapshot && onShowDiff) {
      row.classList.add("clickable");
      row.title = "点击查看变更对比";
      row.tabIndex = 0;
      row.addEventListener("click", () => onShowDiff(entry));
      row.addEventListener("keydown", (event) => {
        if (event.key === "Enter" || event.key === " ") {
          event.preventDefault();
          onShowDiff(entry);
        }
      });
    }

    const kind = document.createElement("span");
    kind.className = `history-kind history-kind-${entry.kind}`;
    kind.textContent = KIND_LABELS[entry.kind] ?? entry.kind;

    const detail = document.createElement("span");
    detail.className = "history-detail";
    detail.textContent = entry.detail;

    const time = document.createElement("span");
    time.className = "history-time";
    time.textContent = formatTimestamp(entry.ts);

    row.append(kind, detail, time);
    list.appendChild(row);
  }
  container.appendChild(list);
}

function formatTimestamp(iso: string): string {
  const date = new Date(iso);
  if (Number.isNaN(date.getTime())) return iso;
  return date.toLocaleString();
}
