import assert from "node:assert/strict";
import { mkdir } from "node:fs/promises";
import path from "node:path";

/** Reads the real disposable PostgreSQL → Axum → React composition. No route stubs. */
export async function verifyTopicWorkspace({
  page,
  baseURL,
  fixture,
  evidenceDir,
}) {
  const origin = new URL(baseURL);
  assert(
    ["127.0.0.1", "localhost", "[::1]"].includes(origin.hostname),
    "Workspace proof is loopback-only",
  );
  assert.equal(
    fixture.materials.length,
    3,
    "fixture supplies three independently classified Work Resources",
  );
  const directory = path.join(evidenceDir, "workspace");
  await mkdir(directory, { recursive: true });
  const steps = [];
  const screenshots = [];
  const writes = [];
  const observe = (request) => {
    if (!["GET", "HEAD"].includes(request.method()))
      writes.push({
        method: request.method(),
        pathname: new URL(request.url()).pathname,
      });
  };
  page.on("request", observe);
  const root = page.locator("[data-topic-workspace]");
  const feedback = page.locator("#topic-feedback");
  const list = page.locator('#topic-material-list [role="option"]');
  const inspector = page.locator("#topic-inspector-facts");
  const materialUrl = (ref) => `/api/local/work-resources/${ref}`;
  async function open() {
    const read = page.waitForResponse(
      (response) =>
        new URL(response.url()).pathname ===
        `/api/local/topic-workspaces/${fixture.workspaceKey}`,
    );
    await page.goto(new URL(`/topics/${fixture.workspaceKey}`, baseURL).href, {
      waitUntil: "networkidle",
    });
    assert.equal((await read).status(), 200);
    await page.locator('#topic-feedback[data-state="ready"]').waitFor();
    assert.equal(
      await page.locator("#topic-title").textContent(),
      fixture.expectedDefinitionName,
    );
    assert.equal(
      await page.locator("#topic-version").textContent(),
      `VERSION ${fixture.expectedDefinitionVersion}`,
    );
    assert.equal(await list.count(), fixture.materials.length);
    assert.equal(
      await root.getAttribute("data-topic-key"),
      fixture.workspaceKey,
    );
  }
  async function shot(name) {
    const file = path.join(directory, `${name}.png`);
    await page.screenshot({ path: file, fullPage: true });
    screenshots.push(file);
  }
  try {
    await open();
    steps.push("real_workspace_definition_and_exact_pack");
    assert.match(await feedback.textContent(), /冻结材料包/);
    assert.equal(await page.locator("#topic-count-all").textContent(), "3");
    for (const role of ["support", "challenge", "boundary"]) {
      const material = fixture.materials.find((item) => item.role === role);
      assert(material, `missing ${role} material`);
      await page.locator(`[data-role-filter="${role}"]`).click();
      assert.equal(await list.count(), 1);
      assert.equal(await list.first().getAttribute("data-ref"), material.ref);
      assert.equal(await list.first().getAttribute("aria-selected"), "true");
      assert.match(await inspector.textContent(), new RegExp(material.ref));
      assert.equal(
        await page.locator("#topic-inspector-link").getAttribute("href"),
        materialUrl(material.ref),
      );
      assert.equal(
        await page.locator(`#topic-count-${role}`).textContent(),
        "1",
      );
      steps.push(`frozen_${role}_filter_and_exact_inspector`);
    }
    const boundary = fixture.materials.find((item) => item.role === "boundary");
    assert(boundary);
    assert.equal(
      await page.locator("#topic-inspector-title").textContent(),
      "标题未知",
    );
    const unknownFacts = await inspector.locator("dd").allTextContents();
    assert(
      unknownFacts.filter((value) => value === "未知").length >= 2,
      "unknown creator and platform date remain unknown",
    );
    await page.locator('[data-role-filter="all"]').click();
    assert.equal(await list.count(), 3);
    assert.equal(
      await page
        .locator('[role="option"][aria-selected="true"]')
        .getAttribute("data-ref"),
      boundary.ref,
    );
    await list.first().click();
    assert.equal(
      await page.locator("#topic-inspector-link").getAttribute("href"),
      materialUrl(fixture.materials[0].ref),
    );
    assert.equal(
      await page.locator('img[src="x"],script:not([src])').count(),
      0,
      "source markup is never parsed as HTML",
    );
    steps.push("selection_survives_filter_and_source_markup_is_text");

    const detailResponse = await page.goto(
      new URL(materialUrl(fixture.materials[0].ref), baseURL).href,
    );
    assert(detailResponse);
    // The existing Workspace link has no ObservationDomain identity. Keep its exact
    // behavior and verify the API rejects absent context; never invent a domain from domainKey.
    assert.equal(detailResponse.status(), 400);
    assert.equal((await detailResponse.json()).code, "domain_required");
    steps.push("existing_detail_link_keeps_explicit_domain_required_boundary");
    const bounded = await page.request.get(
      new URL(
        `${materialUrl(fixture.materials[0].ref)}?domain=${fixture.domain}`,
        baseURL,
      ).href,
    );
    assert.equal(bounded.status(), 200);
    assert.equal(
      (await bounded.json()).item.identity.publicRef,
      fixture.materials[0].ref,
    );
    steps.push("exact_work_resource_json_with_explicit_fixture_domain");

    for (const width of [1440, 1280, 1024, 390]) {
      await page.setViewportSize({ width, height: 1000 });
      await open();
      const geometry = await page.evaluate(() => ({
        viewport: window.innerWidth,
        scroll: document.documentElement.scrollWidth,
      }));
      assert(
        geometry.scroll <= geometry.viewport,
        `Workspace ${width}px document overflows: ${JSON.stringify(geometry)}`,
      );
      await shot(`ready-${width}`);
    }
    steps.push(
      "four_viewport_actual_react_screenshots_without_document_overflow",
    );

    const missingRead = page.waitForResponse(
      (response) =>
        new URL(response.url()).pathname ===
        `/api/local/topic-workspaces/${fixture.missingWorkspaceKey}`,
    );
    await page.goto(
      new URL(`/topics/${fixture.missingWorkspaceKey}`, baseURL).href,
      { waitUntil: "networkidle" },
    );
    assert.equal((await missingRead).status(), 404);
    await page.locator('#topic-feedback[data-state="unavailable"]').waitFor();
    assert.match(await feedback.textContent(), /topic_workspace_not_found/);
    assert.equal(await list.count(), 0);
    assert.equal(await page.locator("#topic-count-all").textContent(), "—");
    assert.equal(
      await page.locator("#topic-inspector-link").getAttribute("hidden"),
      "",
    );
    assert.equal(
      await page.locator("#topic-inspector-link").getAttribute("href"),
      null,
    );
    await shot("not-found-390");
    steps.push("real_404_never_becomes_zero_or_previous_pack");
    const invalid = await page.goto(
      new URL("/topics/INVALID-key", baseURL).href,
    );
    assert(invalid);
    assert.equal(invalid.status(), 400);
    steps.push("invalid_canonical_key_keeps_server_guard");
    assert.deepEqual(
      writes,
      [],
      "Workspace interaction performs no write requests",
    );
    return { steps, screenshots, writes };
  } catch (error) {
    await page
      .screenshot({ path: path.join(directory, "failure.png"), fullPage: true })
      .catch(() => {});
    throw error;
  } finally {
    page.off("request", observe);
  }
}
