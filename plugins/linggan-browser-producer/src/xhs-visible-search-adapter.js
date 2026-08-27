/**
 * PLUGIN-MIGRATION-001 active adapter.
 *
 * Adapted from `Himog0921/linggan-boom@8a00cc1`, specifically its XHS
 * `discoverNotesFromDOM` surface-card rules and the 2026-06-01 XHS search-page
 * field survey. This is intentionally a smaller, Linggan-owned adapter: it never
 * scrolls, opens a detail, reads hidden state, calls a platform API, or reads an
 * account/cookie. The function is self-contained because Chrome serializes it
 * into the user-selected active tab through `chrome.scripting.executeScript`.
 */
export function inspectXhsVisibleSearchSurface() {
  const NOT_READY = (code, detail) => ({ status: "not_ready", code, detail });
  const firstText = (value) => (typeof value === "string" ? value.trim() : "");
  const textFrom = (element) => firstText(element?.textContent || "");
  const isXhsSearchPage = () => {
    try {
      const current = new URL(window.location.href);
      return /(^|\.)xiaohongshu\.com$/i.test(current.hostname) && /\/search_result\/?$/i.test(current.pathname);
    } catch {
      return false;
    }
  };
  const currentKeyword = () => {
    try {
      return firstText(new URL(window.location.href).searchParams.get("keyword") || "");
    } catch {
      return "";
    }
  };
  const selectedComprehensiveSort = () => {
    const candidates = [...document.querySelectorAll("[aria-selected='true'], [data-active='true'], .active, .selected")];
    return candidates.some((element) => textFrom(element) === "综合");
  };
  const extractPlatformContentId = (href) => {
    try {
      const url = new URL(href, window.location.href);
      const explore = /\/(?:explore|discovery\/item|search_result)\/([a-z0-9_-]+)/i.exec(url.pathname);
      if (explore?.[1]) return explore[1];
      const profileDetail = /\/user\/profile\/[a-z0-9_-]+\/([a-z0-9_-]+)/i.exec(url.pathname);
      return profileDetail?.[1] || "";
    } catch {
      return "";
    }
  };
  const imageUrl = (section) => {
    const image = section.querySelector("a.cover img, img");
    const candidate = firstText(image?.currentSrc) || firstText(image?.getAttribute("src")) || firstText(image?.getAttribute("data-src"));
    return candidate || "";
  };
  const cardText = (section, selectors) => {
    for (const selector of selectors) {
      const value = textFrom(section.querySelector(selector));
      if (value) return value;
    }
    return "";
  };

  if (!isXhsSearchPage()) {
    return NOT_READY("wrong_page", "请先打开小红书的搜索结果页。");
  }
  if (currentKeyword() !== "ADHD") {
    return NOT_READY("fixed_query_mismatch", "本次仅允许关键词 ADHD；请在 ADHD 搜索结果页重新打开插件。");
  }
  if (!selectedComprehensiveSort()) {
    return NOT_READY("sort_not_observable", "未能从当前页面确认已选综合排序；请明确选择“综合”后重试。");
  }

  const feed = document.querySelector(".feeds-container");
  if (!feed) {
    return NOT_READY("search_surface_unavailable", "未找到当前搜索结果卡片区域；请等待结果加载完成后重试。");
  }

  const seen = new Set();
  const candidates = [];
  for (const section of feed.querySelectorAll("section")) {
    const coverLink = section.querySelector("a.cover");
    const href = firstText(coverLink?.getAttribute("href"));
    const platformContentId = extractPlatformContentId(href);
    if (!platformContentId || seen.has(platformContentId)) continue;
    seen.add(platformContentId);

    const title = cardText(section, [".title", ".footer span"]);
    const creatorDisplayName = cardText(section, [".author-wrapper", ".author", ".footer .name"]);
    const publishedAtSourceText = cardText(section, [".date", ".time", ".corner-tag"]);
    const cover = imageUrl(section);
    const rect = section.getBoundingClientRect();
    candidates.push({
      platformContentId,
      ...(title ? { title } : {}),
      ...(creatorDisplayName ? { creatorDisplayName } : {}),
      ...(publishedAtSourceText ? { publishedAtSourceText } : {}),
      ...(cover ? { coverCandidate: { observedExternalUri: cover } } : {}),
      _top: Number(rect.top) || 0,
      _left: Number(rect.left) || 0
    });
  }

  candidates.sort((left, right) => {
    const rowDifference = Math.abs(left._top - right._top);
    return rowDifference < 50 ? left._left - right._left : left._top - right._top;
  });

  const cards = candidates.slice(0, 20).map(({ _top, _left, ...card }, index) => ({
    ...card,
    resultPosition: index + 1
  }));

  return {
    status: "ready",
    cards,
    stoppedReason: cards.length === 20 ? "quota_reached" : "unknown"
  };
}
