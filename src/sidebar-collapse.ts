// Sidebar collapse: a persistent left icon rail whose ☰ button toggles the
// nav sidebar. The collapsed state is remembered in localStorage and the
// Cmd/Ctrl+B shortcut toggles it too. Pure DOM, so it can be tested standalone.

const STORAGE_KEY = "docswiki.navCollapsed";

export interface SidebarCollapse {
  toggle(): void;
}

export function initSidebarCollapse(
  nav: HTMLElement,
  toggleButton: HTMLButtonElement,
  storage: Pick<Storage, "getItem" | "setItem"> = localStorage,
): SidebarCollapse {
  let collapsed = storage.getItem(STORAGE_KEY) === "1";
  apply();

  toggleButton.addEventListener("click", toggle);

  document.addEventListener("keydown", (event) => {
    if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === "b" && !isTyping(event.target)) {
      event.preventDefault();
      toggle();
    }
  });

  function toggle(): void {
    collapsed = !collapsed;
    storage.setItem(STORAGE_KEY, collapsed ? "1" : "0");
    apply();
  }

  function apply(): void {
    nav.classList.toggle("collapsed", collapsed);
    toggleButton.classList.toggle("collapsed", collapsed);
    toggleButton.setAttribute("aria-expanded", String(!collapsed));
  }

  return { toggle };
}

export function isTyping(target: EventTarget | null): boolean {
  if (!(target instanceof HTMLElement)) return false;
  return (
    target.tagName === "INPUT" ||
    target.tagName === "TEXTAREA" ||
    target.tagName === "SELECT" ||
    target.isContentEditable
  );
}
