// Actual local Topic UI evidence. Writes and external origins are blocked, never mocked as success.
import { chromium } from "playwright";
import { mkdir, readFile, writeFile } from "node:fs/promises";
import { createHash } from "node:crypto";
import path from "node:path";
const origin = process.env.TOPICS_VISUAL_ORIGIN || "http://127.0.0.1:3000";
const dir = process.env.TOPICS_VISUAL_DIR;
if (!dir)
  throw new Error("TOPICS_VISUAL_DIR is required (private evidence only)");
await mkdir(path.join(dir, "responses"), { recursive: true });
const response = await fetch(`${origin}/api/local/topic-map/summary`);
if (!response.ok)
  throw new Error(`Cannot establish actual topic scope: ${response.status}`);
const snapshot = await response.json();
const pageHtml = await (await fetch(`${origin}/topics`)).text();
const initialDomain = /data-default-domain-ref="([^"]+)"/.exec(pageHtml)?.[1];
const domain =
  process.env.TOPICS_VISUAL_DOMAIN ||
  initialDomain ||
  snapshot.scope.domainRef ||
  snapshot.domains[0]?.domainRef;
if (!domain) throw new Error("Actual page has no established default domain");
const selected = snapshot.topics.find(
  (t) => t.workRefs.length > 0 && (!t.domainRef || t.domainRef === domain),
);
const report = {
  origin,
  revision: process.env.TOPICS_VISUAL_REVISION || null,
  time: new Date().toISOString(),
  screenshots: [],
  errors: [],
  blocked: [],
  failures: [],
  timings: [],
  reads: [],
};
const replayDir = process.env.TOPICS_VISUAL_REPLAY_DIR;
const readKey = (raw) => {
  const url = new URL(raw);
  url.searchParams.sort();
  return url.pathname + url.search;
};
const replay = new Map();
if (replayDir) {
  const recorded = JSON.parse(
    await readFile(path.join(replayDir, "report.json"), "utf8"),
  );
  for (const read of recorded.reads)
    if (!replay.has(readKey(read.url))) replay.set(readKey(read.url), read);
  report.inputMode =
    "unmodified actual GET bodies replayed for deterministic renderer comparison";
  report.inputSource = replayDir;
}
const browser = await chromium.launch({ headless: true });
const context = await browser.newContext({
  viewport: { width: 1440, height: 900 },
  deviceScaleFactor: 1,
  reducedMotion: "reduce",
});
const legacyEntry = process.env.TOPICS_VISUAL_LEGACY_ENTRY
  ? await readFile(process.env.TOPICS_VISUAL_LEGACY_ENTRY)
  : null;
await context.route("**/*", async (route) => {
  const req = route.request();
  if (
    new URL(req.url()).origin !== origin ||
    !["GET", "HEAD"].includes(req.method())
  ) {
    let action = null;
    try {
      action = req.postDataJSON()?.action || null;
    } catch {
      /* An invalid body is still blocked. */
    }
    report.blocked.push({ method: req.method(), url: req.url(), action });
    return route.abort();
  }
  if (legacyEntry && new URL(req.url()).pathname === "/topics")
    return route.fulfill({
      status: 200,
      headers: {
        "content-type": "text/html; charset=utf-8",
        "cache-control": "no-store",
      },
      body: legacyEntry,
    });
  if (
    replayDir &&
    new URL(req.url()).pathname.startsWith("/api/") &&
    !new URL(req.url()).pathname.startsWith("/api/local/media/")
  ) {
    const read = replay.get(readKey(req.url()));
    if (!read) {
      report.failures.push({
        message: "Actual GET recording missing",
        url: req.url(),
      });
      return route.abort();
    }
    const body = await readFile(
      path.join(replayDir, "responses", `${read.hash}.body`),
    );
    return route.fulfill({
      status: read.status,
      headers: {
        "content-type": read.contentType || "application/json",
        "cache-control": "no-store",
      },
      body,
    });
  }
  return route.continue();
});
const page = await context.newPage();
page.on("pageerror", (error) => report.errors.push(error.message));
const inflight = new Set();
let lastActivity = Date.now();
page.on("request", (request) => {
  if (new URL(request.url()).pathname.startsWith("/api/")) {
    inflight.add(request);
    lastActivity = Date.now();
  }
});
for (const event of ["requestfinished", "requestfailed"])
  page.on(event, (request) => {
    inflight.delete(request);
    lastActivity = Date.now();
  });
async function settle() {
  const deadline = Date.now() + 90000;
  while (inflight.size || Date.now() - lastActivity < 150) {
    if (Date.now() > deadline)
      throw new Error("Actual API reads did not settle");
    await page.waitForTimeout(25);
  }
  await Promise.allSettled([...pendingReads]);
}
const pendingReads = new Set();
page.on("response", (res) => {
  if (!new URL(res.url()).pathname.startsWith("/api/")) return;
  const pending = (async () => {
    try {
      const body = await res.body();
      const hash = createHash("sha256").update(body).digest("hex");
      await writeFile(path.join(dir, "responses", `${hash}.body`), body);
      report.reads.push({
        url: res.url(),
        status: res.status(),
        contentType: res.headers()["content-type"],
        hash,
      });
    } catch {
      /* Navigation cancellation may make a body unavailable; no fallback is invented. */
    }
  })();
  pendingReads.add(pending);
  pending.finally(() => pendingReads.delete(pending));
});
async function shot(name) {
  if (process.env.TOPICS_VISUAL_NESTED_ONLY === "1" && !name.includes("-nested")) return;
  await settle();
  await page.mouse.move(0, 0);
  await page.evaluate(() => document.fonts.ready);
  await page.evaluate(
    () =>
      new Promise((resolve) =>
        requestAnimationFrame(() => requestAnimationFrame(resolve)),
      ),
  );
  if (!/-y\d+$|-right$/.test(name))
    await writeFile(
      path.join(dir, `${name}.dom.json`),
      JSON.stringify(
        await page.evaluate(() => ({
          text: document.body.innerText,
          title: document.title,
          rail: [
            ...document.querySelectorAll(
              '[class*="judgment-rail"] p,[class*="rail-stats"] span,[class*="rail-stats"] b',
            ),
          ].map((el) => {
            const s = getComputedStyle(el),
              r = el.getBoundingClientRect();
            return {
              text: el.textContent,
              x: r.x,
              y: r.y,
              w: r.width,
              h: r.height,
              font: s.font,
              letterSpacing: s.letterSpacing,
              whiteSpace: s.whiteSpace,
              display: s.display,
              html: el.innerHTML,
            };
          }),
        })),
        null,
        2,
      ),
    );
  await page.screenshot({
    path: path.join(dir, `${name}.png`),
    fullPage: true,
    animations: "disabled",
  });
  report.screenshots.push({
    name,
    url: page.url(),
    viewport: page.viewportSize(),
    geometry: await page.evaluate(() => ({
      width: innerWidth,
      scrollWidth: document.documentElement.scrollWidth,
    })),
  });
}
function positions(length, viewport) {
  const end = Math.max(0, length - viewport);
  const values = [0];
  for (let value = Math.max(1, viewport - 60); value < end; value += Math.max(1, viewport - 60))
    values.push(value);
  if (end > 0) values.push(end);
  return values;
}
async function nestedScrollShots(name, surface) {
  await settle();
  await page.evaluate(() => document.fonts.ready);
  await page.evaluate(() => new Promise((resolve) =>
    requestAnimationFrame(() => requestAnimationFrame(resolve))));
  // Enumerate actual nested scroll containers, independent of CSS Module names.
  const nested = surface.locator("*");
  const indexes = await nested.evaluateAll((elements) => elements.flatMap((el, index) => {
    const style = getComputedStyle(el), rect = el.getBoundingClientRect();
    return rect.width > 0 && rect.height > 0 && (
      (/auto|scroll/.test(style.overflowX) && el.scrollWidth > el.clientWidth + 1) ||
      (/auto|scroll/.test(style.overflowY) && el.scrollHeight > el.clientHeight + 1)
    ) ? [index] : [];
  }));
  const parentSize = await surface.evaluate((el) => ({ h: el.scrollHeight, ch: el.clientHeight }));
  for (const [number, index] of indexes.entries()) {
    const child = nested.nth(index);
    const size = await child.evaluate((el) => ({
      h: el.scrollHeight, ch: el.clientHeight, w: el.scrollWidth, cw: el.clientWidth,
    }));
    for (const parentY of positions(parentSize.h, parentSize.ch)) {
      await surface.evaluate((el, y) => { el.scrollTop = y; }, parentY);
      const visible = await child.evaluate((el) => {
        const rect = el.getBoundingClientRect();
        let top = Math.max(0, rect.top), bottom = Math.min(innerHeight, rect.bottom);
        for (let parent = el.parentElement; parent; parent = parent.parentElement) {
          if (/auto|scroll|hidden/.test(getComputedStyle(parent).overflowY)) {
            const box = parent.getBoundingClientRect();
            top = Math.max(top, box.top); bottom = Math.min(bottom, box.bottom);
          }
        }
        return bottom > top;
      });
      if (!visible) continue;
      for (const y of positions(size.h, size.ch)) for (const x of positions(size.w, size.cw)) {
        await child.evaluate((el, point) => { el.scrollTop = point.y; el.scrollLeft = point.x; }, { x, y });
        await shot(`${name}-nested${number}-p${parentY}-x${x}-y${y}`);
      }
    }
    await child.evaluate((el) => { el.scrollTop = 0; el.scrollLeft = 0; });
  }
  await surface.evaluate((el) => { el.scrollTop = 0; el.scrollLeft = 0; });
}
async function scrollShots(name, selector) {
  if (process.env.TOPICS_VISUAL_SCROLL === "0") return;
  const el = page.locator(selector).first();
  if (!(await el.count())) return;
  if (process.env.TOPICS_VISUAL_NESTED_ONLY === "1") {
    await nestedScrollShots(name, el);
    return;
  }
  const size = await el.evaluate((el) => ({
    h: el.scrollHeight,
    ch: el.clientHeight,
    w: el.scrollWidth,
    cw: el.clientWidth,
  }));
  // Every visible page of the surface is captured; no cropped/masked comparison.
  for (
    let y = Math.max(1, size.ch - 60);
    y < size.h;
    y += Math.max(1, size.ch - 60)
  ) {
    await el.evaluate((el, y) => {
      el.scrollTop = y;
    }, y);
    await shot(`${name}-y${y}`);
  }
  await el.evaluate((el) => {
    el.scrollTop = 0;
    el.scrollLeft = el.scrollWidth;
  });
  if (size.w > size.cw) await shot(`${name}-right`);
  await el.evaluate((el) => {
    el.scrollLeft = 0;
  });
}
const base = `${origin}/topics?tm=1&domainRef=${encodeURIComponent(domain)}`;
async function open(suffix = "") {
  const start = performance.now();
  await page.goto(base + suffix, { waitUntil: "networkidle" });
  await page
    .locator('[data-loaded="true"][aria-label="主题内容"]')
    .waitFor({ state: "visible", timeout: 90000 });
  await page
    .getByText(/正在载入其余材料与讨论/)
    .waitFor({ state: "hidden", timeout: 90000 });
  await page.waitForLoadState("networkidle");
  report.timings.push({
    url: page.url(),
    milliseconds: performance.now() - start,
  });
}
async function action(action, id) {
  let locator = page.locator(`[data-action="${action}"]`);
  if (id !== undefined)
    locator = locator.and(page.locator(`[data-id="${id}"]`));
  await locator.first().click();
  await page.waitForLoadState("networkidle");
}
try {
  const widths = (process.env.TOPICS_VISUAL_WIDTHS || "1440,1280,1024,390")
    .split(",")
    .map(Number);
  for (const width of widths) {
    await page.setViewportSize({ width, height: width === 390 ? 844 : 900 });
    await open();
    await shot(`${width}-overview`);
    await scrollShots(`${width}-overview`, ".lgi-tm-main, [data-topic-main]");
    await page.locator(".v7-domain-picker > summary").click();
    await shot(`${width}-domain-picker`);
    await page.locator(".v7-domain-picker > summary").click();
    for (const tab of [
      "structure",
      "patterns",
      "sources",
      "candidates",
      "action",
    ]) {
      await action("tab", tab);
      await shot(`${width}-${tab}`);
      await scrollShots(`${width}-${tab}`, ".lgi-tm-main, [data-topic-main]");
      if (
        tab === "candidates" &&
        (await page.locator('[data-action="structure-change"]').count())
      ) {
        await action("structure-change");
        await shot(`${width}-dialog-structure-change`);
        await scrollShots(
          `${width}-dialog-structure-change`,
          ".lgi-tm-dialog-body, [data-topic-dialog-body]",
        );
        await page.keyboard.press("Escape");
      }
      if (
        tab === "structure" &&
        (await page
          .locator('[data-action="summary-chart"][data-id="scatter"]')
          .count())
      ) {
        await action("summary-chart", "scatter");
        await shot(`${width}-structure-scatter`);
        await action("summary-chart", "table");
      }
    }
    await open("&view=journey");
    await shot(`${width}-journey-map`);
    await scrollShots(
      `${width}-journey-map`,
      ".lgi-tm-main, [data-topic-main]",
    );
    await action("map-mode", "list");
    await shot(`${width}-journey-list`);
    await action("metric", "involved");
    await shot(`${width}-journey-involved`);
    await action("stage", "pending");
    await shot(`${width}-journey-pending`);
    for (const kind of [
      "method",
      "identity",
      "saved",
      "research",
      "matrix",
      "rules",
      "changes",
    ]) {
      await open();
      const trigger = page.locator(`[data-action="${kind}"]`).first();
      if (!(await trigger.count())) continue;
      await trigger.click();
      await page.waitForLoadState("networkidle");
      await page.locator("dialog[open]").waitFor({ state: "visible" });
      await shot(`${width}-dialog-${kind}`);
      await scrollShots(
        `${width}-dialog-${kind}`,
        ".lgi-tm-dialog-body, [data-topic-dialog-body]",
      );
      if (kind === "research") {
        const config = page
          .locator("dialog details")
          .filter({ has: page.locator('form[data-form="research-config"]') })
          .first();
        if (await config.count()) await config.locator("summary").click();
        const history = page
          .locator("dialog summary")
          .filter({ hasText: "历史任务账本" })
          .first();
        if (await history.count()) await history.click();
        await shot(`${width}-dialog-research-expanded`);
        await scrollShots(
          `${width}-dialog-research-expanded`,
          ".lgi-tm-dialog-body, [data-topic-dialog-body]",
        );
      }
      await page.keyboard.press("Escape");
    }
    await open();
    await action("tab", "structure");
    const compareChoices = page.locator("[data-compare]");
    if ((await compareChoices.count()) >= 2) {
      for (let i = 0; i < 2; i++) await compareChoices.nth(i).check();
      await action("compare");
      await shot(`${width}-dialog-compare`);
      await scrollShots(
        `${width}-dialog-compare`,
        ".lgi-tm-dialog-body, [data-topic-dialog-body]",
      );
      await page.keyboard.press("Escape");
    }
    if (selected) {
      await open(`&topicRef=${selected.topicRef}`);
      await shot(`${width}-topic-selected`);
      await scrollShots(
        `${width}-topic-selected`,
        ".lgi-tm-main, [data-topic-main]",
      );
      for (const kind of ["definition", "judge", "move-topic"]) {
        const trigger = page.locator(`[data-action="${kind}"]`).first();
        if (!(await trigger.count())) continue;
        if (
          !(await trigger.isVisible()) &&
          (await page.locator('[data-action="toggle-navigation"]').isVisible())
        ) {
          await action("toggle-navigation");
        }
        await trigger.click();
        await page.waitForLoadState("networkidle");
        await shot(`${width}-dialog-${kind}`);
        await scrollShots(
          `${width}-dialog-${kind}`,
          ".lgi-tm-dialog-body, [data-topic-dialog-body]",
        );
        if (kind === "judge") {
          for (const tab of ["samples", "own", "angles"]) {
            const t = page
              .locator(`[data-action="judge-tab"][data-id="${tab}"]`)
              .first();
            if (!(await t.count())) continue;
            await t.click();
            await page.waitForLoadState("networkidle");
            await shot(`${width}-dialog-judge-${tab}`);
            await scrollShots(
              `${width}-dialog-judge-${tab}`,
              ".lgi-tm-dialog-body, [data-topic-dialog-body]",
            );
          }
          const samples = page
            .locator('[data-action="judge-tab"][data-id="samples"]')
            .first();
          if (await samples.count()) await samples.click();
          for (const sub of [
            "replace-samples",
            "judge-product",
            "judge-sources",
            "capture",
          ]) {
            const trigger = page.locator(`[data-action="${sub}"]`).first();
            if (!(await trigger.count()) || !(await trigger.isEnabled()))
              continue;
            await trigger.click();
            await settle();
            await shot(`${width}-dialog-${sub}`);
            await scrollShots(
              `${width}-dialog-${sub}`,
              ".lgi-tm-dialog-body, [data-topic-dialog-body]",
            );
            const back = page
              .locator('dialog [data-action="dialog-back"]')
              .first();
            if (await back.count()) await back.click();
          }
        }
        while (await page.locator("dialog[open]").count())
          await page.keyboard.press("Escape");
      }
    }
    await open();
    for (const kind of ["scope-materials", "reader", "manual-save"]) {
      const trigger = page.locator(`[data-action="${kind}"]`).first();
      if (!(await trigger.count()) || !(await trigger.isEnabled())) continue;
      await trigger.click();
      await page.waitForLoadState("networkidle");
      await shot(`${width}-dialog-${kind}`);
      await scrollShots(
        `${width}-dialog-${kind}`,
        ".lgi-tm-dialog-body, [data-topic-dialog-body]",
      );
      while (await page.locator("dialog[open]").count())
        await page.keyboard.press("Escape");
    }
    await page.goto(`${origin}/topics/task-initiation-difficulty`, {
      waitUntil: "networkidle",
    });
    await shot(`${width}-workspace`);
    await scrollShots(
      `${width}-workspace`,
      ".topic-workspace, [data-topic-workspace-main]",
    );
  }
} catch (error) {
  report.failures.push({ message: error.message, url: page.url() });
  await shot("failure");
} finally {
  await Promise.allSettled([...pendingReads]);
  await writeFile(
    path.join(dir, "report.json"),
    JSON.stringify(report, null, 2),
  );
  await browser.close();
}
console.log(
  JSON.stringify({
    screenshots: report.screenshots.length,
    errors: report.errors,
    blocked: report.blocked.length,
    failures: report.failures,
    dir,
  }),
);
if (report.failures.length || report.errors.length || report.blocked.length)
  process.exitCode = 1;
