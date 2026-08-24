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

    await resolveAssetImages(el, "shape-up", "chapter-02", fetcher);

    const imgs = [...el.querySelectorAll("img")];
    expect(imgs[0].getAttribute("src")).toBe("data:image/png;base64,QUJD");
    expect(imgs[1].getAttribute("src")).toBe("data:image/jpeg;base64,REVG");
    expect(fetcher).toHaveBeenCalledWith("shape-up", "assets/pic.png");
    expect(fetcher).toHaveBeenCalledWith("shape-up", "assets/nested/deep.jpg");
  });

  it("resolves relative img srcs against the current doc's directory", async () => {
    const el = domWith(`<img src="images/arch.png" alt="a"><img src="./assets/x.jpg" alt="b">`);
    const fetcher = vi.fn().mockResolvedValue("data:image/png;base64,QQ==");

    await resolveAssetImages(el, "remote-src", "guides/getting-started", fetcher);

    expect(fetcher).toHaveBeenCalledWith("remote-src", "guides/images/arch.png");
    expect(fetcher).toHaveBeenCalledWith("remote-src", "guides/assets/x.jpg");
    const imgs = [...el.querySelectorAll("img")];
    expect(imgs.every((i) => (i.getAttribute("src") ?? "").startsWith("data:"))).toBe(true);
  });

  it("resolves ../ references up to the doc's parent directories", async () => {
    const el = domWith(`<img src="../diagrams/flow.svg" alt="a">`);
    const fetcher = vi.fn().mockResolvedValue("data:image/svg+xml;base64,QQ==");

    await resolveAssetImages(el, "s", "guides/advanced/tuning", fetcher);

    expect(fetcher).toHaveBeenCalledWith("s", "guides/diagrams/flow.svg");
  });

  it("leaves external and non-asset images untouched", async () => {
    const el = domWith(
      `<img src="https://example.com/x.png"><img src="data:image/png;base64,QQ=="><a href="/assets/other.md">link</a>`
    );
    const fetcher = vi.fn();

    await resolveAssetImages(el, "s", "doc", fetcher);

    expect(fetcher).not.toHaveBeenCalled();
    expect(el.querySelector("img[src='https://example.com/x.png']")).toBeTruthy();
    expect(el.querySelector("img[src='data:image/png;base64,QQ==']")).toBeTruthy();
    expect(el.querySelector("a")!.getAttribute("href")).toBe("assets/other.md".replace("assets/", "/assets/"));
  });

  it("hides an image whose asset cannot be loaded instead of leaving a broken src", async () => {
    const el = domWith(`<img src="assets/missing.png" alt="gone">`);
    const fetcher = vi.fn().mockRejectedValue(new Error("not found"));

    await resolveAssetImages(el, "s", "doc", fetcher);

    const img = el.querySelector("img")!;
    expect(img.getAttribute("src")).toBe("assets/missing.png");
    expect(img.style.display).toBe("none");
  });
});
