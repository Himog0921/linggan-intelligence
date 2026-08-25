import assert from "node:assert/strict";
import test from "node:test";

import { inspectXhsVisibleSearchSurface } from "../src/xhs-visible-search-adapter.js";

function selectedSort() {
  return { textContent: "综合" };
}

function card({ id, top, left, title = "", creator = "", published = "", cover = "" }) {
  return {
    getBoundingClientRect() {
      return { top, left };
    },
    querySelector(selector) {
      if (selector === "a.cover") {
        return id ? { getAttribute: (name) => name === "href" ? `/search_result/${id}?xsec_token=opaque` : null } : null;
      }
      if (selector === ".title") return title ? { textContent: title } : null;
      if (selector === ".footer span") return null;
      if ([".author-wrapper", ".author", ".footer .name"].includes(selector)) return creator ? { textContent: creator } : null;
      if ([".date", ".time", ".corner-tag"].includes(selector)) return published ? { textContent: published } : null;
      if (selector === "a.cover img, img") {
        return cover ? { currentSrc: cover, getAttribute: () => "" } : null;
      }
      return null;
    }
  };
}

function withSearchSurface({ href = "https://www.xiaohongshu.com/search_result?keyword=ADHD", sort = true, cards = [] }, run) {
  const originalWindow = globalThis.window;
  const originalDocument = globalThis.document;
  globalThis.window = { location: { href } };
  globalThis.document = {
    querySelectorAll(selector) {
      return selector.includes("aria-selected") ? (sort ? [selectedSort()] : []) : [];
    },
    querySelector(selector) {
      if (selector !== ".feeds-container") return null;
      return { querySelectorAll: (inner) => inner === "section" ? cards : [] };
    }
  };
  try {
    return run();
  } finally {
    globalThis.window = originalWindow;
    globalThis.document = originalDocument;
  }
}

test("adapted XHS adapter preserves visual card order, deduplicates and does not invent optional fields", () => {
  const result = withSearchSurface({
    cards: [
      card({ id: "note-b", top: 100, left: 320, title: "第二张", creator: "作者 B", cover: "https://observed.invalid/b.jpg" }),
      card({ id: "note-a", top: 100, left: 20, title: "第一张", creator: "作者 A", published: "3小时前" }),
      card({ id: "note-a", top: 220, left: 20, title: "重复卡" }),
      card({ id: "", top: 220, left: 320, title: "没有稳定身份" })
    ]
  }, inspectXhsVisibleSearchSurface);

  assert.equal(result.status, "ready");
  assert.equal(result.stoppedReason, "unknown");
  assert.deepEqual(result.cards.map((item) => item.platformContentId), ["note-a", "note-b"]);
  assert.equal(result.cards[0].resultPosition, 1);
  assert.equal(result.cards[0].publishedAtSourceText, "3小时前");
  assert.equal("coverCandidate" in result.cards[0], false);
  assert.equal(result.cards[1].coverCandidate.observedExternalUri, "https://observed.invalid/b.jpg");
});

test("adapter refuses fixed query and unverifiable sort before it emits a card", () => {
  const wrongQuery = withSearchSurface({ href: "https://www.xiaohongshu.com/search_result?keyword=A%E5%A8%83", cards: [] }, inspectXhsVisibleSearchSurface);
  assert.deepEqual(wrongQuery.status, "not_ready");
  assert.equal(wrongQuery.code, "fixed_query_mismatch");

  const unknownSort = withSearchSurface({ sort: false, cards: [] }, inspectXhsVisibleSearchSurface);
  assert.deepEqual(unknownSort.status, "not_ready");
  assert.equal(unknownSort.code, "sort_not_observable");
});

test("adapter calls quota reached only when exactly twenty stable cards are emitted", () => {
  const cards = Array.from({ length: 22 }, (_, index) => card({
    id: `note${String(index).padStart(20, "0")}`,
    top: index * 80,
    left: 20
  }));
  const result = withSearchSurface({ cards }, inspectXhsVisibleSearchSurface);
  assert.equal(result.cards.length, 20);
  assert.equal(result.cards.at(-1).resultPosition, 20);
  assert.equal(result.stoppedReason, "quota_reached");
});
