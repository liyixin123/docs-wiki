// Doc markdown imported from doc sites references images at the site root
// (`/assets/pic.png`). Those files were copied next to the source's docs on
// import; the backend `get_asset_data` command serves each as a data URL.
// This module rewrites rendered <img> elements to those data URLs.
//
// Pure DOM + an injected fetcher (no `api` import) so it stays testable in
// isolation — the fetcher is wired up in main.ts.

const ASSET_SRC_PREFIX = "/assets/";

/** Rewrite every `/assets/...` img src under `root` to the data URL returned
 * by `fetchAssetUrl(sourceId, path)`. Images that fail to resolve are hidden
 * rather than left with a broken path. */
export async function resolveAssetImages(
  root: HTMLElement,
  sourceId: string,
  fetchAssetUrl: (sourceId: string, path: string) => Promise<string>
): Promise<void> {
  const imgs = [
    ...root.querySelectorAll<HTMLImageElement>("img"),
  ].filter((img) => (img.getAttribute("src") ?? "").startsWith(ASSET_SRC_PREFIX));

  await Promise.all(
    imgs.map(async (img) => {
      const src = img.getAttribute("src")!;
      const path = src.replace(/^\//, "");
      try {
        img.src = await fetchAssetUrl(sourceId, path);
      } catch {
        // Missing or unreadable asset: hide instead of showing a broken icon.
        img.style.display = "none";
      }
    })
  );
}
