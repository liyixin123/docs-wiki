// Owns the "⋯" overflow menu in the app header: open/close (button click,
// outside click, Escape), and the data-driven states of its items —
// "检查更新" badge, the three states of "翻译全部待翻译" (disabled /
// pending-count / in-progress), and the attention dot on the kebab button.
// Pure DOM, no api imports, so it can be tested standalone.

export interface AppMenuActions {
  onImportFolder: () => void;
  onImportRemote: () => void;
  onRemoveSource: () => void;
  onEnableTranslation: () => void;
  onCheckUpdates: () => void;
  onHistory: () => void;
  onTranslateAll: () => void;
  onOpenSettings: () => void;
}

export interface AppMenu {
  close(): void;
  setSourceName(name: string | null): void;
  setImportBusy(busy: boolean): void;
  setCheckUpdatesEnabled(enabled: boolean): void;
  setCheckUpdatesBusy(busy: boolean): void;
  setUpdateBadge(count: number): void;
  /** Show/hide "启用翻译…" — only meaningful for single-language sources. */
  setEnableTranslationVisible(visible: boolean): void;
  /** idle-empty: disabled + "无待翻译"; idle-pending: enabled + "N 篇" badge. */
  setTranslateAll(pendingCount: number): void;
  /** running: replaces the item with an inline progress row "done / total". */
  showTranslateProgress(done: number, total: number): void;
  setDot(hasAttention: boolean): void;
}

export function initAppMenu(actions: AppMenuActions): AppMenu {
  const kebab = document.querySelector<HTMLButtonElement>("#kebab-button")!;
  const menu = document.querySelector<HTMLElement>("#app-menu")!;
  const sourceSection = document.querySelector<HTMLElement>("#menu-source-section")!;
  const importFolder = document.querySelector<HTMLButtonElement>("#mi-import-folder")!;
  const importRemote = document.querySelector<HTMLButtonElement>("#mi-import-remote")!;
  const removeSource = document.querySelector<HTMLButtonElement>("#mi-remove-source")!;
  const enableTranslation = document.querySelector<HTMLButtonElement>("#mi-enable-translation")!;
  const checkUpdates = document.querySelector<HTMLButtonElement>("#mi-check-updates")!;
  const checkUpdatesLabel = document.querySelector<HTMLElement>("#mi-check-updates-label")!;
  const updatesBadge = document.querySelector<HTMLElement>("#menu-updates-badge")!;
  const history = document.querySelector<HTMLButtonElement>("#mi-history")!;
  const translateSlot = document.querySelector<HTMLElement>("#mi-translate-slot")!;
  const translateAll = document.querySelector<HTMLButtonElement>("#mi-translate-all")!;
  const translateHint = document.querySelector<HTMLElement>("#mi-translate-hint")!;
  const translateBadge = document.querySelector<HTMLElement>("#menu-translate-badge")!;
  const settings = document.querySelector<HTMLButtonElement>("#mi-settings")!;

  let running = false;
  let pendingCount = 0;

  kebab.addEventListener("click", () => (menu.hidden ? open() : close()));
  document.addEventListener("click", (event) => {
    if (!menu.hidden && !menu.contains(event.target as Node) && event.target !== kebab) close();
  });
  document.addEventListener("keydown", (event) => {
    if (event.key === "Escape" && !menu.hidden) close();
  });

  const items: Array<[HTMLButtonElement, () => void]> = [
    [importFolder, actions.onImportFolder],
    [importRemote, actions.onImportRemote],
    [removeSource, actions.onRemoveSource],
    [enableTranslation, actions.onEnableTranslation],
    [checkUpdates, actions.onCheckUpdates],
    [history, actions.onHistory],
    [translateAll, actions.onTranslateAll],
    [settings, actions.onOpenSettings],
  ];
  for (const [button, handler] of items) {
    button.addEventListener("click", () => {
      if (button.disabled) return;
      close();
      handler();
    });
  }

  function open(): void {
    menu.hidden = false;
    kebab.setAttribute("aria-expanded", "true");
  }

  function close(): void {
    if (running) {
      running = false;
      restoreTranslateSlot();
    }
    menu.hidden = true;
    kebab.setAttribute("aria-expanded", "false");
  }

  function restoreTranslateSlot(): void {
    if (translateAll.parentElement !== translateSlot) {
      translateSlot.replaceChildren(translateAll);
    }
    applyTranslateIdleState();
  }

  function applyTranslateIdleState(): void {
    translateAll.disabled = pendingCount === 0;
    translateHint.hidden = pendingCount > 0;
    translateBadge.hidden = pendingCount === 0;
    translateBadge.textContent = `${pendingCount} 篇`;
  }

  function setTranslateAll(count: number): void {
    running = false;
    pendingCount = count;
    restoreTranslateSlot();
  }

  function showTranslateProgress(done: number, total: number): void {
    running = true;
    const row = document.createElement("div");
    row.className = "menu-progress";
    const icon = document.createElement("span");
    icon.textContent = "🌐";
    const bar = document.createElement("div");
    bar.className = "menu-progress-bar";
    const fill = document.createElement("i");
    fill.style.width = `${total > 0 ? Math.round((done / total) * 100) : 0}%`;
    bar.appendChild(fill);
    const label = document.createElement("span");
    label.textContent = `${done} / ${total}`;
    row.append(icon, bar, label);
    translateSlot.replaceChildren(row);
  }

  return {
    close,
    setSourceName(name: string | null): void {
      sourceSection.textContent = name ? `来源 · ${name}` : "来源";
    },
    setEnableTranslationVisible(visible: boolean): void {
      enableTranslation.hidden = !visible;
    },
    setImportBusy(busy: boolean): void {
      importFolder.disabled = busy;
      importRemote.disabled = busy;
    },
    setCheckUpdatesEnabled(enabled: boolean): void {
      checkUpdates.disabled = !enabled;
    },
    setCheckUpdatesBusy(busy: boolean): void {
      checkUpdates.disabled = busy;
      checkUpdatesLabel.textContent = busy ? "检查中…" : "检查更新";
    },
    setUpdateBadge(count: number): void {
      updatesBadge.hidden = count <= 0;
      updatesBadge.textContent = `${count} 篇待更新`;
    },
    setTranslateAll,
    showTranslateProgress,
    setDot(hasAttention: boolean): void {
      kebab.classList.toggle("has-dot", hasAttention);
    },
  };
}
