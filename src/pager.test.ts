// @vitest-environment jsdom
// Tests the bottom prev/next pager's visible behavior.
import { describe, expect, it, vi } from "vitest";
import { appendPager } from "./pager";

function container(): HTMLElement {
  const el = document.createElement("div");
  document.body.appendChild(el);
  return el;
}

function buttons(el: HTMLElement): HTMLButtonElement[] {
  return Array.from(el.querySelectorAll(".doc-pager button"));
}

describe("appendPager", () => {
  it("renders both links with titles in order", () => {
    const el = container();
    appendPager(
      el,
      { docId: "a.md", title: "上一篇标题" },
      { docId: "c.md", title: "下一篇标题" },
      () => {},
    );
    const [prev, next] = buttons(el);
    expect(prev.textContent).toContain("上一篇标题");
    expect(next.textContent).toContain("下一篇标题");
  });

  it("omits the missing side at the ends of the tree", () => {
    const el = container();
    appendPager(el, null, { docId: "c.md", title: "下一篇" }, () => {});
    const [only] = buttons(el);
    expect(buttons(el)).toHaveLength(1);
    expect(only.textContent).toContain("下一篇");
  });

  it("renders nothing when there is no prev and no next", () => {
    const el = container();
    appendPager(el, null, null, () => {});
    expect(el.querySelector(".doc-pager")).toBeNull();
  });

  it("invokes onSelect with the target docId on click", () => {
    const el = container();
    const onSelect = vi.fn();
    appendPager(el, { docId: "a.md", title: "A" }, { docId: "c.md", title: "C" }, onSelect);
    buttons(el)[1].click();
    expect(onSelect).toHaveBeenCalledWith("c.md");
  });

  it("replaces a previous pager instead of stacking", () => {
    const el = container();
    appendPager(el, null, { docId: "b.md", title: "B" }, () => {});
    appendPager(el, { docId: "a.md", title: "A" }, null, () => {});
    expect(el.querySelectorAll(".doc-pager")).toHaveLength(1);
    expect(buttons(el)[0].textContent).toContain("A");
  });
});
