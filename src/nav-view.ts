// Renders the category/document sidebar from a NavTree and reports clicks
// (and drag-to-reorder moves) back to the caller — this module owns no
// state of its own beyond what's briefly needed mid-drag.
import type { NavTree } from "./api";

export function renderNav(
  container: HTMLElement,
  tree: NavTree,
  activeDocId: string | null,
  onSelect: (docId: string) => void,
  onReorder?: (orderedIds: string[]) => void,
  changedDocIds?: ReadonlySet<string>,
): void {
  container.replaceChildren();

  if (tree.categories.length === 0) {
    const empty = document.createElement("p");
    empty.className = "nav-empty";
    empty.textContent = "此来源暂无文档。";
    container.appendChild(empty);
    return;
  }

  for (const category of tree.categories) {
    const section = document.createElement("section");
    section.className = "nav-category";

    const heading = document.createElement("h3");
    heading.textContent = category.name;
    section.appendChild(heading);

    const list = document.createElement("ul");
    for (const item of category.items) {
      const li = document.createElement("li");
      li.dataset.docId = item.docId;

      if (onReorder) {
        li.draggable = true;
        const handle = document.createElement("span");
        handle.className = "drag-handle";
        handle.textContent = "⠿";
        handle.setAttribute("aria-hidden", "true");
        li.appendChild(handle);

        li.addEventListener("dragstart", () => li.classList.add("dragging"));
        li.addEventListener("dragend", () => {
          li.classList.remove("dragging");
          onReorder(flattenOrder(container));
        });
      }

      const button = document.createElement("button");
      button.type = "button";
      button.className = "nav-item";
      button.textContent = item.title;
      button.dataset.docId = item.docId;
      if (item.docId === activeDocId) {
        button.classList.add("active");
      }
      button.addEventListener("click", () => onSelect(item.docId));
      li.appendChild(button);

      if (changedDocIds?.has(item.docId)) {
        const badge = document.createElement("span");
        badge.className = "update-badge";
        badge.title = "上游内容已更新";
        badge.textContent = "●";
        li.appendChild(badge);
      }

      list.appendChild(li);
    }

    if (onReorder) {
      list.addEventListener("dragover", (event) => {
        event.preventDefault(); // required to allow a drop
        const dragging = container.querySelector<HTMLLIElement>("li.dragging");
        // Only allow reordering within the same category's list.
        if (!dragging || dragging.parentElement !== list) return;
        const afterElement = elementAfterDragPosition(list, event.clientY);
        if (afterElement == null) {
          list.appendChild(dragging);
        } else {
          list.insertBefore(dragging, afterElement);
        }
      });
    }

    section.appendChild(list);
    container.appendChild(section);
  }
}

/** Which existing item the dragged one should land before, based on the
 * pointer's vertical position — or `null` to mean "at the end". */
function elementAfterDragPosition(list: HTMLElement, y: number): HTMLElement | null {
  const items = Array.from(list.querySelectorAll<HTMLLIElement>("li:not(.dragging)"));
  let closest: { offset: number; element: HTMLElement | null } = { offset: Number.NEGATIVE_INFINITY, element: null };
  for (const child of items) {
    const box = child.getBoundingClientRect();
    const offset = y - box.top - box.height / 2;
    if (offset < 0 && offset > closest.offset) {
      closest = { offset, element: child };
    }
  }
  return closest.element;
}

function flattenOrder(container: HTMLElement): string[] {
  return Array.from(container.querySelectorAll<HTMLLIElement>("li[data-doc-id]")).map((li) => li.dataset.docId!);
}
