// Link handling inside rendered markdown articles. A raw `<a href>` click
// would navigate the Tauri webview itself — killing the SPA — so we intercept
// clicks and route them by target kind:
// - In-page anchors (`#section`) scroll to the heading (heading ids are
//   assigned by `assignHeadingIds`, called from content-view after render).
// - Relative `.md` links navigate to another doc in the same source.
// - Absolute links (http/https/mailto) open in the system browser via the
//   opener plugin — the webview can't navigate to external hosts anyway.
import { openUrl } from "@tauri-apps/plugin-opener";

export interface DocLinkHooks {
  /** The doc currently on screen; relative links resolve against it. */
  currentDocId: () => string | null;
  /** Map a resolved candidate id to the canonical id of a doc that exists in
   * the current source (implementations fall back to case-insensitive
   * matching, since docs are often authored on case-insensitive filesystems),
   * or null when no such doc exists. */
  resolveKnownId: (docId: string) => string | null;
  /** Suffix-matching retry for absolute route-style links whose candidate id
   * matched no doc verbatim — doc-site generators link by site route
   * (Docusaurus: `/docs/configuration/yazi`), which shares only its tail
   * segments with our doc ids (`configuration/yazi`). */
  resolveRouteSuffix: (docId: string) => string | null;
  /** Navigate to a doc within the same source. `fragment` may be "". */
  navigate: (docId: string, fragment: string) => void;
}

export function initDocLinks(contentEl: HTMLElement, hooks: DocLinkHooks): void {
  contentEl.addEventListener("click", (event) => {
    const target = event.target;
    if (!(target instanceof Element)) return;
    const anchor = target.closest("a");
    if (!anchor) return;

    const href = anchor.getAttribute("href")?.trim();
    if (!href) return;

    // In-page anchor: scroll instead of letting the browser "navigate".
    if (href.startsWith("#")) {
      event.preventDefault();
      scrollToFragment(contentEl, href.slice(1));
      return;
    }

    // Anything with a scheme (or scheme-relative `//host/...`) is external.
    if (/^[a-zA-Z][a-zA-Z0-9+.-]*:/.test(href) || href.startsWith("//")) {
      event.preventDefault();
      if (/^https?:\/\//i.test(href) || href.startsWith("//") || /^mailto:/i.test(href)) {
        void openUrl(href.startsWith("//") ? `https:${href}` : href);
      }
      return;
    }

    // Everything else is treated as a link to another doc in the source.
    event.preventDefault();
    const currentDocId = hooks.currentDocId();
    if (!currentDocId) return;

    const { path, fragment } = splitFragment(href);
    const docId = resolveDocPath(currentDocId, path);
    if (!docId) return;

    // Exact (case-insensitive) id match first; absolute links that miss —
    // doc-site route style, e.g. Docusaurus `/docs/configuration/yazi` — get
    // a suffix-matching retry.
    const known = hooks.resolveKnownId(docId) ?? (href.startsWith("/") ? hooks.resolveRouteSuffix(docId) : null);
    if (!known) {
      console.warn(`doc link target not found in current source: '${href}' (resolved to '${docId}')`);
      return;
    }
    hooks.navigate(known, fragment);
  });
}

/** Scroll the content pane to the element whose id matches the fragment
 * (percent-decoded, like browsers do). An empty fragment scrolls to top. */
export function scrollToFragment(contentEl: HTMLElement, fragment: string): void {
  const decoded = tryDecode(fragment);
  if (decoded === "") {
    contentEl.scrollTo({ top: 0, behavior: "smooth" });
    return;
  }
  // Fall back to a slugified lookup: link fragments follow the *source*
  // site's id scheme (e.g. `#mgr.sort_by`), while our heading ids are slugs
  // (`mgrsort_by`) — the same slug rule assignHeadingIds applies.
  const target =
    contentEl.querySelector(`#${CSS.escape(decoded)}`) ??
    contentEl.querySelector(`#${CSS.escape(slugify(decoded))}`);
  target?.scrollIntoView({ behavior: "smooth", block: "start" });
}

/** Assign GitHub-style ids to headings so in-page `#anchor` links work —
 * marked stopped generating heading ids years ago. Idempotent per render
 * (the article is rebuilt on every show, and explicit ids are preserved). */
export function assignHeadingIds(article: HTMLElement): void {
  const used = new Map<string, number>();
  for (const heading of article.querySelectorAll<HTMLHeadingElement>("h1, h2, h3, h4, h5, h6")) {
    if (heading.id) continue; // keep ids the markdown set explicitly
    const slug = slugify(heading.textContent ?? "");
    if (!slug) continue;
    const count = used.get(slug) ?? 0;
    used.set(slug, count + 1);
    heading.id = count === 0 ? slug : `${slug}-${count}`;
  }
}

/** Resolve a markdown link path against the doc it appears in, returning a
 * candidate doc id (extensionless, `/`-separated). A leading `/` anchors at
 * the source root; anything else resolves against the current doc's
 * directory. Returns null if the path escapes the source root. */
export function resolveDocPath(currentDocId: string, hrefPath: string): string | null {
  let path = hrefPath.replace(/\.(md|markdown)$/i, "");
  if (!path.startsWith("/")) {
    const lastSlash = currentDocId.lastIndexOf("/");
    const dir = lastSlash === -1 ? "" : currentDocId.slice(0, lastSlash);
    path = dir ? `${dir}/${path}` : path;
  }

  const segments: string[] = [];
  for (const raw of path.split("/")) {
    if (raw === "" || raw === ".") continue;
    if (raw === "..") {
      if (segments.length === 0) return null; // escapes the source root
      segments.pop();
      continue;
    }
    segments.push(tryDecode(raw));
  }
  return segments.length > 0 ? segments.join("/") : null;
}

function splitFragment(href: string): { path: string; fragment: string } {
  const hashIndex = href.indexOf("#");
  if (hashIndex === -1) return { path: href, fragment: "" };
  return { path: href.slice(0, hashIndex), fragment: href.slice(hashIndex + 1) };
}

function tryDecode(value: string): string {
  try {
    return decodeURIComponent(value);
  } catch {
    return value; // malformed percent-encoding; keep as-is
  }
}

/** Lowercase, drop punctuation (keeping `-`/`_`), spaces to hyphens — close
 * to GitHub's heading-slug algorithm, and keeps CJK characters intact. */
function slugify(text: string): string {
  return text
    .toLowerCase()
    .trim()
    .replace(/[^\p{L}\p{N}\s_-]+/gu, "")
    .replace(/\s+/g, "-");
}
