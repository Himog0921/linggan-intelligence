// Private, read-only visual comparison against the actual local page. No writes or external sites.
import { chromium } from "playwright";
import assert from "node:assert/strict";
import { mkdir, writeFile } from "node:fs/promises";
import path from "node:path";
const origin = process.env.OPS_VISUAL_ORIGIN || "http://127.0.0.1:3000";
const dir = process.env.OPS_VISUAL_DIR;
if (!dir)
  throw new Error("OPS_VISUAL_DIR is required (private evidence directory)");
await mkdir(dir, { recursive: true });
const browser = await chromium.launch({ headless: true });
const context = await browser.newContext({
  viewport: { width: 1440, height: 900 },
  deviceScaleFactor: 1,
  reducedMotion: "reduce",
});
const report = {
  origin,
  time: new Date().toISOString(),
  screenshots: [],
  errors: [],
  blocked: [],
};
await context.route("**/*", async (route) => {
  const req = route.request();
  if (
    new URL(req.url()).origin !== origin ||
    !["GET", "HEAD"].includes(req.method())
  ) {
    report.blocked.push({ method: req.method(), url: req.url() });
    return route.abort();
  }
  return route.continue();
});
const page = await context.newPage();
page.on("pageerror", (e) => report.errors.push(e.message));
async function shot(name) {
  await page.screenshot({
    path: path.join(dir, `${name}.png`),
    fullPage: true,
  });
  report.screenshots.push({
    name,
    url: page.url(),
    viewport: page.viewportSize(),
    geometry: await page.evaluate(() => ({
      width: innerWidth,
      scrollWidth: document.documentElement.scrollWidth,
      targetPeriod:
        document.querySelector('dialog[open] input[name="periodType"]:checked')
          ?.value || null,
      main: document
        .querySelector(".ops-main, main")
        ?.getBoundingClientRect()
        .toJSON(),
    })),
  });
}
for (const width of [1440, 1280, 1024, 390]) {
  await page.setViewportSize({ width, height: width === 390 ? 844 : 900 });
  for (const name of ["overview", "publications", "accounts", "daily", "io"]) {
    await page.goto(`${origin}/ops/${name}?domain=all`, {
      waitUntil: "networkidle",
    });
    await page
      .locator("[aria-busy=true]")
      .waitFor({ state: "hidden", timeout: 20000 })
      .catch(() => {});
    await shot(`${width}-${name}`);
    await page.locator(".v7-domain-picker > summary").click();
    await shot(`${width}-${name}-business`);
    await page.locator(".v7-domain-picker > summary").click();
    const scroll = page
      .locator(
        name === "overview"
          ? ".ops-overview, [data-ops-overview]"
          : ".ops-table-wrap, [data-ops-table]",
      )
      .first();
    if (await scroll.count()) {
      const size = await scroll.evaluate((el) => ({
        h: el.scrollHeight,
        ch: el.clientHeight,
        w: el.scrollWidth,
        cw: el.clientWidth,
      }));
      const step = Math.max(1, size.ch - 60);
      for (let y = step; y < size.h; y += step) {
        await scroll.evaluate((el, pos) => {
          el.scrollTop = pos;
        }, y);
        await shot(`${width}-${name}-y${y}`);
      }
      await scroll.evaluate((el) => {
        el.scrollTop = 0;
        el.scrollLeft = el.scrollWidth;
      });
      if (size.w > size.cw) await shot(`${width}-${name}-right`);
    }
    if (name === "overview") {
      await page.goto(`${origin}/ops/overview?domain=all&slice=month`, {
        waitUntil: "networkidle",
      });
      await shot(`${width}-overview-month`);
      const more = page.getByRole("button", { name: /另有 .*项，展开/ });
      if (await more.count()) {
        await more.click();
        await shot(`${width}-overview-attention`);
      }
      await page.getByRole("button", { name: /设定目标|修改目标/ }).click();
      await page.getByRole("radio", { name: /本周/ }).check();
      await page.waitForFunction(() => {
        const form = document.querySelector("#ops-target-form");
        return form && /当前：(未设定|[0-9])/.test(form.textContent);
      });
      await page.waitForLoadState("networkidle");
      assert.equal(
        await page.getByRole("radio", { name: /本周/ }).isChecked(),
        true,
      );
      await shot(`${width}-target-week-dialog`);
      await page.getByRole("radio", { name: /本月/ }).check();
      await page.waitForFunction(() => {
        const form = document.querySelector("#ops-target-form");
        return form && /当前：(未设定|[0-9])/.test(form.textContent);
      });
      await page.waitForLoadState("networkidle");
      assert.equal(
        await page.getByRole("radio", { name: /本月/ }).isChecked(),
        true,
      );
      await shot(`${width}-target-month-dialog`);
      await page.keyboard.press("Escape");
      await page.getByRole("button", { name: "自定义", exact: true }).click();
      await shot(`${width}-custom-dialog`);
      await page.keyboard.press("Escape");
    }
    if (name === "publications") {
      await page.getByRole("button", { name: "日期范围", exact: true }).click();
      await shot(`${width}-date-dialog`);
      await page.keyboard.press("Escape");
      const cover = page.getByRole("button", { name: "查看封面大图" }).first();
      if (await cover.count()) {
        await cover.click();
        await page.waitForTimeout(100);
        await shot(`${width}-cover-dialog`);
        await page.keyboard.press("Escape");
      }
    }
  }
}
await writeFile(path.join(dir, "report.json"), JSON.stringify(report, null, 2));
await browser.close();
console.log(
  JSON.stringify({
    screenshots: report.screenshots.length,
    errors: report.errors,
    blocked: report.blocked.length,
    dir,
  }),
);
