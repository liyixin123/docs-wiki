// @vitest-environment jsdom
// Tests rewriting doc images that reference site-root asset paths
// (`/assets/...`) into loadable data URLs after markdown rendering.
import { describe, it, expect, vi } from "vitest";
import { resolveAssetImages } from "./asset-images";

function domWith(html: string) {
  const el = document.createElement("div");
  el.innerHTML = html;
  return el;
}

describe("resolveAssetImages", () => {
  it("replaces /assets/ img srcs with the resolved data URL for the given source", async () => {
    const el = domWith(
      `<img src="/assets/pic.png" alt="a"><img src="/assets/nested/deep.jpg" alt="b">`
    );
    const fetcher = vi
      .fn()
      .mockResolvedValueOnce("data:image/png;base64,QUJD")
      .mockResolvedValueOnce("data:image/jpeg;base64,REVG");

    await resolveAssetImages(el, "shape-up", fetcher);

    const imgs = [...el.querySelectorAll("img")];
    expect(imgs[0].getAttribute("src")).toBe("data:image/png;base64,QUJD");
    expect(imgs[1].getAttribute("src")).toBe("data:image/jpeg;base64,REVG");
    expect(fetcher).toHaveBeenCalledWith("shape-up", "assets/pic.png");
    expect(fetcher).toHaveBeenCalledWith("shape-up", "assets/nested/deep.jpg");
  });

  it("leaves non-/assets/ images and other elements untouched", async () => {
    const el = domWith(
      `<img src="https://example.com/x.png"><img src="relative.png"><a href="/assets/other.md">link</a>`
    );
    const fetcher = vi.fn();

    await resolveAssetImages(el, "s", fetcher);

    expect(fetcher).not.toHaveBeenCalled();
    expect(el.querySelector("img[src='https://example.com/x.png']")).toBeTruthy();
    expect(el.querySelector("a")!.getAttribute("href")).toBe("/assets/other.md");
  });

  it("hides an image whose asset cannot be loaded instead of leaving a broken src", async () => {
    const el = domWith(`<img src="/assets/missing.png" alt="gone">`);
    const fetcher = vi.fn().mockRejectedValue(new Error("not found"));

    await resolveAssetImages(el, "s", fetcher);

    const img = el.querySelector("img")!;
    expect(img.getAttribute("src")).toBe("/assets/missing.png");
    expect(img.style.display).toBe("none");
  });
});
