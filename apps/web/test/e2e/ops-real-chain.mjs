// Real Axum + isolated PostgreSQL; synthetic data only. No external/platform/model requests.
import assert from "node:assert/strict";
import { chromium } from "playwright";
import { mkdir, readFile, writeFile } from "node:fs/promises";
import path from "node:path";
const dir = process.env.WEB_APP_PROOF_DIR;
if (!dir) throw new Error("WEB_APP_PROOF_DIR required");
const fixture = JSON.parse(
  await readFile(path.join(dir, "fixture.json"), "utf8"),
);
const { origin, domainOne, domainTwo, cover } = fixture;
const sessionToken = process.env.WEB_APP_PROOF_SESSION_TOKEN;
if (!sessionToken) throw new Error("WEB_APP_PROOF_SESSION_TOKEN required");
const report = { steps: [], errors: [], writes: [], blocked: [] };
const browser = await chromium.launch({ headless: true });
const context = await browser.newContext({
  viewport: { width: 1440, height: 900 },
  reducedMotion: "reduce",
});
await context.addCookies([
  {
    name: "linggan_ops_session",
    value: sessionToken,
    url: origin,
    httpOnly: true,
    sameSite: "Strict",
  },
]);
await context.route("**/*", (route) => {
  const request = route.request();
  if (
    new URL(request.url()).origin !== origin ||
    (!["GET", "HEAD"].includes(request.method()) &&
      !(
        request.method() === "POST" &&
        new URL(request.url()).pathname === "/api/local/ops/targets"
      ))
  ) {
    report.blocked.push({ method: request.method(), url: request.url() });
    return route.abort();
  }
  return route.continue();
});
const page = await context.newPage();
page.on("pageerror", (e) => report.errors.push(e.message));
page.on("request", (r) => {
  if (!["GET", "HEAD"].includes(r.method()))
    report.writes.push(JSON.parse(r.postData() || "{}"));
});
async function wait(check, message) {
  const until = Date.now() + 15000;
  while (Date.now() < until) {
    if (await check()) return;
    await page.waitForTimeout(25);
  }
  throw new Error(message);
}
async function step(name, fn) {
  await fn();
  report.steps.push(name);
  console.log(`ops proof: ${name}`);
}
async function go(name, query = "domain=all") {
  await page.goto(`${origin}/ops/${name}?${query}`, {
    waitUntil: "networkidle",
  });
  await page.getByRole("heading", { level: 1 }).waitFor({ state: "attached" });
}
async function json(url, body) {
  return page.evaluate(
    async ({ url, body }) => {
      const res = await fetch(url, {
        ...(body
          ? {
              method: "POST",
              headers: { "content-type": "application/json" },
              body: JSON.stringify(body),
            }
          : {}),
      });
      return { status: res.status, body: await res.json() };
    },
    { url, body },
  );
}
async function openTarget() {
  await page.getByRole("button", { name: /设定目标|修改目标/ }).click();
  await wait(
    async () =>
      !(await page.locator("#ops-target-form").innerText()).includes(
        "当前：读取中",
      ),
    "target history loaded",
  );
}
const targetValue = () =>
  page.getByRole("spinbutton", { name: "目标值（加微人数）" });
const save = () =>
  page.getByRole("button", { name: "保存目标", exact: true }).click();
try {
  await step(
    "publications filters, URL, paging and explicit unknowns",
    async () => {
      await go("publications");
      assert.equal(await page.locator("#ops-rows > tr").count(), 50);
      assert.match(
        await page.locator("#ops-page-summary").innerText(),
        /共 67 条/,
      );
      await page.getByRole("button", { name: "下一页", exact: true }).click();
      await wait(
        async () =>
          new URL(page.url()).searchParams.get("page") === "2" &&
          (await page.locator("#ops-rows > tr").count()) === 17,
        "next page",
      );
      await page.reload({ waitUntil: "networkidle" });
      assert.match(await page.locator("#ops-page-summary").innerText(), /第 2/);
      await page
        .getByRole("navigation", { name: "平台", exact: true })
        .getByRole("button", { name: "抖音", exact: true })
        .click();
      await wait(
        async () => (await page.locator("#ops-rows > tr").count()) === 1,
        "platform filter",
      );
      assert.equal(new URL(page.url()).searchParams.has("page"), false);
      assert.match(
        await page.locator("#ops-rows").innerText(),
        /抖音选填|阅读量已填/,
      );
      await page
        .getByRole("button", { name: "待补阅读量", exact: true })
        .click();
      await wait(
        async () =>
          (await page.locator("#ops-rows").innerText()).includes(
            "没有符合条件",
          ),
        "empty filter",
      );
      await page.getByRole("button", { name: "清除筛选", exact: true }).click();
      await wait(
        async () => (await page.locator("#ops-rows > tr").count()) === 50,
        "clear",
      );
      await page
        .getByRole("searchbox", { name: "搜索账号" })
        .fill("不存在的合成账号");
      await page.getByRole("button", { name: "搜索", exact: true }).click();
      await wait(
        async () =>
          (await page.locator("#ops-rows").innerText()).includes(
            "没有符合条件",
          ),
        "account search",
      );
      await page.reload({ waitUntil: "networkidle" });
      assert.match(page.url(), /account=/);
      await page.getByRole("button", { name: "清除筛选", exact: true }).click();
      await wait(
        async () => (await page.locator("#ops-rows > tr").count()) === 50,
        "clear search",
      );
      await page.getByRole("button", { name: "日期范围", exact: true }).click();
      await page.getByLabel("开始日期", { exact: true }).fill("2026-09-01");
      await page.getByLabel("结束日期", { exact: true }).fill("2026-09-30");
      await page.getByRole("button", { name: "确认", exact: true }).click();
      await wait(
        async () => (await page.locator("#ops-rows > tr").count()) === 4,
        "inclusive date filter",
      );
      assert.match(
        await page.locator("#ops-rows").innerText(),
        /另有 1 条重复登记/,
      );
      assert.match(await page.locator("#ops-rows").innerText(), /已停用/);
    },
  );
  await step(
    "historical cover is qualified and preview modal returns focus",
    async () => {
      const response = await context.request.get(
        `${origin}/ops/covers/${cover}`,
      );
      assert.equal(response.status(), 200);
      assert.match(response.headers()["content-type"], /image\/png/);
      await go("publications");
      let button = page.getByRole("button", { name: "查看封面大图" }).first();
      if (!(await button.count())) {
        await page.getByRole("button", { name: "下一页", exact: true }).click();
        await wait(
          async () =>
            (await page.getByRole("button", { name: "查看封面大图" }).count()) >
            0,
          "cover on page two",
        );
        button = page.getByRole("button", { name: "查看封面大图" }).first();
      }
      await button.click();
      await page.getByRole("dialog", { name: "封面大图" }).waitFor();
      await wait(
        async () =>
          await page
            .locator("#ops-cover-large")
            .evaluate((img) => img.complete && img.naturalWidth === 1),
        "real PNG decoded",
      );
      await page.keyboard.press("Escape");
      assert.equal(
        await button.evaluate((el) => document.activeElement === el),
        true,
      );
      assert.equal(
        (await context.request.get(`${origin}/ops/covers/not-a-ref`)).status(),
        404,
      );
    },
  );
  await step("accounts, daily and business all/domain scopes", async () => {
    const first = await json(`/api/local/ops/publications?domain=${domainOne}`);
    const second = await json(
      `/api/local/ops/publications?domain=${domainTwo}`,
    );
    assert.equal(first.body.total, 65); // The account-less registration belongs only to all.
    assert.equal(second.body.total, 1);
    await go("accounts");
    assert.equal(await page.locator("#ops-rows > tr").count(), 3);
    const zero = page
      .locator("#ops-rows > tr")
      .filter({ hasText: "SYNTHETIC 账号乙" });
    assert.equal(await zero.locator("td").nth(4).innerText(), "0");
    const unknown = page
      .locator("#ops-rows > tr")
      .filter({ hasText: "SYNTHETIC 账号丙" });
    assert.equal(await unknown.locator("td").nth(4).innerText(), "—");
    await page.getByRole("group").count();
    await page.getByText("全部业务", { exact: true }).first().click();
    await page.getByRole("button", { name: /合成业务二.*1 个账号/ }).click();
    await wait(
      async () => (await page.locator("#ops-rows > tr").count()) === 1,
      "business selection",
    );
    assert.match(await page.locator("#ops-rows").innerText(), /账号丙/);
    assert.equal(new URL(page.url()).searchParams.get("domain"), domainTwo);
    await page
      .getByRole("navigation", { name: "运营导航" })
      .count()
      .catch(() => {});
    await page
      .getByRole("navigation", { name: "数据页面" })
      .getByRole("link", { name: /每日数据/ })
      .click();
    await wait(
      async () => (await page.locator("#ops-rows > tr").count()) === 4,
      "daily rows",
    );
    assert.match(
      await page.locator("#ops-scope-note").innerText(),
      /每日数据不分业务/,
    );
    const all = await json("/api/local/ops/daily?domain=all"),
      scoped = await json(`/api/local/ops/daily?domain=${domainTwo}`);
    assert.deepEqual(scoped.body.items, all.body.items);
    await go("accounts", "domain=unknown");
    await wait(
      async () => new URL(page.url()).searchParams.get("domain") === "all",
      "invalid business corrected",
    );
  });
  await step(
    "retired IO redirects; six read-only exports keep frozen core bytes and cutoff",
    async () => {
      await go("io");
      assert.equal(new URL(page.url()).pathname, '/ops/overview');
      assert.equal(await page.getByText('导入导出', { exact: true }).count(), 0);
      for (const kind of ["accounts", "publications", "daily"])
        for (const format of ["csv", "json"]) {
          // The visible IO module is retired; its existing read-only backend remains compatible.
          const response = await context.request.get(`${origin}/api/local/ops/export?kind=${kind}&format=${format}`);
          assert.equal(response.status(), 200);
          const bytes = await response.body(), expected = await readFile(path.join(dir, `expected-${kind}.${format}`));
          assert.deepEqual(bytes, expected, `${kind} ${format} byte equality`);
          if (format === "json") assert.equal(JSON.parse(bytes).asOf, fixture.asOf.exportedAt);
          else assert.match(bytes.toString(), /2026-10-12 22:43/);
        }
    },
  );
  await step(
    "overview covered/unknown windows, charts, custom dates and reduced motion",
    async () => {
      await go("overview");
      assert.match(await page.locator("#ops-window").innerText(), /还没有迁入/);
      assert.match(await page.locator("#ops-target").innerText(), /未设定/);
      await page.getByRole("button", { name: "本月", exact: true }).click();
      await wait(
        async () =>
          (await page.locator("#ops-hero-title").innerText()) === "本月加微",
        "month",
      );
      // Visual reorganization is data-only: no action list, but all analysis sections remain.
      assert.equal(await page.getByRole("heading", { name: "今日必须处理" }).count(), 0);
      assert.match(await page.locator("[data-ops-overview]").innerText(), /有发布账号/);
      assert.match(await page.locator("[data-ops-overview]").innerText(), /运营人表现/);
      assert.match(await page.locator("[data-ops-overview]").innerText(), /账号矩阵/);
      for (const name of ["按周", "按月", "按日"]) {
        await page.getByRole("button", { name, exact: true }).click();
        await wait(
          async () => (await page.locator("#ops-trend svg").count()) === 1,
          "trend",
        );
      }
      await page
        .getByRole("navigation", { name: "产出指标" })
        .getByRole("button", { name: "加微", exact: true })
        .click();
      assert.match(
        await page.locator("#ops-output-legend").innerText(),
        /不分平台/,
      );
      await page.getByRole("button", { name: "自定义", exact: true }).click();
      await page.getByLabel("开始日期", { exact: true }).fill("2026-10-02");
      await page.getByLabel("结束日期", { exact: true }).fill("2026-10-01");
      await page.getByRole("button", { name: "确认", exact: true }).click();
      assert.match(
        await page.getByRole("alert").innerText(),
        /开始日期不能晚于/,
      );
      await page.getByLabel("开始日期", { exact: true }).fill("2026-09-01");
      await page.getByLabel("结束日期", { exact: true }).fill("2026-10-11");
      await page.getByRole("button", { name: "确认", exact: true }).click();
      await wait(
        async () =>
          (await page.locator("#ops-target").innerText()).includes(
            "自定义时段不对应目标",
          ),
        "custom",
      );
      assert.match(page.url(), /from=2026-09-01/);
      await page.reload({ waitUntil: "networkidle" });
      assert.match(await page.locator("#ops-window").innerText(), /2026-09-01/);
    },
  );
  await step(
    "real target accepted, unchanged and concurrent stale receipt",
    async () => {
      await go("overview", "domain=all&slice=month");
      await openTarget();
      await targetValue().fill("100");
      await save();
      await page.getByRole("dialog").waitFor({ state: "hidden" });
      await wait(
        async () =>
          (await page.locator("#ops-target").innerText()).includes("100"),
        "target accepted",
      );
      await openTarget();
      await save();
      await page.getByRole("dialog").waitFor({ state: "hidden" });
      await wait(
        async () =>
          (await page.locator("#ops-status").innerText()).includes(
            "目标没有变化",
          ),
        "unchanged",
      );
      await openTarget();
      const month = (
        await json("/api/local/ops/overview?domain=all&slice=month")
      ).body.window.thisMonth;
      const history = (
        await json(`/api/local/ops/targets?periodType=month&periodKey=${month}`)
      ).body;
      const concurrent = await json("/api/local/ops/targets", {
        periodType: "month",
        periodKey: month,
        targetValue: 250,
        expectedRevision: history.current.revisionRef,
        requestId: crypto.randomUUID(),
      });
      assert.equal(concurrent.body.outcome, "accepted");
      await targetValue().fill("200");
      await save();
      await wait(
        async () =>
          (await page.getByRole("alert").innerText()).includes(
            "目标刚被修改过",
          ),
        "stale",
      );
      assert.match(
        await page.locator("#ops-target-form").innerText(),
        /当前：250/,
      );
      await save();
      await page.getByRole("dialog").waitFor({ state: "hidden" });
      await wait(
        async () =>
          (await page.locator("#ops-target").innerText()).includes("200"),
        "stale confirmed",
      );
    },
  );
  await step(
    "committed but lost response retries frozen request after dialog remount",
    async () => {
      await openTarget();
      await targetValue().fill("300");
      let lose = true;
      const bodies = [];
      const routeHandler = async (route) => {
        if (route.request().method() !== "POST") return route.fallback();
        bodies.push(JSON.parse(route.request().postData()));
        const response = await route.fetch();
        if (lose) {
          lose = false;
          assert.equal((await response.json()).outcome, "accepted");
          return route.abort("failed");
        }
        return route.fulfill({ response });
      };
      await page.route("**/api/local/ops/targets", routeHandler);
      await save();
      await wait(
        async () =>
          (await page.getByRole("alert").innerText()).includes("保存失败"),
        "unconfirmed",
      );
      await page.getByRole("button", { name: "取消", exact: true }).click();
      await page.getByRole("button", { name: "收起并继续跟踪", exact: true }).click();
      await openTarget();
      assert.equal(await targetValue().inputValue(), "300");
      assert.equal(await targetValue().isDisabled(), true);
      await save();
      await page.getByRole("dialog").waitFor({ state: "hidden" });
      assert.deepEqual(bodies[1], bodies[0]);
      await page.unroute("**/api/local/ops/targets", routeHandler);
      const conflict = await json("/api/local/ops/targets", {
        ...bodies[0],
        targetValue: 301,
      });
      assert.equal(conflict.status, 409);
      assert.equal(conflict.body.code, "ops_target_request_conflict");
    },
  );
  await step(
    "explicit 409 is shown without confirming a target write",
    async () => {
      await go("overview", "domain=all&slice=month");
      await openTarget();
      await targetValue().fill("350");
      const accepted = report.writes.find(
        (command) => command.targetValue === 300,
      );
      assert.ok(accepted);
      const handler = async (route) => {
        if (route.request().method() !== "POST") return route.fallback();
        const response = await route.fetch({
          postData: JSON.stringify({ ...accepted, targetValue: 350 }),
        });
        assert.equal(response.status(), 409);
        return route.fulfill({ response });
      };
      await page.route("**/api/local/ops/targets", handler);
      await save();
      await wait(
        async () =>
          (await page.getByRole("alert").innerText()).includes("保存失败"),
        "409 shown",
      );
      assert.equal(await targetValue().inputValue(), "350");
      assert.equal(await targetValue().isDisabled(), false);
      assert.match(await page.locator("#ops-target").innerText(), /300/);
      await page.getByRole("button", { name: "取消", exact: true }).click();
      await page.getByRole("button", { name: "放弃修改", exact: true }).click();
      await page.unroute("**/api/local/ops/targets", handler);
    },
  );
  await step(
    "failed read is retryable, invalid response not empty, stale filter cannot overwrite",
    async () => {
      for (const body of [
        '{"items":[]}',
        JSON.stringify({ total: 0, active: 0, operators: [], items: [] }),
      ]) {
        const malformed = (route) =>
          route.fulfill({ status: 200, contentType: "application/json", body });
        await page.route("**/api/local/ops/accounts?*", malformed);
        await go("accounts");
        assert.match(
          await page.locator("#ops-status").innerText(),
          /返回的内容无法读取/,
        );
        assert.equal(await page.locator("#ops-rows > tr").count(), 0);
        assert.doesNotMatch(
          await page.locator("#ops-rows").innerText(),
          /还没有迁入数据/,
        );
        await page.unroute("**/api/local/ops/accounts?*", malformed);
      }
      let fail = true;
      const handler = (route) => {
        if (fail)
          return route.fulfill({
            status: 503,
            contentType: "application/json",
            body: JSON.stringify({ code: "ops_read_unavailable" }),
          });
        return route.fallback();
      };
      await page.route("**/api/local/ops/publications?*", handler);
      await go("publications");
      assert.match(await page.locator("#ops-status").innerText(), /读取失败/);
      assert.equal(await page.locator("#ops-rows > tr").count(), 0);
      fail = false;
      await page.getByRole("button", { name: "重试", exact: true }).click();
      await wait(
        async () => (await page.locator("#ops-rows > tr").count()) === 50,
        "read retry",
      );
      await page.unroute("**/api/local/ops/publications?*", handler);
      let release;
      const held = new Promise((resolve) => {
        release = resolve;
      });
      let entered;
      const started = new Promise((resolve) => {
        entered = resolve;
      });
      const slow = async (route) => {
        const url = new URL(route.request().url());
        if (url.searchParams.get("platform") === "douyin") {
          entered();
          await held;
          await route.continue().catch(() => {});
        } else await route.continue();
      };
      await page.route("**/api/local/ops/publications?*", slow);
      await page
        .getByRole("navigation", { name: "平台", exact: true })
        .getByRole("button", { name: "抖音", exact: true })
        .click();
      await started;
      await page
        .getByRole("navigation", { name: "平台", exact: true })
        .getByRole("button", { name: "小红书", exact: true })
        .click();
      await wait(
        async () => (await page.locator("#ops-rows > tr").count()) === 50,
        "new filter",
      );
      release();
      await page.waitForLoadState("networkidle");
      assert.equal(new URL(page.url()).searchParams.get("platform"), "xhs");
      assert.equal(await page.locator("#ops-rows > tr").count(), 50);
      assert.equal(
        await page
          .locator("#ops-rows > tr")
          .filter({ hasText: "抖音选填" })
          .count(),
        0,
      );
      await page.unroute("**/api/local/ops/publications?*", slow);
    },
  );
  await step(
    "four viewports, four data pages and dialog keyboard navigation",
    async () => {
      for (const width of [1440, 1280, 1024, 390]) {
        await page.setViewportSize({
          width,
          height: width === 390 ? 844 : 900,
        });
        for (const name of [
          "overview",
          "publications",
          "accounts",
          "daily",
        ]) {
          await go(name);
          assert.ok(
            await page.evaluate(
              () => document.documentElement.scrollWidth <= innerWidth,
            ),
            `${width} ${name} document overflow`,
          );
          await page.screenshot({
            path: path.join(dir, `${width}-${name}.png`),
            fullPage: true,
          });
        }
      }
      await go("overview");
      await openTarget();
      await page.keyboard.press("Tab");
      assert.equal(
        await page.evaluate(() =>
          Boolean(document.activeElement?.closest("dialog")),
        ),
        true,
      );
      await page.keyboard.press("Escape");
      await page.getByRole("dialog").waitFor({ state: "hidden" });
    },
  );
  assert.deepEqual(report.errors, []);
  assert.deepEqual(report.blocked, []);
} finally {
  await mkdir(dir, { recursive: true });
  await writeFile(
    path.join(dir, "report.json"),
    JSON.stringify(report, null, 2),
  );
  await browser.close();
}
console.log(
  JSON.stringify({
    passed: report.steps.length,
    writes: report.writes.length,
    blocked: report.blocked.length,
  }),
);
