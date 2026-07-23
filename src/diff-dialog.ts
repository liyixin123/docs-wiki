// The "what changed" diff dialog for a history entry: fetches the
// snapshot-vs-current diff from the backend and renders it either inline
// (red/green rows, GitHub-style) or side-by-side, switchable via a toggle.
// Mirrors the native-<dialog> wiring pattern in remote-source-dialog.ts.
import { getDiff, type DiffLine, type LogEntry } from "./api";

type DiffView = "inline" | "side";

let dialog: HTMLDialogElement;
let titleEl: HTMLElement;
let metaEl: HTMLElement;
let bodyEl: HTMLElement;
let inlineBtn: HTMLButtonElement;
let sideBtn: HTMLButtonElement;

let currentView: DiffView = "inline";
// Cache the last fetched lines so flipping the view re-renders locally
// without another round-trip; cleared each time the dialog opens.
let cachedLines: DiffLine[] | null = null;

export function initDiffDialog(): void {
  dialog = document.querySelector<HTMLDialogElement>("#diff-dialog")!;
  titleEl = document.querySelector<HTMLElement>("#diff-title")!;
  metaEl = document.querySelector<HTMLElement>("#diff-meta")!;
  bodyEl = document.querySelector<HTMLElement>("#diff-body")!;
  inlineBtn = document.querySelector<HTMLButtonElement>("#diff-view-inline")!;
  sideBtn = document.querySelector<HTMLButtonElement>("#diff-view-side")!;

  document.querySelector<HTMLButtonElement>("#diff-close")!.addEventListener("click", () => dialog.close());
  inlineBtn.addEventListener("click", () => switchView("inline"));
  sideBtn.addEventListener("click", () => switchView("side"));
}

function switchView(view: DiffView): void {
  if (currentView === view) return;
  currentView = view;
  inlineBtn.classList.toggle("active", view === "inline");
  sideBtn.classList.toggle("active", view === "side");
  if (cachedLines) render(cachedLines);
}

export async function openDiffDialog(entry: LogEntry): Promise<void> {
  if (!entry.snapshot) return;
  cachedLines = null;
  // Reset the toggle to the default view each time, so the dialog doesn't
  // inherit a stale "side" selection from a previous open.
  currentView = "inline";
  inlineBtn.classList.add("active");
  sideBtn.classList.remove("active");

  titleEl.textContent = labelForKind(entry.kind);
  metaEl.textContent =
    `文档：${entry.docId ?? "?"} · 语言：${entry.snapshot.lang} · ${formatTimestamp(entry.ts)}`;

  showLoading();
  dialog.showModal();

  try {
    const payload = await getDiff(
      entry.sourceId,
      entry.snapshot.lang,
      entry.docId ?? "",
      entry.snapshot.file,
    );
    cachedLines = payload.lines;
    render(payload.lines);
  } catch (err) {
    showError(err);
  }
}

function render(lines: DiffLine[]): void {
  bodyEl.classList.remove("diff-error");
  bodyEl.replaceChildren();
  if (lines.length === 0) {
    const empty = document.createElement("p");
    empty.className = "diff-empty";
    empty.textContent = "新旧内容完全一致，没有差异。";
    bodyEl.appendChild(empty);
    return;
  }
  bodyEl.appendChild(currentView === "inline" ? renderInline(lines) : renderSideBySide(lines));
}

function renderInline(lines: DiffLine[]): HTMLTableElement {
  const table = document.createElement("table");
  table.className = "diff-inline";
  for (const line of lines) {
    const row = document.createElement("tr");
    row.className = `diff-line diff-line-${line.kind}`;
    row.append(
      linenoCell(line.oldNumber),
      linenoCell(line.newNumber),
      signCell(line.kind),
      textCell(line.text),
    );
    table.appendChild(row);
  }
  return table;
}

function renderSideBySide(lines: DiffLine[]): HTMLTableElement {
  const table = document.createElement("table");
  table.className = "diff-side";
  for (const line of lines) {
    const row = document.createElement("tr");
    row.className = `diff-line diff-line-${line.kind}`;
    const oldCell = document.createElement("td");
    oldCell.className = "diff-side-col";
    const newCell = document.createElement("td");
    newCell.className = "diff-side-col";
    if (line.kind === "ctx") {
      oldCell.textContent = line.text;
      newCell.textContent = line.text;
    } else if (line.kind === "del") {
      oldCell.textContent = line.text;
    } else {
      newCell.textContent = line.text;
    }
    row.append(oldCell, newCell);
    table.appendChild(row);
  }
  return table;
}

function linenoCell(n: number | null): HTMLTableCellElement {
  const cell = document.createElement("td");
  cell.className = "diff-lineno";
  cell.textContent = n === null ? "" : String(n);
  return cell;
}

function signCell(kind: DiffLine["kind"]): HTMLTableCellElement {
  const cell = document.createElement("td");
  cell.className = "diff-sign";
  cell.textContent = kind === "add" ? "+" : kind === "del" ? "-" : "";
  return cell;
}

function textCell(text: string): HTMLTableCellElement {
  const cell = document.createElement("td");
  cell.className = "diff-text";
  cell.textContent = text;
  return cell;
}

function showLoading(): void {
  bodyEl.classList.remove("diff-error");
  bodyEl.replaceChildren();
  const p = document.createElement("p");
  p.className = "diff-loading";
  p.textContent = "正在生成对比…";
  bodyEl.appendChild(p);
}

function showError(err: unknown): void {
  bodyEl.classList.add("diff-error");
  bodyEl.replaceChildren();
  const p = document.createElement("p");
  p.className = "diff-error-text";
  p.textContent = `无法生成对比：${String(err)}`;
  bodyEl.appendChild(p);
}

function labelForKind(kind: LogEntry["kind"]): string {
  switch (kind) {
    case "applyUpdate":
      return "应用更新 · 变更对比";
    case "translate":
      return "翻译 · 变更对比";
    case "translateError":
      return "翻译（需复核）· 变更对比";
    default:
      return "变更对比";
  }
}

function formatTimestamp(iso: string): string {
  const date = new Date(iso);
  return Number.isNaN(date.getTime()) ? iso : date.toLocaleString();
}
