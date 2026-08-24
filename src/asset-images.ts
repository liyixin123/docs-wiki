// Doc markdown references images either at the imported site's root
// (`/assets/pic.png`) or relative to the doc itself (`images/arch.png`,
// `../diagrams/flow.svg`). Those files were copied next to the source's
// docs on import; the backend `get_asset_data` command serves each as a
// data URL. This module rewrites rendered <img> elements to those data
// URLs, resolving relative references against the current doc's directory.
//
// Pure DOM + an injected fetcher (no `api` import) so it stays testable in
// isolation — the fetcher is wired up in main.ts.

/** Rewrite every in-repo img src under `root` to the data URL returned by
 * `fetchAssetUrl(sourceId, path)`. External URLs and data URLs are left
 * untouched; images that fail to resolve are hidden rather than left with
 * a broken path. `docId` is the doc's slash-separated id (`guides/intro`),
 * used as the base directory for relative srcs. */
export async function resolveAssetImages(
  root: HTMLElement,
  sourceId: string,
  docId: string,
  fetchAssetUrl: (sourceId: string, path: string) => Promise<string>
): Promise<void> {
  const docDir = docId.includes("/") ? docId.slice(0, docId.lastIndexOf("/") + 1) : "";

  const imgs = [...root.querySelectorAll<HTMLImageElement>("img")].filter((img) => {
    const src = img.getAttribute("src") ?? "";
    return src !== "" && !/^(https?:|data:)/.test(src);
  });

  await Promise.all(
    imgs.map(async (img) => {
      const src = img.getAttribute("src")!;
      try {
        img.src = await fetchAssetUrl(sourceId, resolveAssetPath(docDir, src));
      } catch {
        // Missing or unreadable asset: hide instead of showing a broken icon.
        img.style.display = "none";
      }
    })
  );
}

/** Resolve an img src against the doc's directory into a path relative to
 * the source root: `./` and `../` segments are collapsed, a leading `/`
 * means the source root. */
function resolveAssetPath(docDir: string, src: string): string {
  const base = src.startsWith("/") ? "" : docDir;
  const segments: string[] = [];
  for (const seg of (base + src).split("/")) {
    if (seg === "" || seg === ".") continue;
    if (seg === "..") {
      segments.pop(); // escaping the root just clamps to it
    } else {
      segments.push(seg);
    }
  }
  return segments.join("/");
}
