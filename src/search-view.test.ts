// @vitest-environment jsdom
import { describe, expect, it } from "vitest";
import { renderSearchResults } from "./search-view";
import type { SearchHit, Snippet } from "./api";

// Snippet highlight ranges come from the Rust backend as UTF-16 code unit
// offsets. This locks down that the view wraps exactly the query term in
// <mark> when CJK text precedes it (regression: byte offsets drifted 3x).
function zhSnippet(termOffset: number, termLen: number): Snippet {
  return {
    text: "这里详细介绍小工具和配件的使用方法",
    highlightRanges: [[termOffset, termOffset + termLen]],
  };
}

function makeHit(snippets: Snippet[]): SearchHit {
  return {
    sourceId: "demo",
    docId: "alpha",
    lang: "zh",
    title: "Alpha 指南",
    score: 1,
    snippets,
  };
}

describe("renderSearchResults highlighting", () => {
  it("marks exactly the matched term at UTF-16 offsets", () => {
    const container = document.createElement("div");
    // "小工具" starts at UTF-16 index 6; the backend used to send byte
    // offsets (would be 18) which highlighted the wrong characters.
    renderSearchResults(container, "小工具", [makeHit([zhSnippet(6, 3)])], new Map(), () => {});

    const marks = container.querySelectorAll("mark");
    expect(marks.length).toBe(1);
    expect(marks[0].textContent).toBe("小工具");
  });
});
