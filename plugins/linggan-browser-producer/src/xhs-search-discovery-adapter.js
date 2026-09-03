export const XHS_SEARCH_DISCOVERY_ADAPTER_VERSION = "xhs-visible-search-card-adapter.v1";

/**
 * This function is deliberately self-contained because Chrome serializes it into the current
 * tab after a user gesture. It only reads rendered DOM state and never scrolls, fetches, opens a
 * detail page, or examines browser credentials.
 */
export function collectVisibleXhsSearchDiscovery(input) {
  const page = input ?? { href: globalThis.location?.href, document: globalThis.document };
  const href = typeof page.href === "string" ? page.href : "";
  const document = page.document;

  const fail = (code, detail, extras = {}) => ({
    ok: false,
    adapterVersion: "xhs-visible-search-card-adapter.v1",
    code,
    detail,
    ...extras
  });

  if (!document || typeof document.querySelectorAll !== "function") {
    return fail("NOT_READY_DOCUMENT_UNAVAILABLE", "当前标签页无法读取可见搜索卡片。");
  }

  let url;
  try {
    url = new URL(href);
  } catch {
    return fail("NOT_READY_XHS_SEARCH_PAGE_REQUIRED", "请在小红书 ADHD 搜索结果页打开插件。");
  }

  if (
    !["www.xiaohongshu.com", "xiaohongshu.com"].includes(url.hostname) ||
    url.pathname !== "/search_result"
  ) {
    return fail("NOT_READY_XHS_SEARCH_PAGE_REQUIRED", "请在小红书 ADHD 搜索结果页打开插件。");
  }
  if (url.searchParams.get("keyword") !== "ADHD") {
    return fail("NOT_READY_FIXED_ADHD_QUERY_REQUIRED", "首批采集只允许关键词 ADHD；当前页面不是该固定搜索。");
  }

  const visible = (element) => {
    if (!element || typeof element.getBoundingClientRect !== "function") {
      return false;
    }
    const rect = element.getBoundingClientRect();
    const viewportWidth = globalThis.innerWidth ?? 0;
    const viewportHeight = globalThis.innerHeight ?? 0;
    return rect.width > 0 && rect.height > 0 && rect.bottom > 0 && rect.right > 0 && rect.top < viewportHeight && rect.left < viewportWidth;
  };
  const directText = (element) => {
    const value = typeof element?.textContent === "string" ? element.textContent.replace(/\s+/g, " ").trim() : "";
    return value === "" ? undefined : value;
  };
  const textFrom = (root, selectors) => {
    for (const selector of selectors) {
      const candidate = root.querySelector?.(selector);
      if (visible(candidate)) {
        const value = directText(candidate);
        if (value) return value;
      }
    }
    return undefined;
  };
  const comprehensiveSelected = [...document.querySelectorAll("[role='tab'][aria-selected='true'], [aria-current='page'], .active, .selected")]
    .some((element) => visible(element) && directText(element) === "综合");
  if (!comprehensiveSelected) {
    return fail(
      "NOT_READY_COMPREHENSIVE_SORT_UNVERIFIED",
      "无法确认当前搜索页选中“综合”排序；未读取或提交任何卡片。"
    );
  }

  const contentIdFrom = (anchor) => {
    const rawHref = typeof anchor?.href === "string" ? anchor.href : anchor?.getAttribute?.("href");
    if (typeof rawHref !== "string" || rawHref.trim() === "") return undefined;
    let target;
    try {
      target = new URL(rawHref, href);
    } catch {
      return undefined;
    }
    const match = /^\/(?:explore|discovery\/item)\/([A-Za-z0-9_-]+)(?:\/|$)/.exec(target.pathname);
    return match?.[1];
  };
  const cardRootFor = (anchor) =>
    anchor.closest?.("[data-note-id], article, section, .note-item, .feeds-container > div") ?? anchor.parentElement ?? anchor;
  const cards = [];
  const seenContentIds = new Set();
  let skippedWithoutStableId = 0;

  for (const anchor of document.querySelectorAll("a[href]")) {
    const root = cardRootFor(anchor);
    if (!visible(root)) continue;
    const platformContentId = contentIdFrom(anchor);
    if (!platformContentId) {
      if (/\/(?:explore|discovery\/item)\//.test(anchor.href ?? "")) skippedWithoutStableId += 1;
      continue;
    }
    if (seenContentIds.has(platformContentId)) continue;
    seenContentIds.add(platformContentId);
    if (cards.length === 20) break;

    const title = textFrom(root, ["[class*='title']", "a.title", "[data-note-title]"]) ?? directText(anchor);
    const creatorDisplayName = textFrom(root, ["a[href*='/user/profile/']", "[class*='author']", "[class*='user-name']"]);
    const publishedAtSourceText = textFrom(root, ["[class*='time']", "[class*='date']", "[data-published-at]"]);
    const image = root.querySelector?.("img");
    const coverUrl = image?.currentSrc || image?.src;
    const coverCandidate = (() => {
      try {
        const parsed = new URL(coverUrl, href);
        return ["http:", "https:"].includes(parsed.protocol)
          ? { observedExternalUri: parsed.toString() }
          : undefined;
      } catch {
        return undefined;
      }
    })();

    cards.push({
      platformContentId,
      ...(title ? { title } : {}),
      ...(creatorDisplayName ? { creatorDisplayName } : {}),
      ...(publishedAtSourceText ? { publishedAtSourceText } : {}),
      ...(coverCandidate ? { coverCandidate } : {}),
      resultPosition: cards.length + 1
    });
  }

  if (cards.length === 0) {
    return fail(
      "NOT_READY_VISIBLE_CARD_SELECTOR_UNAVAILABLE",
      "没有找到带稳定内容编号的当前可见搜索卡片；未提交任何数据。",
      { skippedWithoutStableId }
    );
  }

  return {
    ok: true,
    adapterVersion: "xhs-visible-search-card-adapter.v1",
    observedAt: new Date().toISOString(),
    cards,
    skippedWithoutStableId,
    stoppedReason: cards.length === 20 ? "quota_reached" : "manual_stop"
  };
}
