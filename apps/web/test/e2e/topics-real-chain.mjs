// Stage F real Axum + disposable PostgreSQL + React. All material is handwritten synthetic input.
import assert from "node:assert/strict";
import { chromium } from "playwright";
import { mkdir, readFile, writeFile } from "node:fs/promises";
import path from "node:path";
import { verifyTopicWorkspace } from "./topic-workspace-real-chain.mjs";
const dir = process.env.WEB_APP_PROOF_DIR;
if (!dir) throw new Error("WEB_APP_PROOF_DIR required");
const fixture = JSON.parse(
  await readFile(path.join(dir, "fixture.json"), "utf8"),
);
const { origin, domain, domainTwo, topicRef, workRefs } = fixture;
const report = {
  steps: [],
  errors: [],
  writes: [],
  blocked: [],
  receipts: [],
  reads: [],
};
const browser = await chromium.launch({ headless: true });
const context = await browser.newContext({
  viewport: { width: 1440, height: 900 },
  reducedMotion: "reduce",
  acceptDownloads: true,
});
const commandPaths = new Set([
  "/api/local/topic-map/commands",
  "/api/local/topic-map/structure/preview",
  "/api/local/topic-map/structure/apply",
  "/api/local/topic-map/research/commands",
  "/api/local/topic-map/search/commands",
  "/api/local/topic-map/collection/commands",
]);
await context.route("**/*", (route) => {
  const r = route.request(),
    u = new URL(r.url());
  if (
    u.origin !== origin ||
    (!["GET", "HEAD"].includes(r.method()) &&
      !(r.method() === "POST" && commandPaths.has(u.pathname)))
  ) {
    report.blocked.push({ method: r.method(), url: r.url() });
    return route.abort();
  }
  return route.continue();
});
const page = await context.newPage();
page.on("pageerror", (e) => report.errors.push(e.message));
const reads = [];
page.on("response", (response) => {
  if (
    response.url().includes("/api/") &&
    response.headers()["content-type"]?.includes("application/json")
  )
    reads.push(
      (async () => {
        try {
          report.reads.push({
            url: response.url(),
            status: response.status(),
            body: await response.json(),
          });
        } catch {
          /* A canceled read has no synthetic replacement. */
        }
      })(),
    );
});
page.on("request", (r) => {
  if (r.method() === "POST")
    report.writes.push({ url: r.url(), body: r.postDataJSON() });
});
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
async function settled() {
  const end = Date.now() + 30000;
  while (inflight.size || Date.now() - lastActivity < 150) {
    if (Date.now() > end) throw Error("Isolated API reads did not settle");
    await page.waitForTimeout(25);
  }
}
async function step(name, fn) {
  await fn();
  report.steps.push(name);
  console.log(`Topic proof: ${name}`);
}
async function go(suffix = "") {
  await page.goto(`${origin}/topics?tm=1&domainRef=${domain}${suffix}`, {
    waitUntil: "networkidle",
  });
  await page.locator('[data-loaded="true"][aria-label="主题内容"]').waitFor();
  await page.getByText(/正在载入其余材料与讨论/).waitFor({ state: "hidden" });
}
async function action(name, id) {
  let el = page.locator(`[data-action="${name}"]`);
  if (id !== undefined) el = el.and(page.locator(`[data-id="${id}"]`));
  await el.first().click();
  await page.waitForLoadState("networkidle");
  await settled();
}
async function close() {
  while (await page.locator("dialog[open]").count())
    await page.keyboard.press("Escape");
}
async function post(endpoint, body) {
  return page.evaluate(
    async ({ endpoint, body }) => {
      const r = await fetch(endpoint, {
        method: "POST",
        headers: { "content-type": "application/json" },
        body: JSON.stringify(body),
      });
      return { status: r.status, body: await r.json() };
    },
    { endpoint, body },
  );
}
async function submit(form) {
  const response = page.waitForResponse(
    (r) =>
      r.request().method() === "POST" &&
      commandPaths.has(new URL(r.url()).pathname),
  );
  await form.locator('button[type="submit"]').click();
  const r = await response;
  const body = await r.json();
  await settled();
  report.receipts.push({ status: r.status(), body });
  return { status: r.status(), body };
}
try {
  await step(
    "default name-order domain, staged hydration, full denominator and safe source text",
    async () => {
      await page.goto(`${origin}/topics`, { waitUntil: "networkidle" });
      await page
        .locator('[data-loaded="true"][aria-label="主题内容"]')
        .waitFor();
      await page
        .getByText(/正在载入其余材料与讨论/)
        .waitFor({ state: "hidden" });
      assert.equal(new URL(page.url()).searchParams.get("domainRef"), domain);
      assert.match(
        await page.locator('[aria-label="主题内容"]').innerText(),
        /26/,
      );
      await go();
      assert.equal(
        await page.locator('img[src="x"]').count(),
        0,
        "synthetic HTML title must remain text",
      );
      for (const tab of [
        "structure",
        "patterns",
        "sources",
        "candidates",
        "action",
        "content",
      ]) {
        await action("tab", tab);
        await page.screenshot({
          path: path.join(dir, `topic-${tab}.png`),
          fullPage: true,
        });
      }
      await go("&view=journey");
      await action("map-mode", "list");
      await action("metric", "involved");
      await action("stage", "pending");
      await page.reload({ waitUntil: "networkidle" });
      assert.equal(new URL(page.url()).searchParams.get("metric"), "involved");
    },
  );
  await step("material-first overview retains full statistics, local search, distribution drill-down and left navigation", async () => {
    await go();
    const navigation = page.getByRole("navigation", { name: "主题图谱视图" });
    assert.equal(await navigation.count(), 1);
    const sidebar = page.locator('aside[aria-label="主题图谱导航"]');
    const tree = sidebar.getByLabel("稳定主题结构");
    assert.equal(await tree.getByRole("button", { name: "调整归属" }).count(), 0,
      "adjust-assignment control must not occupy the first row of the topic tree");
    assert.equal(await tree.locator('button[data-action="topic"]').first().innerText(), "全部方向");
    assert.equal(await sidebar.getByRole("button", { name: "调整归属" }).count(), 1,
      "existing move-topic capability must remain reachable below the tree");
    const scopeFilter = page.locator('summary[aria-label="选择统计平台与发布时间"]').locator("..");
    assert.equal(await scopeFilter.count(), 1, "scope filters must remain discoverable");
    assert.equal(await scopeFilter.evaluate((el) => el.open), false);
    await scopeFilter.locator("summary").click();
    assert.equal(await scopeFilter.evaluate((el) => el.open), true);
    assert.ok(await scopeFilter.locator("select").first().isVisible(), "scope selector must remain usable");
    await scopeFilter.locator("summary").click();
    assert.equal(await scopeFilter.evaluate((el) => el.open), false);
    assert.equal(await page.locator('aside[aria-label="主题图谱导航"]').getByRole("navigation", { name: "主题图谱视图" }).count(), 0);
    const totals = await page.getByLabel("基础概览指标").innerText();
    const distribution = await page.getByLabel("已观察内容点赞分布").innerText();
    await page.getByRole("textbox", { name: "查找材料与方向" }).fill("no-such-material-b-overview");
    await page.getByRole("button", { name: "查找", exact: true }).click();
    await page.getByText("当前条件没有关键词命中", { exact: true }).waitFor();
    assert.equal(await page.getByLabel("基础概览指标").innerText(), totals);
    assert.equal(await page.getByLabel("已观察内容点赞分布").innerText(), distribution);
    await navigation.getByRole("button", { name: "用户生命旅程", exact: true }).click();
    assert.equal(new URL(page.url()).searchParams.get("query"), "no-such-material-b-overview");
    await navigation.getByRole("button", { name: "主题概览", exact: true }).click();
    await action("clear-filter", "query");
    const chart = page.getByLabel("已观察内容点赞分布");
    assert.equal(await page.getByLabel("基础概览指标").getByRole("button",
      { name: "查看或调整样本爆款规则" }).count(), 0,
      "the sample rule action must never shift a KPI label or the neighboring median");
    const ruleLink = chart.getByRole("button", { name: "查看或调整样本爆款规则" });
    assert.equal(await ruleLink.count(), 1, "rule action stays on the chart heading");
    const titleBox = await chart.getByText("样本表现", { exact: true }).boundingBox();
    const compactBars = await chart.locator('button[data-action="subset"]').evaluateAll((buttons) => buttons.map((button) => {
      const count = Number(button.querySelector("b")?.textContent);
      const { y } = button.querySelector("b").getBoundingClientRect();
      const { height, width } = button.querySelector("b + span > span").getBoundingClientRect();
      return { count, y, height, width };
    }));
    assert.ok(titleBox && compactBars.every(({ y }) => y >= titleBox.y + titleBox.height), "compact bar counts must remain below the chart heading");
    const maxCount = Math.max(...compactBars.map(({ count }) => count));
    const maxHeight = Math.max(...compactBars.map(({ height }) => height));
    assert.ok(compactBars.every(({ count, height, width }) => count === 0 ? height === 0 : height > 0 && width > 0), "non-empty bins must keep visible bars");
    assert.ok(compactBars.every(({ count, height }) => Math.abs(height * maxCount - maxHeight * count) <= 1), "compact bars must preserve the sample-count ratio");
    const compactControlSizes = await chart.getByRole("group", { name: "材料表现视图" }).getByRole("button").evaluateAll((buttons) => buttons.map((button) => {
      const { width, height } = button.getBoundingClientRect();
      return { width, height };
    }));
    assert.ok(compactControlSizes.every(({ width, height }) => width >= 24 && height >= 24), "compact view controls must have 24px targets");
    // Visual-fidelity guard against the merged V3 regression reported in #448.
    await page.setViewportSize({ width: 2048, height: 960 });
    await page.evaluate(() => document.fonts.ready);
    const visual = await page.getByLabel("基础概览指标").evaluate((region) => {
      const cards = [...region.firstElementChild.children].slice(0, 4);
      return cards.map((card) => {
        const caption = card.children[0].getBoundingClientRect();
        const value = card.children[1].querySelector("strong")?.getBoundingClientRect();
        const foot = card.children[2].getBoundingClientRect();
        return { captionTop: caption.top, valueBottom: value?.bottom ?? 0, footTop: foot.top };
      });
    });
    await page.screenshot({ path: path.join(dir, "topic-overview-v3-alignment-2048.png") });
    assert.equal(visual.length, 4, "single-platform baseline includes four aligned readings");
    const spread = (values) => Math.max(...values) - Math.min(...values);
    assert.ok(spread(visual.map((m) => m.captionTop)) <= 2,
      "four KPI headings must share a precise top baseline: " + JSON.stringify(visual));
    assert.ok(spread(visual.map((m) => m.valueBottom)) <= 2,
      "all four KPI numbers must share the same bottom baseline: " + JSON.stringify(visual));
    assert.ok(spread(visual.map((m) => m.footTop)) <= 2,
      "secondary statistics must align on one row: " + JSON.stringify(visual));
    const bandHeight = await page.getByLabel("主题概览与样本表现")
      .evaluate((node) => node.getBoundingClientRect().height);
    assert.ok(bandHeight < 165,
      "the chart and metrics must fit the approved compact V3 band without a spare explanation row");
    await page.setViewportSize({ width: 1440, height: 900 });
    await ruleLink.click();
    await page.locator("dialog[open]").waitFor();
    await page.keyboard.press("Escape");
    await chart.getByRole("button", { name: "表格", exact: true }).click();
    const drill = chart.getByRole("table").getByRole("button", { name: "查看", exact: true }).filter({ visible: true });
    const enabled = await drill.evaluateAll((buttons) => buttons.findIndex((button) => !button.disabled));
    assert.ok(enabled >= 0);
    const refs = (await drill.nth(enabled).getAttribute("data-id")).split(",");
    await drill.nth(enabled).click();
    await page.locator("dialog[open]").waitFor();
    const openedRefs = await page.locator('dialog [data-action="reader"]').evaluateAll((buttons) => [...new Set(buttons.map((button) => button.dataset.id))]);
    assert.deepEqual(new Set(openedRefs), new Set(refs));
    await page.keyboard.press("Escape");
    await page.screenshot({ path: path.join(dir, "topic-overview-b-table.png"), fullPage: true });
    await chart.getByRole("button", { name: "分布图", exact: true }).click();
    await page.screenshot({ path: path.join(dir, "topic-overview-b-chart.png"), fullPage: true });
  });
  await step(
    "identity POST receives persisted receipt; identical retry replays and changed payload conflicts",
    async () => {
      await go();
      await action("identity");
      const form = page.locator('form[data-form="own-creator"]');
      await form
        .locator('select[name="creator"]')
        .selectOption("xhs|synthetic-author-0");
      let lost = true,
        committedReceipt;
      const interceptor = async (route) => {
        const body = route.request().postDataJSON();
        if (lost && body.action === "ownCreator") {
          const actual = await route.fetch();
          assert.equal(actual.status(), 200);
          committedReceipt = await actual.json();
          lost = false;
          await route.abort("failed");
        } else await route.continue();
      };
      await page.route("**/api/local/topic-map/commands", interceptor);
      await form.locator('button[type="submit"]').click();
      const retry = page.locator('dialog [data-action="mutation-retry"]');
      await retry.waitFor();
      await form
        .locator('[name="creator"]')
        .selectOption("xhs|synthetic-author-1");
      const retried = page.waitForResponse(
        (r) =>
          new URL(r.url()).pathname === "/api/local/topic-map/commands" &&
          r.request().method() === "POST",
      );
      await retry.click();
      const answer = await retried;
      const first = { status: answer.status(), body: await answer.json() };
      await settled();
      assert.deepEqual(
        first.body,
        committedReceipt,
        "lost response retry returns the committed receipt",
      );
      const ownRequests = report.writes.filter(
        (w) => w.body.action === "ownCreator",
      );
      assert.deepEqual(
        ownRequests[0].body,
        ownRequests[1].body,
        "retry freezes the original full payload despite edited selection",
      );
      await page.unroute("**/api/local/topic-map/commands", interceptor);
      assert.equal(first.status, 200);
      assert.equal(first.body.action, "ownCreator");
      assert.ok(first.body.receiptRef);
      await page
        .getByText(new RegExp(`已保存 · 回执 ${first.body.receiptRef}`))
        .waitFor();
      const request = report.writes.find(
        (w) => w.body.action === "ownCreator",
      ).body;
      const replay = await post("/api/local/topic-map/commands", request);
      assert.deepEqual(replay, first);
      const conflict = await post("/api/local/topic-map/commands", {
        ...request,
        active: false,
      });
      assert.equal(conflict.status, 409);
      assert.equal(conflict.body.outcome, "conflict");
      await close();
    },
  );
  await step("platform rule persists with explicit version", async () => {
    await action("rules");
    const form = page.locator('form[data-form="rule"][data-platform="xhs"]');
    await form.locator('[name="threshold"]').fill("1000");
    const result = await submit(form);
    assert.equal(result.status, 200);
    assert.equal(result.body.action, "performanceRule");
    assert.equal(result.body.revision, 1);
    await close();
  });
  await step(
    "manual alternative saves real frozen sources, reopens and exports exact references",
    async () => {
      await go();
      const manual = page.locator('[data-action="manual-save"]').first();
      const selectedRef = await manual.getAttribute("data-id");
      assert(workRefs.includes(selectedRef));
      await manual.click();
      const form = page.locator('form[data-form="save"]');
      await form.locator('[name="title"]').fill("合成浏览器备选");
      await form.locator('[name="angle"]').fill("只保留这条明确材料");
      await form.locator('[name="rationale"]').fill("检查边界，不推断现实分布");
      const saved = await submit(form);
      assert.equal(saved.status, 200);
      assert.equal(saved.body.action, "saveAlternative");
      assert.ok(saved.body.subjectRef);
      await close();
      await action("saved");
      await page
        .getByRole("heading", { name: "合成浏览器备选", exact: true })
        .waitFor();
      await action("saved-materials", saved.body.subjectRef);
      await page
        .getByText(/只保留这条明确材料/)
        .first()
        .waitFor();
      assert.match(
        await page.locator("dialog").innerText(),
        /只保留这条明确材料/,
      );
      await close();
      await action("saved");
      const download = page.waitForEvent("download");
      await action("export-alternative", saved.body.subjectRef);
      const file = await download;
      const output = await file.path();
      const text = await readFile(output, "utf8");
      assert.match(text, /合成浏览器备选/);
      assert.ok(text.includes(selectedRef));
      await close();
    },
  );
  await step(
    "retired research preserves history and central settings without configuration or dispatch",
    async () => {
      await go();
      await action("research");
      const dialog=page.getByRole("dialog",{name:"AI 状态与历史研究"});await dialog.waitFor();
      assert.equal(await dialog.getByRole("link",{name:"去 AI 设置"}).getAttribute("href"),"/settings/models?section=automation&task=legacy_topic_map");
      assert.equal(await dialog.locator('form[data-form="research-config"]').count(),0);
      const refreshed=page.waitForResponse(r=>r.request().method()==="GET"&&r.url().includes("/topic-map/research?"));
      await dialog.getByRole("button",{name:"刷新历史记录"}).click();assert.equal((await refreshed).status(),200);
      const response=await page.request.post(origin+"/api/local/topic-map/research/commands",{data:{
        action:"configure",requestRef:crypto.randomUUID(),domainRef:domain,modelConfigRef:fixture.modelConfigRef,
        dailyTokenLimit:50000,runTokenLimit:10000,automaticEnabled:true,collectionEnabled:false,
      }});
      assert.equal(response.status(),410);assert.equal((await response.json()).error,"ai_legacy_execution_retired");
      await close();
    },
  );
  await step(
    "existing bounded search/detail rounds decode and explicit stop receipts remain separate from acquisition",
    async () => {
      await go(`&topicRef=${topicRef}`);
      await action("judge", topicRef);
      await action("capture", topicRef);
      for (const [name, ref] of [
        ["search-stop", fixture.searchRound],
        ["collection-stop", fixture.detailRound],
      ]) {
        const received = page.waitForResponse(
          (r) =>
            r.request().method() === "POST" &&
            new URL(r.url()).pathname.endsWith("/commands"),
        );
        await action(name, ref);
        const response = await received;
        const receipt = await response.json();
        assert.equal(response.status(), 200);
        assert.equal(receipt.roundRef, ref);
        assert.equal(receipt.state, "stopped");
      }
      await page.screenshot({
        path: path.join(dir, "bounded-rounds.png"),
        fullPage: true,
      });
      await close();
    },
  );
  await step(
    "structure merge previews exact sources then persists one explicit replacement",
    async () => {
      await go();
      await action("tab", "candidates");
      await action("structure-change");
      for (const ref of [topicRef, fixture.peerTopicRef])
        await page.locator(`[data-structure-source="${ref}"]`).check();
      const form = page.locator('form[data-form="structure"]');
      await form.locator('[name="title-0"]').fill("合成结构替换");
      await form
        .locator('[name="definition-0"]')
        .fill("只分配两条合成来源；旧判断不继承。");
      for (const ref of [workRefs[0], workRefs[3]]) {
        await form.locator(`[name="member-${ref}"]`).selectOption("0");
        await form.locator(`[name="reason-${ref}"]`).fill("合成明确分配理由");
      }
      await form
        .locator('[name="rationale"]')
        .fill("合成 UI 结构预览与保存证明");
      const preview = await submit(form);
      assert.equal(preview.status, 200);
      assert.match(preview.body.previewHash, /^[0-9a-f]{64}$/);
      assert.equal(preview.body.impact.claimsInherited, false);
      const received = page.waitForResponse(
        (r) =>
          new URL(r.url()).pathname.endsWith("/structure/apply") &&
          r.request().method() === "POST",
      );
      await action("structure-apply");
      const response = await received;
      const result = await response.json();
      assert.equal(response.status(), 200);
      assert.equal(result.state, "persisted");
      assert.equal(result.oldReferencesRetained, true);
      assert.equal(result.claimsInherited, false);
      assert.equal(result.destinations.length, 1);
      await settled();
      assert.equal(
        await page.locator('[data-action="structure-apply"]').isDisabled(),
        true,
      );
      await close();
    },
  );
  await step(
    "independent Topic workspace exact roles, canonical references and real missing response",
    async () => {
      await verifyTopicWorkspace({
        page,
        baseURL: origin,
        fixture,
        evidenceDir: dir,
      });
    },
  );
  await step(
    "narrow layout, browser navigation and explicit unreadable API boundary",
    async () => {
      await page.setViewportSize({ width: 390, height: 844 });
      await go(`&topicRef=${topicRef}`);
      await action("toggle-navigation");
      await page.screenshot({
        path: path.join(dir, "topic-mobile.png"),
        fullPage: true,
      });
      await page.setViewportSize({ width: 1440, height: 900 });
      await go();
      const foreign = await page.request.get(
        `${origin}/api/local/topic-map/summary?domainRef=${domain}`,
        { headers: { origin: "https://evil.example" } },
      );
      assert.equal(foreign.status(), 403);
      const unknown = await page.request.get(
        `${origin}/api/local/topic-map/unknown`,
      );
      assert.equal(unknown.status(), 404);
      const other = await page.request.get(
        `${origin}/api/local/topic-map/overview?domainRef=${domainTwo}`,
      );
      const data = await other.json();
      assert.equal(data.statistics.workCount, 1);
      assert.equal(data.scope.domainRef, domainTwo);
    },
  );
  assert.deepEqual(report.errors, []);
  assert.deepEqual(report.blocked, []);
} catch (error) {
  report.failure = error.stack;
  report.failureText = await page.locator("body").innerText();
  await page.screenshot({
    path: path.join(dir, "failure.png"),
    fullPage: true,
  });
  process.exitCode = 1;
} finally {
  await Promise.allSettled(reads);
  await mkdir(dir, { recursive: true });
  await writeFile(
    path.join(dir, "report.json"),
    JSON.stringify(report, null, 2),
  );
  await browser.close();
}
console.log(
  JSON.stringify({
    steps: report.steps,
    errors: report.errors,
    blocked: report.blocked,
    failure: report.failure,
  }),
);
