// Private, paired render/interaction measurements with the same recorded actual GET inputs.
// No write, provider request, screenshot masking or source data is put in the report.
import { chromium } from "playwright";
import { readFile, mkdir, writeFile } from "node:fs/promises";
import path from "node:path";
const inputs = process.env.TOPICS_PERF_INPUTS,
  output = process.env.TOPICS_PERF_DIR;
if (!inputs || !output)
  throw new Error("Private input and output directories are required");
const recorded = JSON.parse(
  await readFile(path.join(inputs, "report.json"), "utf8"),
);
const key = (raw) => {
  const u = new URL(raw);
  u.searchParams.sort();
  return u.pathname + u.search;
};
const reads = new Map();
for (const r of recorded.reads)
  if (!reads.has(key(r.url))) reads.set(key(r.url), r);
const bodies = new Map();
for (const r of reads.values())
  if (!bodies.has(r.hash))
    bodies.set(
      r.hash,
      await readFile(path.join(inputs, "responses", `${r.hash}.body`)),
    );
const html = await readFile(path.join(inputs, "legacy-entry.html"));
const domain = /data-default-domain-ref="([^"]+)"/.exec(html.toString())?.[1];
if (!domain) throw new Error("Actual default scope missing");
const trials = Number(process.env.TOPICS_PERF_TRIALS || 7),
  rows = [],
  errors = [],
  blocked = [];
const browser = await chromium.launch({ headless: true });
async function paint(page) {
  await page.evaluate(
    () =>
      new Promise((done) =>
        requestAnimationFrame(() => requestAnimationFrame(done)),
      ),
  );
}
async function run(kind, trial) {
  const origin =
    kind === "native" ? "http://127.0.0.1:3000" : "http://127.0.0.1:3112";
  const context = await browser.newContext({
    viewport: { width: 1440, height: 900 },
    deviceScaleFactor: 1,
    reducedMotion: "reduce",
  });
  await context.route("**/*", async (route) => {
    const req = route.request(),
      u = new URL(req.url());
    if (u.origin !== origin || !["GET", "HEAD"].includes(req.method())) {
      blocked.push({ kind, method: req.method(), path: u.pathname });
      return route.abort();
    }
    if (kind === "native" && u.pathname === "/topics")
      return route.fulfill({
        status: 200,
        contentType: "text/html",
        body: html,
      });
    if (
      u.pathname.startsWith("/api/") &&
      !u.pathname.startsWith("/api/local/media/")
    ) {
      const r = reads.get(key(u));
      if (!r) {
        errors.push({
          kind,
          path: u.pathname,
          reason: "recorded read missing",
        });
        return route.abort();
      }
      return route.fulfill({
        status: r.status,
        contentType: r.contentType || "application/json",
        body: bodies.get(r.hash),
      });
    }
    return route.continue();
  });
  const page = await context.newPage();
  const inflight = new Set();
  page.on("request", (r) => {
    if (new URL(r.url()).pathname.startsWith("/api/")) inflight.add(r);
  });
  for (const event of ["requestfinished", "requestfailed"])
    page.on(event, (r) => inflight.delete(r));
  async function settled() {
    const deadline = Date.now() + 90000;
    await paint(page);
    while (inflight.size) {
      if (Date.now() > deadline) throw new Error("API settlement timed out");
      await page.waitForTimeout(10);
    }
    await paint(page);
  }
  await page.addInitScript(() => {
    window.topicPerfLongTasks = [];
    new PerformanceObserver((list) => {
      for (const e of list.getEntries())
        window.topicPerfLongTasks.push({
          start: e.startTime,
          duration: e.duration,
        });
    }).observe({ type: "longtask", buffered: true });
  });
  page.on("pageerror", (e) => errors.push({ kind, reason: e.message }));
  const start = performance.now();
  const overviewResponse = page.waitForResponse(
    (r) => new URL(r.url()).pathname === "/api/local/topic-map/overview",
  );
  await page.goto(`${origin}/topics?tm=1&domainRef=${domain}`, {
    waitUntil: "domcontentloaded",
  });
  await page
    .locator('[data-loaded="true"][aria-label="主题内容"]')
    .waitFor({ timeout: 90000 });
  await page
    .getByText(/正在载入其余材料与讨论/)
    .waitFor({ state: "hidden", timeout: 90000 });
  await (await overviewResponse).finished();
  await settled();
  rows.push({
    kind,
    trial,
    metric: "all-materials-ready",
    ms: performance.now() - start,
  });
  rows.push({
    kind,
    trial,
    diagnostic: await page.evaluate(() => ({
      longTasks: window.topicPerfLongTasks,
      resources: performance
        .getEntriesByType("resource")
        .map((e) => ({
          path: new URL(e.name).pathname,
          start: e.startTime,
          end: e.responseEnd,
          duration: e.duration,
        })),
    })),
  });
  async function click(action, id, metric) {
    const selector = `[data-action="${action}"]${id === undefined ? "" : `[data-id="${id}"]`}`;
    const ms = await page.evaluate(async (selector) => {
      const button = document.querySelector(selector);
      if (!(button instanceof HTMLElement))
        throw new Error("Missing performance action");
      const start = performance.now();
      button.click();
      await new Promise((done) =>
        requestAnimationFrame(() => requestAnimationFrame(done)),
      );
      return performance.now() - start;
    }, selector);
    rows.push({ kind, trial, metric, ms });
  }
  await click("tab", "structure", "structure");
  await click("summary-chart", "scatter", "scatter");
  await click("tab", "sources", "sources");
  await click("tab", "content", "content");
  const reader = page.locator('[data-action="reader"]').first();
  const readerStart = performance.now();
  await reader.click();
  await page.locator("dialog[open]").waitFor();
  await page.waitForFunction(() => {
    const d = document.querySelector("dialog[open]");
    return (
      d &&
      !d.textContent.includes("正在读取") &&
      !d.textContent.includes("正在核对")
    );
  });
  await settled();
  rows.push({
    kind,
    trial,
    metric: "reader-with-recorded-read",
    ms: performance.now() - readerStart,
  });
  await context.close();
}
try {
  for (let i = 0; i < trials; i++) {
    for (const kind of i % 2 ? ["react", "native"] : ["native", "react"])
      await run(kind, i);
  }
} finally {
  await browser.close();
  const summary = [];
  const quantile = (xs, p) =>
    xs[Math.min(xs.length - 1, Math.ceil(xs.length * p) - 1)];
  for (const metric of [
    ...new Set(rows.filter((r) => r.metric).map((r) => r.metric)),
  ])
    for (const kind of ["native", "react"]) {
      const xs = rows
        .filter((r) => r.kind === kind && r.metric === metric)
        .map((r) => r.ms)
        .sort((a, b) => a - b);
      if (xs.length)
        summary.push({
          metric,
          kind,
          n: xs.length,
          median: quantile(xs, 0.5),
          p95: quantile(xs, 0.95),
          min: xs[0],
          max: xs.at(-1),
        });
    }
  await mkdir(output, { recursive: true });
  await writeFile(
    path.join(output, "report.json"),
    JSON.stringify(
      {
        time: new Date().toISOString(),
        conditions: {
          trials,
          viewport: "1440x900",
          freshContext: true,
          alternatingOrder: true,
          pinnedActualInputs: true,
          legacyEntryPinned: true,
          scope:
            "renderer and recorded API response handling; excludes live SQL/model/platform latency",
          interaction:
            "DOM action to two animation frames; reader also includes explicit request settlement",
        },
        summary,
        rows,
        errors,
        blocked,
      },
      null,
      2,
    ),
  );
  console.log(JSON.stringify({ summary, errors, blocked }));
}
if (errors.length || blocked.length) process.exitCode = 1;
