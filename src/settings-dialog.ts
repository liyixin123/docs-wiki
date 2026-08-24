// Wires up the "⚙ 设置" native <dialog>: two fixed provider slots
// (anthropic / openai-compatible) plus translation behavior settings.
// Owns its own DOM elements; the API key inputs never show a stored key —
// only a placeholder noting one is configured — since the backend never
// sends the real key to the frontend in the first place.
import { ask, message as messageDialog, open as openFileDialog, save as saveFileDialog } from "@tauri-apps/plugin-dialog";
import {
  exportBackup,
  exportLibrary,
  getConfig,
  importLibrary,
  importBackup,
  saveConfig,
  testProviderConnection,
  type ConfigInput,
  type PublicConfig,
} from "./api";

const DEFAULTS = {
  anthropic: { baseUrl: "https://api.anthropic.com", model: "claude-sonnet-4-5" },
  openai: { baseUrl: "https://api.openai.com/v1", model: "gpt-4o" },
};

export function initSettingsDialog(
  onBackupImported: () => Promise<void>,
): { open: () => void } {
  const dialog = document.querySelector<HTMLDialogElement>("#settings-dialog")!;
  const form = document.querySelector<HTMLFormElement>("#settings-form")!;
  const cancelButton = document.querySelector<HTMLButtonElement>("#settings-cancel")!;
  const errorEl = document.querySelector<HTMLElement>("#settings-error")!;
  const activeProviderSelect = document.querySelector<HTMLSelectElement>("#settings-active-provider")!;

  const anthropic = {
    baseUrl: document.querySelector<HTMLInputElement>("#anthropic-base-url")!,
    model: document.querySelector<HTMLInputElement>("#anthropic-model")!,
    key: document.querySelector<HTMLInputElement>("#anthropic-key")!,
    maxTokens: document.querySelector<HTMLInputElement>("#anthropic-max-tokens")!,
    testButton: document.querySelector<HTMLButtonElement>("#anthropic-test")!,
    testResult: document.querySelector<HTMLElement>("#anthropic-test-result")!,
  };
  const openai = {
    baseUrl: document.querySelector<HTMLInputElement>("#openai-base-url")!,
    model: document.querySelector<HTMLInputElement>("#openai-model")!,
    key: document.querySelector<HTMLInputElement>("#openai-key")!,
    maxTokens: document.querySelector<HTMLInputElement>("#openai-max-tokens")!,
    testButton: document.querySelector<HTMLButtonElement>("#openai-test")!,
    testResult: document.querySelector<HTMLElement>("#openai-test-result")!,
  };

  const autoTranslate = document.querySelector<HTMLInputElement>("#auto-translate-on-change")!;
  const checkOnStartup = document.querySelector<HTMLInputElement>("#check-on-startup")!;
  const concurrency = document.querySelector<HTMLInputElement>("#translation-concurrency")!;
  const chunkThreshold = document.querySelector<HTMLInputElement>("#chunk-threshold")!;
  const githubToken = document.querySelector<HTMLInputElement>("#github-token")!;
  const githubUseGhCli = document.querySelector<HTMLInputElement>("#github-use-gh-cli")!;
  const exportBackupButton = document.querySelector<HTMLButtonElement>("#export-backup-button")!;
  const importBackupButton = document.querySelector<HTMLButtonElement>("#import-backup-button")!;
  const exportLibraryButton = document.querySelector<HTMLButtonElement>("#export-library-button")!;
  const importLibraryButton = document.querySelector<HTMLButtonElement>("#import-library-button")!;

  cancelButton.addEventListener("click", () => dialog.close());

  // Segmented tabs: providers / behavior / backup.
  const tabButtons = Array.from(document.querySelectorAll<HTMLButtonElement>("#settings-dialog .settings-tab"));
  const panes = Array.from(document.querySelectorAll<HTMLElement>("#settings-dialog .settings-pane"));
  for (const tab of tabButtons) {
    tab.addEventListener("click", () => {
      for (const other of tabButtons) other.classList.toggle("active", other === tab);
      for (const pane of panes) pane.hidden = pane.dataset.pane !== tab.dataset.tab;
    });
  }
  form.addEventListener("submit", (event) => {
    event.preventDefault();
    void submitAndClose();
  });
  anthropic.testButton.addEventListener("click", () => void runTest("anthropic", anthropic));
  openai.testButton.addEventListener("click", () => void runTest("openai", openai));
  exportBackupButton.addEventListener("click", () => void runExportBackup());
  importBackupButton.addEventListener("click", () => void runImportBackup());
  exportLibraryButton.addEventListener("click", () => void runExportLibrary());
  importLibraryButton.addEventListener("click", () => void runImportLibrary());

  async function openDialog(): Promise<void> {
    errorEl.hidden = true;
    anthropic.testResult.textContent = "";
    openai.testResult.textContent = "";
    try {
      populate(await getConfig());
    } catch (err) {
      errorEl.textContent = String(err);
      errorEl.hidden = false;
    }
    dialog.showModal();
  }

  function populate(cfg: PublicConfig): void {
    activeProviderSelect.value = cfg.activeProvider ?? "anthropic";

    const a = cfg.providers.anthropic;
    anthropic.baseUrl.value = a?.baseUrl ?? DEFAULTS.anthropic.baseUrl;
    anthropic.model.value = a?.model ?? DEFAULTS.anthropic.model;
    anthropic.maxTokens.value = String(a?.maxOutputTokens ?? 8192);
    anthropic.key.value = "";
    anthropic.key.placeholder = a?.hasApiKey ? "已配置（留空则不修改）" : "例如 $ANTHROPIC_API_KEY";

    const o = cfg.providers.openai;
    openai.baseUrl.value = o?.baseUrl ?? DEFAULTS.openai.baseUrl;
    openai.model.value = o?.model ?? DEFAULTS.openai.model;
    openai.maxTokens.value = String(o?.maxOutputTokens ?? 8192);
    openai.key.value = "";
    openai.key.placeholder = o?.hasApiKey ? "已配置（留空则不修改）" : "例如 $OPENAI_API_KEY";

    autoTranslate.checked = cfg.translation.autoTranslateOnChange;
    checkOnStartup.checked = cfg.translation.checkOnStartup;
    concurrency.value = String(cfg.translation.concurrency);
    chunkThreshold.value = String(cfg.translation.chunkThresholdChars);
    githubToken.value = "";
    githubToken.placeholder = cfg.github.hasToken ? "已配置（留空则不修改）" : "例如 $GITHUB_TOKEN";
    githubUseGhCli.checked = cfg.github.useGhCli;
  }

  function buildInput(): ConfigInput {
    return {
      activeProvider: activeProviderSelect.value,
      providers: {
        anthropic: {
          kind: "anthropicMessages",
          baseUrl: anthropic.baseUrl.value.trim(),
          apiKey: anthropic.key.value.trim() || undefined,
          model: anthropic.model.value.trim(),
          maxOutputTokens: Number(anthropic.maxTokens.value) || 8192,
        },
        openai: {
          kind: "openAiCompatible",
          baseUrl: openai.baseUrl.value.trim(),
          apiKey: openai.key.value.trim() || undefined,
          model: openai.model.value.trim(),
          maxOutputTokens: Number(openai.maxTokens.value) || 8192,
        },
      },
      translation: {
        autoTranslateOnChange: autoTranslate.checked,
        checkOnStartup: checkOnStartup.checked,
        concurrency: Number(concurrency.value) || 2,
        chunkThresholdChars: Number(chunkThreshold.value) || 12_000,
      },
      github: {
        token: githubToken.value.trim() || undefined,
        useGhCli: githubUseGhCli.checked,
      },
    };
  }

  async function submitAndClose(): Promise<void> {
    errorEl.hidden = true;
    try {
      await saveConfig(buildInput());
      dialog.close();
    } catch (err) {
      errorEl.textContent = String(err);
      errorEl.hidden = false;
    }
  }

  async function runTest(
    providerId: string,
    fields: { testButton: HTMLButtonElement; testResult: HTMLElement },
  ): Promise<void> {
    fields.testResult.textContent = "测试中…";
    fields.testResult.className = "provider-test-result";
    fields.testButton.disabled = true;
    try {
      // Save what's currently in the form first — test_provider_connection
      // reads config.json from disk, not the unsaved form state.
      await saveConfig(buildInput());
      await testProviderConnection(providerId);
      fields.testResult.textContent = "✓ 连接成功";
      fields.testResult.className = "provider-test-result provider-test-ok";
    } catch (err) {
      fields.testResult.textContent = `✗ ${String(err)}`;
      fields.testResult.className = "provider-test-result provider-test-error";
    } finally {
      fields.testButton.disabled = false;
    }
  }

  async function runExportBackup(): Promise<void> {
    errorEl.hidden = true;
    const path = await saveFileDialog({
      defaultPath: "docswiki-backup.json",
      filters: [{ name: "JSON", extensions: ["json"] }],
    });
    if (!path) return; // user cancelled

    try {
      await exportBackup(path);
      // window.alert is a no-op in Tauri's WKWebView — use the native dialog.
      await messageDialog("备份已导出。", { title: "备份" });
    } catch (err) {
      errorEl.textContent = String(err);
      errorEl.hidden = false;
    }
  }

  async function runExportLibrary(): Promise<void> {
    errorEl.hidden = true;
    const path = await saveFileDialog({
      defaultPath: "docswiki-library.docswiki",
      filters: [{ name: "DocsWiki 文档库", extensions: ["docswiki"] }],
    });
    if (!path) return; // user cancelled

    try {
      await exportLibrary(path);
      await messageDialog("文档库已导出。", { title: "文档库分享" });
    } catch (err) {
      errorEl.textContent = String(err);
      errorEl.hidden = false;
    }
  }

  async function runImportLibrary(): Promise<void> {
    errorEl.hidden = true;
    const path = await openFileDialog({
      multiple: false,
      filters: [{ name: "DocsWiki 文档库", extensions: ["docswiki"] }],
    });
    if (!path || Array.isArray(path)) return; // user cancelled

    try {
      const summary = await importLibrary(path);
      dialog.close();
      await onBackupImported();
      await messageDialog(
        summary.imported > 0
          ? `导入完成：新增 ${summary.imported} 个来源${summary.skipped > 0 ? `，跳过已存在的 ${summary.skipped} 个` : ""}。`
          : "没有导入任何来源：文档库中的来源都已存在。",
        { title: "文档库分享" },
      );
    } catch (err) {
      errorEl.textContent = String(err);
      errorEl.hidden = false;
    }
  }

  async function runImportBackup(): Promise<void> {
    errorEl.hidden = true;
    const path = await openFileDialog({ multiple: false, filters: [{ name: "JSON", extensions: ["json"] }] });
    if (!path || Array.isArray(path)) return; // user cancelled

    const confirmed = await ask(
      "导入备份会覆盖当前的来源列表、文档状态和翻译设置（不影响已下载的文档正文），确定要继续吗？",
      { title: "导入备份", kind: "warning" },
    );
    if (!confirmed) return;

    try {
      await importBackup(path);
      dialog.close();
      await onBackupImported();
      await messageDialog("备份已导入。", { title: "备份" });
    } catch (err) {
      errorEl.textContent = String(err);
      errorEl.hidden = false;
    }
  }

  return { open: () => void openDialog() };
}
