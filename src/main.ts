// App entry point: bootstraps sources, wires the source selector, the
// language toggle, the sidebar nav, the search box, and the content pane
// together.
import { open as openFolderDialog } from "@tauri-apps/plugin-dialog";
import {
  addLocalSource,
  applyUpdate,
  checkUpdates,
  getDocContent,
  getHistory,
  getNav,
  listDocs,
  listSources,
  onTranslateProgress,
  removeSource,
  searchDocs,
  setNavOverride,
  translateAllPending,
  translateDoc,
  type DocMeta,
  type NavTree,
  type SearchHit,
  type Source,
  type TranslateProgress,
} from "./api";
import { initDiffDialog, openDiffDialog } from "./diff-dialog";
import { initDocLinks, scrollToFragment } from "./doc-links";
import { renderHistory } from "./history-view";
import { languageLabel } from "./lang";
import { renderNav } from "./nav-view";
import { initRemoteSourceDialog } from "./remote-source-dialog";
import { renderSearchResults } from "./search-view";
import { initSettingsDialog } from "./settings-dialog";
import { showDoc, showError, showLoading, showPlaceholder, type DocBanner } from "./content-view";

interface AppState {
  sources: Source[];
  currentSource: Source | null;
  currentLang: string;
  currentDocId: string | null;
  currentTree: NavTree | null;
  docMetaById: Map<string, DocMeta>;
}

const state: AppState = {
  sources: [],
  currentSource: null,
  currentLang: "en",
  currentDocId: null,
  currentTree: null,
  docMetaById: new Map(),
};

const SEARCH_DEBOUNCE_MS = 250;

let sourceSelectEl: HTMLSelectElement;
let addSourceButtonEl: HTMLButtonElement;
let removeSourceButtonEl: HTMLButtonElement;
let checkUpdatesButtonEl: HTMLButtonElement;
let historyButtonEl: HTMLButtonElement;
let translateAllButtonEl: HTMLButtonElement;
let translateProgressEl: HTMLElement;
let searchInputEl: HTMLInputElement;
let langToggleEl: HTMLElement;
let navEl: HTMLElement;
let contentEl: HTMLElement;
let searchDebounceHandle: number | undefined;
/** Fragment from an in-content doc link (`other.md#section`), scrolled to
 * once the target doc has rendered. Null when the navigation had none. */
let pendingFragment: string | null = null;

window.addEventListener("DOMContentLoaded", () => {
  sourceSelectEl = document.querySelector("#source-select")!;
  addSourceButtonEl = document.querySelector("#add-source-button")!;
  removeSourceButtonEl = document.querySelector("#remove-source-button")!;
  checkUpdatesButtonEl = document.querySelector("#check-updates-button")!;
  historyButtonEl = document.querySelector("#history-button")!;
  translateAllButtonEl = document.querySelector("#translate-all-button")!;
  translateProgressEl = document.querySelector("#translate-progress")!;
  searchInputEl = document.querySelector("#search-input")!;
  langToggleEl = document.querySelector("#lang-toggle")!;
  navEl = document.querySelector("#nav")!;
  contentEl = document.querySelector("#content")!;

  sourceSelectEl.addEventListener("change", () => {
    clearSearch();
    void selectSource(sourceSelectEl.value);
  });
  searchInputEl.addEventListener("input", onSearchInput);
  addSourceButtonEl.addEventListener("click", () => void addLocalFolderSource());
  removeSourceButtonEl.addEventListener("click", () => void removeCurrentSource());
  checkUpdatesButtonEl.addEventListener("click", () => void runCheckUpdates());
  historyButtonEl.addEventListener("click", () => void showHistory());
  translateAllButtonEl.addEventListener("click", () => void runTranslateAllPending());
  initRemoteSourceDialog((source) => applyNewSource(source));
  initSettingsDialog(() => refreshSourcesAfterExternalChange());
  initDiffDialog();
  initDocLinks(contentEl, {
    currentDocId: () => state.currentDocId,
    resolveKnownId: (docId) => resolveDocIdCaseInsensitive(docId),
    resolveRouteSuffix: (docId) => resolveDocIdBySuffix(docId),
    navigate: (docId, fragment) => void openLinkedDoc(docId, fragment),
  });
  void onTranslateProgress(handleTranslateProgress);

  void bootstrap();
});

async function bootstrap(): Promise<void> {
  showLoading(contentEl);
  try {
    state.sources = await listSources();
  } catch (err) {
    showError(contentEl, String(err));
    return;
  }

  if (state.sources.length === 0) {
    showError(contentEl, "没有可用的文档来源。");
    return;
  }

  populateSourceSelect(state.sources);
  await selectSource(state.sources[0].id);
}

function populateSourceSelect(sources: Source[]): void {
  sourceSelectEl.replaceChildren();
  for (const source of sources) {
    const option = document.createElement("option");
    option.value = source.id;
    option.textContent = source.name;
    sourceSelectEl.appendChild(option);
  }
}

async function addLocalFolderSource(): Promise<void> {
  const folder = await openFolderDialog({ directory: true, multiple: false, title: "选择要导入的文档文件夹" });
  if (!folder || Array.isArray(folder)) return; // user cancelled

  addSourceButtonEl.disabled = true;
  const originalLabel = addSourceButtonEl.textContent;
  addSourceButtonEl.textContent = "导入中…";
  try {
    const newSource = await addLocalSource(folder);
    await applyNewSource(newSource);
  } catch (err) {
    showError(contentEl, String(err));
  } finally {
    addSourceButtonEl.disabled = false;
    addSourceButtonEl.textContent = originalLabel;
  }
}

/** Common "a new source just got imported" flow, shared by local-folder and
 * remote-git import: refresh the source list, then switch to it. */
async function applyNewSource(source: Source): Promise<void> {
  state.sources = await listSources();
  populateSourceSelect(state.sources);
  clearSearch();
  await selectSource(source.id);
}

/** Refresh the full source list and either select the first one or show
 * the empty state — shared by source removal and backup import, since both
 * can leave the app with zero or a completely different set of sources. */
async function refreshSourcesAfterExternalChange(): Promise<void> {
  state.sources = await listSources();
  populateSourceSelect(state.sources);
  clearSearch();

  if (state.sources.length > 0) {
    await selectSource(state.sources[0].id);
  } else {
    state.currentSource = null;
    state.currentDocId = null;
    state.currentTree = null;
    state.docMetaById = new Map();
    navEl.replaceChildren();
    langToggleEl.hidden = true;
    checkUpdatesButtonEl.disabled = true;
    translateAllButtonEl.disabled = true;
    showError(contentEl, "没有可用的文档来源。");
  }
}

async function removeCurrentSource(): Promise<void> {
  const source = state.currentSource;
  if (!source) return;

  const confirmed = window.confirm(`确定要移除来源「${source.name}」吗？本地缓存的文档也会被一并删除。`);
  if (!confirmed) return;

  try {
    await removeSource(source.id);
    await refreshSourcesAfterExternalChange();
  } catch (err) {
    showError(contentEl, String(err));
  }
}

/** Switch to a source, optionally jumping straight to a specific doc/lang
 * (used when a global search result points at a different source). */
async function selectSource(
  sourceId: string,
  jumpTo?: { docId: string; lang: string },
): Promise<void> {
  const source = state.sources.find((s) => s.id === sourceId) ?? null;
  state.currentSource = source;
  state.currentDocId = null;
  sourceSelectEl.value = sourceId;

  if (!source) {
    showError(contentEl, `未知的文档来源：${sourceId}`);
    return;
  }

  // Prefer Chinese for bilingual sources (this app's primary audience),
  // otherwise fall back to whatever the source declares as primary.
  state.currentLang =
    jumpTo && source.languages.includes(jumpTo.lang)
      ? jumpTo.lang
      : source.languages.includes("zh")
        ? "zh"
        : source.primaryLanguage;
  renderLangToggle(source);
  checkUpdatesButtonEl.disabled = !source.remote && !source.localPath;
  translateAllButtonEl.disabled = source.languages.length < 2;

  showLoading(contentEl);
  try {
    state.currentTree = await getNav(sourceId);
  } catch (err) {
    showError(contentEl, String(err));
    return;
  }

  await refreshDocMetas(sourceId);
  renderCurrentNav(state.currentDocId);

  const firstDocId = state.currentTree.categories.find((c) => c.items.length > 0)
    ?.items[0]?.docId;
  const targetDocId = jumpTo?.docId ?? firstDocId;
  if (targetDocId) {
    await selectDoc(targetDocId);
  } else {
    showPlaceholder(contentEl);
  }
}

function renderLangToggle(source: Source): void {
  langToggleEl.replaceChildren();
  if (source.languages.length < 2) {
    langToggleEl.hidden = true;
    return;
  }
  langToggleEl.hidden = false;

  for (const lang of source.languages) {
    const button = document.createElement("button");
    button.type = "button";
    button.className = "lang-button";
    button.textContent = languageLabel(lang);
    button.dataset.lang = lang;
    if (lang === state.currentLang) {
      button.classList.add("active");
    }
    button.addEventListener("click", () => {
      if (state.currentLang === lang) return;
      state.currentLang = lang;
      for (const sibling of langToggleEl.querySelectorAll(".lang-button")) {
        sibling.classList.toggle("active", sibling === button);
      }
      void reloadContent();
    });
    langToggleEl.appendChild(button);
  }
}

async function selectDoc(docId: string): Promise<void> {
  state.currentDocId = docId;
  renderCurrentNav(docId);
  await reloadContent();
}

/** Navigate to a doc linked from the rendered content of another doc. Falls
 * back to the source's primary language when the target was never translated
 * into the current one — otherwise the jump would land on a read error. */
async function openLinkedDoc(docId: string, fragment: string): Promise<void> {
  const source = state.currentSource;
  const meta = state.docMetaById.get(docId);
  if (!source || !meta) return;

  if (meta.translationStatus === "neverTranslated" && state.currentLang !== source.primaryLanguage) {
    state.currentLang = source.primaryLanguage;
    renderLangToggle(source);
  }
  pendingFragment = fragment || null;
  await selectDoc(docId);
}

/** Doc ids come from filenames, often authored on case-insensitive
 * filesystems, so `[x](Quickstart.md)` should still find `quickstart`. */
function resolveDocIdCaseInsensitive(docId: string): string | null {
  if (state.docMetaById.has(docId)) return docId;
  const lower = docId.toLowerCase();
  for (const id of state.docMetaById.keys()) {
    if (id.toLowerCase() === lower) return id;
  }
  return null;
}

/** Doc-site generators link by site route rather than doc id — e.g. Yazi's
 * Docusaurus links look like `/docs/configuration/yazi`, which resolves to
 * the candidate `docs/configuration/yazi` and matches no doc verbatim.
 * Progressively dropping leading segments lands on the real id
 * (`configuration/yazi`). Only used as a retry for absolute links, so plain
 * relative `.md` links keep their exact-match behavior. */
function resolveDocIdBySuffix(docId: string): string | null {
  let suffix = docId;
  while (suffix.includes("/")) {
    suffix = suffix.slice(suffix.indexOf("/") + 1);
    const match = resolveDocIdCaseInsensitive(suffix);
    if (match) return match;
  }
  return null;
}

/** Render the sidebar for the current source/tree, wiring up doc selection,
 * drag-to-reorder (persisted via set_nav_override), and "needs update" badges. */
function renderCurrentNav(activeDocId: string | null): void {
  if (!state.currentTree) return;
  const sourceId = state.currentSource?.id;
  const changedDocIds = new Set(
    Array.from(state.docMetaById.values())
      .filter((d) => d.lastCheckStatus === "changed")
      .map((d) => d.id),
  );
  renderNav(
    navEl,
    state.currentTree,
    activeDocId,
    (docId) => void selectDoc(docId),
    sourceId ? (orderedIds) => void persistNavOrder(sourceId, orderedIds) : undefined,
    changedDocIds,
  );
}

async function refreshDocMetas(sourceId: string): Promise<void> {
  try {
    const docs = await listDocs(sourceId);
    state.docMetaById = new Map(docs.map((d) => [d.id, d]));
  } catch (err) {
    console.error("failed to load doc metadata:", err);
    state.docMetaById = new Map();
  }
}

async function persistNavOrder(sourceId: string, orderedIds: string[]): Promise<void> {
  try {
    await setNavOverride(sourceId, orderedIds);
  } catch (err) {
    // Best-effort: the DOM already reflects the drag result; a failure here
    // just means it won't survive a reload. Not worth interrupting the user
    // with a modal for what's a rare failure mode (e.g. source removed
    // mid-drag).
    console.error("failed to persist nav order:", err);
  }
}

async function reloadContent(): Promise<void> {
  const source = state.currentSource;
  const docId = state.currentDocId;
  if (!source || !docId) {
    pendingFragment = null;
    showPlaceholder(contentEl);
    return;
  }

  showLoading(contentEl);
  try {
    const markdown = await getDocContent(source.id, docId, state.currentLang);
    showDoc(contentEl, markdown, buildDocBanners(source.id, docId));
    if (pendingFragment) {
      scrollToFragment(contentEl, pendingFragment);
    }
  } catch (err) {
    showError(contentEl, String(err));
  } finally {
    pendingFragment = null;
  }
}

function buildDocBanners(sourceId: string, docId: string): DocBanner[] {
  const meta = state.docMetaById.get(docId);
  if (!meta) return [];

  const banners: DocBanner[] = [];
  if (meta.lastCheckStatus === "changed") {
    banners.push({
      text: "上游内容已更新。",
      buttonLabel: "应用更新",
      onClick: () => void applyDocUpdate(sourceId, docId),
    });
  }
  if (meta.translationStatus === "pending" || meta.translationStatus === "neverTranslated") {
    banners.push({
      text: meta.translationStatus === "pending" ? "原文已更新，翻译需要刷新。" : "此文档尚未翻译。",
      buttonLabel: "翻译",
      onClick: () => void translateCurrentDoc(sourceId, docId),
    });
  } else if (meta.translationStatus === "needsReview") {
    banners.push({
      text: "翻译完成，但结构自查未通过，建议人工核对。",
      buttonLabel: "重新翻译",
      onClick: () => void translateCurrentDoc(sourceId, docId),
    });
  }
  return banners;
}

async function applyDocUpdate(sourceId: string, docId: string): Promise<void> {
  try {
    const updated = await applyUpdate(sourceId, docId);
    state.docMetaById.set(docId, updated);
    renderCurrentNav(state.currentDocId);
    await reloadContent();
  } catch (err) {
    showError(contentEl, String(err));
  }
}

async function translateCurrentDoc(sourceId: string, docId: string): Promise<void> {
  showLoading(contentEl);
  try {
    const updated = await translateDoc(sourceId, docId);
    state.docMetaById.set(docId, updated);
    renderCurrentNav(state.currentDocId);
  } catch (err) {
    showError(contentEl, String(err), "翻译失败");
    return;
  }
  await reloadContent();
}

async function runCheckUpdates(): Promise<void> {
  const source = state.currentSource;
  if (!source) return;

  checkUpdatesButtonEl.disabled = true;
  const originalLabel = checkUpdatesButtonEl.textContent;
  checkUpdatesButtonEl.textContent = "检查中…";
  try {
    const summary = await checkUpdates(source.id);
    await refreshDocMetas(source.id);
    renderCurrentNav(state.currentDocId);
    await reloadContent();
    window.alert(
      `检查完成：共检查 ${summary.checked} 篇，${summary.changed.length} 篇有更新，${summary.errored.length} 篇检查失败。`,
    );
  } catch (err) {
    showError(contentEl, String(err));
  } finally {
    checkUpdatesButtonEl.disabled = !source.remote && !source.localPath;
    checkUpdatesButtonEl.textContent = originalLabel;
  }
}

async function runTranslateAllPending(): Promise<void> {
  const source = state.currentSource;
  if (!source) return;

  const pendingCount = Array.from(state.docMetaById.values()).filter(
    (d) => d.translationStatus === "pending" || d.translationStatus === "neverTranslated",
  ).length;
  if (pendingCount === 0) {
    window.alert("没有待翻译的文档。");
    return;
  }
  const confirmed = window.confirm(`预计翻译 ${pendingCount} 篇文档，可能需要一些时间并消耗 API 额度，确认继续？`);
  if (!confirmed) return;

  translateAllButtonEl.disabled = true;
  translateProgressEl.hidden = false;
  translateProgressEl.textContent = `翻译中… 0/${pendingCount}`;
  try {
    await translateAllPending(source.id);
  } catch (err) {
    showError(contentEl, String(err), "翻译失败");
  } finally {
    await refreshDocMetas(source.id);
    renderCurrentNav(state.currentDocId);
    await reloadContent();
    translateAllButtonEl.disabled = source.languages.length < 2;
    translateProgressEl.hidden = true;
  }
}

function handleTranslateProgress(progress: TranslateProgress): void {
  translateProgressEl.hidden = false;
  const status = progress.success ? "" : `（失败：${progress.error ?? "未知错误"}）`;
  translateProgressEl.textContent = `翻译中… ${progress.done}/${progress.total} - ${progress.currentTitle}${status}`;
}

async function showHistory(): Promise<void> {
  showLoading(contentEl);
  try {
    const entries = await getHistory(state.currentSource?.id);
    renderHistory(contentEl, entries, (entry) => void openDiffDialog(entry));
  } catch (err) {
    showError(contentEl, String(err));
  }
}

function onSearchInput(): void {
  const query = searchInputEl.value.trim();
  window.clearTimeout(searchDebounceHandle);

  if (!query) {
    void reloadContent();
    return;
  }
  searchDebounceHandle = window.setTimeout(() => void performSearch(query), SEARCH_DEBOUNCE_MS);
}

async function performSearch(query: string): Promise<void> {
  showLoading(contentEl);
  try {
    const hits = await searchDocs(query, { limit: 30 });
    // A later keystroke's search may resolve before this one — drop stale results.
    if (searchInputEl.value.trim() !== query) return;
    const sourceNames = new Map(state.sources.map((s) => [s.id, s.name]));
    renderSearchResults(contentEl, query, hits, sourceNames, (hit) => void openSearchHit(hit));
  } catch (err) {
    showError(contentEl, String(err));
  }
}

async function openSearchHit(hit: SearchHit): Promise<void> {
  clearSearch();
  if (state.currentSource?.id === hit.sourceId) {
    if (state.currentSource.languages.includes(hit.lang)) {
      state.currentLang = hit.lang;
      renderLangToggle(state.currentSource);
    }
    await selectDoc(hit.docId);
  } else {
    await selectSource(hit.sourceId, { docId: hit.docId, lang: hit.lang });
  }
}

function clearSearch(): void {
  window.clearTimeout(searchDebounceHandle);
  searchInputEl.value = "";
}
