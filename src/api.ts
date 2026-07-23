// Thin wrappers around the Tauri command surface (see src-tauri/src/commands.rs).
// Types here mirror the Rust structs 1:1 (serde emits camelCase field names).
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

export type SourceKind = "seed" | "localFolder" | "remoteGit";

export interface RemoteSpec {
  owner: string;
  repo: string;
  branch: string;
  path: string;
}

export interface Source {
  id: string;
  name: string;
  kind: SourceKind;
  languages: string[];
  primaryLanguage: string;
  remote: RemoteSpec | null;
  localPath: string | null;
  orderOverride: string[];
  createdAt: string;
  updatedAt: string;
}

export interface NavItem {
  docId: string;
  title: string;
}

export interface NavCategory {
  name: string;
  items: NavItem[];
}

export interface NavTree {
  sourceId: string;
  categories: NavCategory[];
}

export function listSources(): Promise<Source[]> {
  return invoke("list_sources");
}

export function getNav(sourceId: string): Promise<NavTree> {
  return invoke("get_nav", { sourceId });
}

export function getDocContent(
  sourceId: string,
  id: string,
  lang: string,
): Promise<string> {
  return invoke("get_doc_content", { sourceId, id, lang });
}

export function addLocalSource(path: string, name?: string): Promise<Source> {
  return invoke("add_local_source", { path, name });
}

export interface AddRemoteSourceInput {
  owner: string;
  repo: string;
  branch: string;
  path: string;
  name?: string;
  lang?: string;
}

export function addRemoteSource(input: AddRemoteSourceInput): Promise<Source> {
  return invoke("add_remote_source", { ...input });
}

export function removeSource(sourceId: string): Promise<void> {
  return invoke("remove_source", { sourceId });
}

export function setNavOverride(sourceId: string, orderedIds: string[]): Promise<void> {
  return invoke("set_nav_override", { sourceId, orderedIds });
}

export type CheckStatus = "same" | "changed" | "error" | "unknown";
export type TranslationStatus = "translated" | "pending" | "neverTranslated" | "needsReview" | "notApplicable";

export interface DocMeta {
  sourceId: string;
  id: string;
  title: string;
  category: string;
  primaryHash: string;
  lastCheckedAt: string | null;
  lastCheckStatus: CheckStatus;
  translationStatus: TranslationStatus;
  translatedAt: string | null;
  translatedBy: string | null;
}

export function listDocs(sourceId: string): Promise<DocMeta[]> {
  return invoke("list_docs", { sourceId });
}

export interface CheckSummary {
  checked: number;
  changed: string[];
  errored: string[];
}

export function checkUpdates(sourceId: string): Promise<CheckSummary> {
  return invoke("check_updates", { sourceId });
}

export function applyUpdate(sourceId: string, id: string): Promise<DocMeta> {
  return invoke("apply_update", { sourceId, id });
}

export type LogKind = "import" | "check" | "applyUpdate" | "translate" | "translateError";

export interface LogEntry {
  id: string;
  ts: string;
  sourceId: string;
  docId: string | null;
  kind: LogKind;
  detail: string;
}

export function getHistory(sourceId?: string, docId?: string): Promise<LogEntry[]> {
  return invoke("get_history", { sourceId, docId });
}

export type ProviderKind = "anthropicMessages" | "openAiCompatible";

export interface PublicProviderConfig {
  kind: ProviderKind;
  baseUrl: string;
  hasApiKey: boolean;
  model: string;
  maxOutputTokens: number;
}

export interface TranslationSettings {
  autoTranslateOnChange: boolean;
  checkOnStartup: boolean;
  concurrency: number;
  chunkThresholdChars: number;
}

export interface PublicConfig {
  activeProvider: string | null;
  providers: Record<string, PublicProviderConfig>;
  translation: TranslationSettings;
}

export function getConfig(): Promise<PublicConfig> {
  return invoke("get_config");
}

export interface ProviderInput {
  kind: ProviderKind;
  baseUrl: string;
  /** `undefined` (or blank) leaves the existing stored key untouched. */
  apiKey?: string;
  model: string;
  maxOutputTokens: number;
}

export interface ConfigInput {
  activeProvider: string | null;
  providers: Record<string, ProviderInput>;
  translation: TranslationSettings;
}

export function saveConfig(input: ConfigInput): Promise<void> {
  return invoke("save_config", { input });
}

export function testProviderConnection(providerId: string): Promise<void> {
  return invoke("test_provider_connection", { providerId });
}

export interface Snippet {
  text: string;
  highlightRanges: [number, number][];
}

export interface SearchHit {
  sourceId: string;
  docId: string;
  lang: string;
  title: string;
  score: number;
  snippets: Snippet[];
}

export interface SearchOpts {
  sourceId?: string;
  lang?: string;
  limit?: number;
}

export function searchDocs(query: string, opts: SearchOpts = {}): Promise<SearchHit[]> {
  return invoke("search_docs", {
    query,
    sourceId: opts.sourceId,
    lang: opts.lang,
    limit: opts.limit,
  });
}

export function translateDoc(sourceId: string, id: string): Promise<DocMeta> {
  return invoke("translate_doc", { sourceId, id });
}

export function translateAllPending(sourceId: string): Promise<void> {
  return invoke("translate_all_pending", { sourceId });
}

export interface TranslateProgress {
  done: number;
  total: number;
  currentTitle: string;
  success: boolean;
  error: string | null;
}

/** Subscribe to batch-translation progress events; returns an unsubscribe function. */
export function onTranslateProgress(callback: (progress: TranslateProgress) => void): Promise<() => void> {
  return listen<TranslateProgress>("translate-progress", (event) => callback(event.payload));
}

/** Bundles state.json + config.json (metadata only, not document content) into one file. */
export function exportBackup(destPath: string): Promise<void> {
  return invoke("export_backup", { destPath });
}

/** Restores state.json + config.json from a previously exported backup file. */
export function importBackup(srcPath: string): Promise<void> {
  return invoke("import_backup", { srcPath });
}
