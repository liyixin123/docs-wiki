// @vitest-environment jsdom
// Tests the sidebar collapse: rail toggle, localStorage persistence, and
// the Cmd/Ctrl+B shortcut.
import { describe, expect, it, vi } from "vitest";
import { initSidebarCollapse } from "./sidebar-collapse";

function setup() {
  const nav = document.createElement("aside");
  const button = document.createElement("button");
  button.id = "nav-toggle-button";
  document.body.append(nav, button);
  const storage = { getItem: vi.fn(() => null), setItem: vi.fn() };
  const sidebar = initSidebarCollapse(nav, button, storage);
  return { nav, button, storage, sidebar };
}

function pressCmdB() {
  document.dispatchEvent(new KeyboardEvent("keydown", { key: "b", metaKey: true, bubbles: true }));
}

describe("initSidebarCollapse", () => {
  it("starts expanded by default", () => {
    const { nav, button } = setup();
    expect(nav.classList.contains("collapsed")).toBe(false);
    expect(button.getAttribute("aria-expanded")).toBe("true");
  });

  it("restores the collapsed state from storage", () => {
    const nav = document.createElement("aside");
    const button = document.createElement("button");
    document.body.append(nav, button);
    initSidebarCollapse(nav, button, { getItem: () => "1", setItem: () => {} });
    expect(nav.classList.contains("collapsed")).toBe(true);
    expect(button.classList.contains("collapsed")).toBe(true);
    expect(button.getAttribute("aria-expanded")).toBe("false");
  });

  it("toggles and persists via the rail button", () => {
    const { nav, button, storage } = setup();
    button.click();
    expect(nav.classList.contains("collapsed")).toBe(true);
    expect(storage.setItem).toHaveBeenCalledWith("docswiki.navCollapsed", "1");
    button.click();
    expect(nav.classList.contains("collapsed")).toBe(false);
    expect(storage.setItem).toHaveBeenLastCalledWith("docswiki.navCollapsed", "0");
  });

  it("toggles on Cmd/Ctrl+B", () => {
    const { nav } = setup();
    pressCmdB();
    expect(nav.classList.contains("collapsed")).toBe(true);
    document.dispatchEvent(new KeyboardEvent("keydown", { key: "b", ctrlKey: true, bubbles: true }));
    expect(nav.classList.contains("collapsed")).toBe(false);
  });

  it("ignores Cmd+B while typing in an input", () => {
    const { nav } = setup();
    const input = document.createElement("input");
    document.body.appendChild(input);
    input.focus();
    // jsdom focuses the node; the handler checks event.target
    input.dispatchEvent(
      new KeyboardEvent("keydown", { key: "b", metaKey: true, bubbles: true }),
    );
    expect(nav.classList.contains("collapsed")).toBe(false);
  });
});
