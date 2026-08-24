// @vitest-environment jsdom
// Tests the ⋯ overflow menu's externally visible behavior: open/close,
// item dispatch, and the three states of "翻译全部待翻译".
import { beforeEach, describe, expect, it, vi } from "vitest";
import { initAppMenu } from "./app-menu";

const FIXTURE = `
<button id="kebab-button">⋯</button>
<div id="app-menu" hidden>
  <div class="menu-section" id="menu-source-section">来源</div>
  <button id="mi-import-folder">导入本地文件夹</button>
  <button id="mi-import-remote">从 GitHub 导入…</button>
  <button id="mi-remove-source">移除当前来源</button>
  <button id="mi-check-updates"><span id="mi-check-updates-label">检查更新</span><span id="menu-updates-badge" hidden></span></button>
  <button id="mi-history">历史记录</button>
  <div id="mi-translate-slot">
    <button id="mi-enable-translation"></button>
<button id="mi-translate-all">翻译全部待翻译<span id="mi-translate-hint" hidden>无待翻译</span><span id="menu-translate-badge" hidden></span></button>
  </div>
  <button id="mi-settings">设置</button>
</div>`;

function setup() {
  const actions = {
    onImportFolder: vi.fn(),
    onImportRemote: vi.fn(),
    onRemoveSource: vi.fn(),
    onEnableTranslation: vi.fn(),
    onCheckUpdates: vi.fn(),
    onHistory: vi.fn(),
    onTranslateAll: vi.fn(),
    onOpenSettings: vi.fn(),
  };
  const menu = initAppMenu(actions);
  return { menu, actions };
}

function kebab(): HTMLButtonElement {
  return document.querySelector("#kebab-button")!;
}
function menuEl(): HTMLElement {
  return document.querySelector("#app-menu")!;
}
function translateButton(): HTMLButtonElement {
  return document.querySelector("#mi-translate-all")!;
}

beforeEach(() => {
  document.body.innerHTML = FIXTURE;
});

describe("open/close", () => {
  it("opens and closes on kebab clicks", () => {
    setup();
    kebab().click();
    expect(menuEl().hidden).toBe(false);
    expect(kebab().getAttribute("aria-expanded")).toBe("true");
    kebab().click();
    expect(menuEl().hidden).toBe(true);
  });

  it("closes on outside click", () => {
    setup();
    kebab().click();
    document.body.click();
    expect(menuEl().hidden).toBe(true);
  });

  it("closes on Escape", () => {
    setup();
    kebab().click();
    document.dispatchEvent(new KeyboardEvent("keydown", { key: "Escape" }));
    expect(menuEl().hidden).toBe(true);
  });

  it("dispatches the action and closes when an item is clicked", () => {
    const { actions } = setup();
    kebab().click();
    document.querySelector<HTMLButtonElement>("#mi-history")!.click();
    expect(actions.onHistory).toHaveBeenCalledOnce();
    expect(menuEl().hidden).toBe(true);
  });

  it("does not dispatch when the item is disabled", () => {
    const { menu, actions } = setup();
    menu.setTranslateAll(0);
    kebab().click();
    translateButton().click();
    expect(actions.onTranslateAll).not.toHaveBeenCalled();
  });
});

describe("data-driven item states", () => {
  it("shows the source name in the section header", () => {
    const { menu } = setup();
    menu.setSourceName("rust-lang/book");
    expect(document.querySelector("#menu-source-section")!.textContent).toContain("rust-lang/book");
  });

  it("shows the update badge count and the kebab dot", () => {
    const { menu } = setup();
    menu.setUpdateBadge(2);
    const badge = document.querySelector<HTMLElement>("#menu-updates-badge")!;
    expect(badge.hidden).toBe(false);
    expect(badge.textContent).toContain("2");
    menu.setDot(true);
    expect(kebab().classList.contains("has-dot")).toBe(true);
  });

  it("toggles the check-updates busy label", () => {
    const { menu } = setup();
    menu.setCheckUpdatesBusy(true);
    expect(document.querySelector("#mi-check-updates-label")!.textContent).toBe("检查中…");
    menu.setCheckUpdatesBusy(false);
    expect(document.querySelector("#mi-check-updates-label")!.textContent).toBe("检查更新");
  });
});

describe("翻译全部 three states", () => {
  it("idle-empty: disabled with 无待翻译 hint", () => {
    const { menu } = setup();
    menu.setTranslateAll(0);
    expect(translateButton().disabled).toBe(true);
    expect(document.querySelector<HTMLElement>("#mi-translate-hint")!.hidden).toBe(false);
    expect(document.querySelector<HTMLElement>("#menu-translate-badge")!.hidden).toBe(true);
  });

  it("idle-pending: enabled with N 篇 badge", () => {
    const { menu } = setup();
    menu.setTranslateAll(5);
    expect(translateButton().disabled).toBe(false);
    const badge = document.querySelector<HTMLElement>("#menu-translate-badge")!;
    expect(badge.hidden).toBe(false);
    expect(badge.textContent).toContain("5");
    expect(document.querySelector<HTMLElement>("#mi-translate-hint")!.hidden).toBe(true);
  });

  it("running: inline progress row, restored on finish", () => {
    const { menu } = setup();
    menu.setTranslateAll(5);
    menu.showTranslateProgress(2, 5);

    // The button is replaced by a progress row.
    const row = document.querySelector<HTMLElement>(".menu-progress")!;
    expect(row).not.toBeNull();
    expect(row.textContent).toContain("2 / 5");
    expect(document.querySelector("#mi-translate-all")).toBeNull();

    // Finishing puts the button back, back in idle-empty state.
    menu.setTranslateAll(0);
    const restored = document.querySelector<HTMLButtonElement>("#mi-translate-all")!;
    expect(restored).not.toBeNull();
    expect(restored.disabled).toBe(true);
    expect(document.querySelector(".menu-progress")).toBeNull();
  });

  it("closing the menu mid-run restores the translate item", () => {
    const { menu } = setup();
    menu.setTranslateAll(5);
    menu.showTranslateProgress(1, 5);
    kebab().click(); // open
    menu.close();
    expect(document.querySelector<HTMLButtonElement>("#mi-translate-all")).not.toBeNull();
    expect(document.querySelector(".menu-progress")).toBeNull();
  });
});
