import { businessSettingsFixture } from '../settings/aiSettingsFixture.test-support';
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { act, cleanup, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter } from "react-router";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { TopicsPage } from "./TopicsPage";
import {
  jsonResponse,
  topicContext,
  topicDomain,
  topicProgress,
  topicRef,
  topicSnapshot,
  topicWork,
} from "./topicTestFixture";
function page() {
  return render(
    <MemoryRouter>
      <QueryClientProvider
        client={
          new QueryClient({
            defaultOptions: { queries: { retry: false, gcTime: 0 } },
          })
        }
      >
        <TopicsPage />
      </QueryClientProvider>
    </MemoryRouter>,
  );
}
function defaultRead(path: string) {
  if (path === "/api/local/ai-settings") return businessSettingsFixture();
  if (path.includes("/context")) return topicContext;
  if (path.includes("topic-research")) return topicProgress();
  if (path.includes("/comments"))
    return { items: [], total: 0, nextCursor: null };
  return topicSnapshot();
}
function pendingResponse() {
  let resolve!: (value: Response) => void;
  const pending = new Promise<Response>((done) => {
    resolve = done;
  });
  return { pending, resolve };
}
beforeEach(() => {
  sessionStorage.clear();
  window.history.replaceState(
    null,
    "",
    `/topics?tm=1&domainRef=${topicDomain}`,
  );
  Object.defineProperty(HTMLDialogElement.prototype, "showModal", {
    configurable: true,
    value: function (this: HTMLDialogElement) {
      this.setAttribute("open", "");
    },
  });
  Object.defineProperty(HTMLElement.prototype, "scrollTo", {
    configurable: true,
    value: vi.fn(),
  });
});
afterEach(() => {
  cleanup();
  vi.unstubAllGlobals();
});
describe("Topic React interaction and lifetime", () => {
  it("puts the overview/journey switch in the breadcrumb, starts navigation with topics, and keeps the evidence histogram above material", async () => {
    vi.stubGlobal("fetch", vi.fn(async (path: string) => jsonResponse(defaultRead(path))));
    page();
    await screen.findByRole("button", { name: "合成材料 0" });
    const views = screen.getByRole("navigation", { name: "主题图谱视图" });
    expect(views.closest("#topic-map-root")).toBeNull();
    expect(within(views).getByRole("button", { name: "主题概览" })).toHaveAttribute("aria-pressed", "true");
    expect(screen.queryByText("领域结构")).not.toBeInTheDocument();
    const band = screen.getByLabelText("主题概览与样本表现");
    expect(within(band).getByLabelText("基础概览指标")).toBeInTheDocument();
    expect(within(band).getByLabelText("已观察内容点赞分布")).toBeInTheDocument();
    const region = document.querySelector("#topic-map-root");
    expect(region?.querySelector(".lgi-tm-material-insights")).toBeNull();
    await userEvent.click(within(views).getByRole("button", { name: "用户生命旅程" }));
    await waitFor(() => expect(document.querySelector("#topic-map-root")).toHaveAttribute("data-view", "journey"));
    expect(within(views).getByRole("button", { name: "用户生命旅程" })).toHaveAttribute("aria-pressed", "true");
    expect(screen.getByLabelText("主题概览与样本表现")).toBeInTheDocument();
    await userEvent.click(within(views).getByRole("button", { name: "主题概览" }));
    await waitFor(() => expect(document.querySelector("#topic-map-root")).toHaveAttribute("data-view", "overview"));
  });
  it("keeps complete statistics during staged material loading and edits search text", async () => {
    const full = pendingResponse();
    const summary = topicSnapshot();
    summary.works = summary.works.slice(0, 1);
    summary.scope.loadedWorkCount = 1;
    vi.stubGlobal(
      "fetch",
      vi.fn((path: string) =>
        path.includes("/overview?")
          ? full.pending
          : Promise.resolve(
              jsonResponse(
                path.includes("/summary?") ? summary : defaultRead(path),
              ),
            ),
      ),
    );
    page();
    expect(
      await screen.findByText(/正在载入其余材料与讨论/),
    ).toBeInTheDocument();
    expect(screen.getByLabelText("基础概览指标")).toHaveTextContent("2");
    full.resolve(jsonResponse(topicSnapshot()));
    await waitFor(() =>
      expect(
        screen.queryByText(/正在载入其余材料与讨论/),
      ).not.toBeInTheDocument(),
    );
    const input = screen.getByRole("textbox", { name: "查找材料与方向" });
    await userEvent.type(input, "材料 1");
    expect(input).toHaveValue("材料 1");
    await userEvent.click(screen.getByRole("button", { name: "查找" }));
    expect(new URL(window.location.href).searchParams.get("query")).toBe(
      "材料 1",
    );
    expect(
      screen.queryByRole("button", { name: "合成材料 0" }),
    ).not.toBeInTheDocument();
  });
  it("submits the editable manual form once with the exact selected source", async () => {
    const posts: Record<string, unknown>[] = [];
    vi.stubGlobal(
      "fetch",
      vi.fn(async (path: string, init?: RequestInit) => {
        if (init?.method === "POST") {
          const body: unknown = JSON.parse(String(init.body));
          if (typeof body !== "object" || body === null) throw Error("body");
          posts.push(body as Record<string, unknown>);
          return jsonResponse({
            receiptRef: "50000000-0000-4000-8000-000000000001",
            action: "saveAlternative",
            subjectRef: "60000000-0000-4000-8000-000000000001",
            revision: 1,
            persistedAt: "2026-10-16T00:00:00Z",
          });
        }
        return jsonResponse(defaultRead(path));
      }),
    );
    page();
    await screen.findByRole("button", { name: "合成材料 0" });
    const trigger = document.querySelector<HTMLButtonElement>(
      `[data-action="manual-save"][data-id="${topicWork().workRef}"]`,
    );
    if (!trigger) throw Error("manual entry");
    await userEvent.click(trigger);
    await userEvent.clear(
      screen.getByRole("textbox", { name: "备选标题 · 可继续打磨" }),
    );
    await userEvent.type(
      screen.getByRole("textbox", { name: "备选标题 · 可继续打磨" }),
      "合成新的备选",
    );
    await userEvent.type(
      screen.getByRole("textbox", { name: "切入角度" }),
      "保留明确来源",
    );
    await userEvent.type(
      screen.getByRole("textbox", { name: "这篇要回答的问题" }),
      "检验边界",
    );
    await userEvent.click(
      screen.getByRole("button", { name: "保存到我的备选" }),
    );
    await screen.findByText(/已保存 · 回执/);
    expect(posts).toHaveLength(1);
    expect(posts[0]).toMatchObject({
      title: "合成新的备选",
      evidenceWorkRefs: [topicWork().workRef],
      domainRef: topicDomain,
    });
  });
  it("closing a judge dialog prevents a late expanded read from reopening it", async () => {
    const expanded = pendingResponse();
    vi.stubGlobal(
      "fetch",
      vi.fn((path: string) =>
        path.startsWith("/api/local/topic-map?")
          ? expanded.pending
          : Promise.resolve(jsonResponse(defaultRead(path))),
      ),
    );
    page();
    await screen.findByRole("button", { name: "合成材料 0" });
    const trigger = document.querySelector<HTMLButtonElement>(
      `[data-action="judge"][data-id="${topicRef}"]`,
    );
    // The content view exposes judgment by selecting its topic first.
    const topic = document.querySelector<HTMLButtonElement>(
      `[data-action="topic"][data-id="${topicRef}"]`,
    );
    if (!topic) throw Error("topic entry");
    if (!trigger) {
      await userEvent.click(topic);
      await waitFor(() =>
        expect(document.querySelector('[data-action="judge"]')).not.toBeNull(),
      );
    }
    const judge = document.querySelector<HTMLButtonElement>(
      '[data-action="judge"]',
    );
    if (!judge) throw Error("judge");
    await userEvent.click(judge);
    await screen.findByRole("heading", { name: "选题判断" });
    await userEvent.click(
      screen.getByRole("button", { name: "关闭对话框，保留研究任务" }),
    );
    expanded.resolve(jsonResponse(topicSnapshot()));
    await waitFor(() => expect(document.querySelector("dialog")).toBeNull());
  });
  it("unmount aborts pending reads", async () => {
    const pending = pendingResponse();
    let signal: AbortSignal | null | undefined;
    vi.stubGlobal(
      "fetch",
      vi.fn((path: string, init?: RequestInit) => {
        if (path.includes("/summary?")) {
          signal = init?.signal;
          return pending.pending;
        }
        return Promise.resolve(jsonResponse(defaultRead(path)));
      }),
    );
    const view = page();
    await waitFor(() => expect(signal).toBeDefined());
    view.unmount();
    expect(signal?.aborted).toBe(true);
    pending.resolve(jsonResponse(topicSnapshot()));
  });
  it("keeps material reading available when research status fails", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn(async (path: string) =>
        path.includes("topic-research")
          ? jsonResponse({ code: "research_unavailable" }, 503)
          : jsonResponse(defaultRead(path)),
      ),
    );
    page();
    await waitFor(() =>
      expect(
        screen.getByRole("button", { name: "合成材料 0" }),
      ).toBeInTheDocument(),
    );
    expect(await screen.findByRole("button", { name: /研究进度读取失败/ })).toBeInTheDocument();
    expect(screen.queryByText("研究未设置")).not.toBeInTheDocument();
  });
  it("discards an old domain read after popstate even if the transport ignores abort", async () => {
    const old = pendingResponse();
    const otherDomain = "10000000-0000-4000-8000-000000000002";
    let oldSignal: AbortSignal | null | undefined;
    const other = topicSnapshot([topicWork(1)]);
    other.scope.domainRef = otherDomain;
    const domainRow = other.domains[0],
      topicRow = other.topics[0];
    if (!domainRow || !topicRow)
      throw new Error("Synthetic fixture has no scope");
    domainRow.domainRef = otherDomain;
    domainRow.name = "另一个合成领域";
    topicRow.domainRef = otherDomain;
    vi.stubGlobal(
      "fetch",
      vi.fn((path: string, init?: RequestInit) => {
        if (path.includes("/context"))
          return Promise.resolve(
            jsonResponse({
              ...topicContext,
              domains: [
                ...topicContext.domains,
                {
                  ...topicContext.domains[0],
                  domainRef: otherDomain,
                  name: "另一个合成领域",
                },
              ],
            }),
          );
        if (path.includes("topic-research"))
          return Promise.resolve(
            jsonResponse({
              ...topicProgress(),
              domainRef: path.includes(otherDomain) ? otherDomain : topicDomain,
            }),
          );
        if (path.includes(topicDomain)) {
          oldSignal = init?.signal;
          return old.pending;
        }
        return Promise.resolve(jsonResponse(other));
      }),
    );
    page();
    await waitFor(() => expect(oldSignal).toBeDefined());
    await act(async () => {
      window.history.pushState(
        null,
        "",
        `/topics?tm=1&domainRef=${otherDomain}`,
      );
      window.dispatchEvent(new PopStateEvent("popstate"));
    });
    expect(
      await screen.findByRole("button", { name: "合成材料 1" }),
    ).toBeInTheDocument();
    expect(oldSignal?.aborted).toBe(true);
    await act(async () => old.resolve(jsonResponse(topicSnapshot())));
    expect(
      screen.queryByRole("button", { name: "合成材料 0" }),
    ).not.toBeInTheDocument();
    expect(new URL(window.location.href).searchParams.get("domainRef")).toBe(
      otherDomain,
    );
  });
  it("keeps quoted topic names as text and attributes without changing delegated actions", async () => {
    const malicious = '" data-action="research-domain-resume" data-id="other';
    const snapshot = topicSnapshot();
    const row = snapshot.topics[0];
    if (!row) throw new Error("No synthetic topic");
    row.displayName = malicious;
    vi.stubGlobal(
      "fetch",
      vi.fn(async (path: string) =>
        jsonResponse(
          path.includes("/context")
            ? topicContext
            : path.includes("topic-research")
              ? topicProgress()
              : snapshot,
        ),
      ),
    );
    page();
    await waitFor(() => {
      for (const button of screen.getAllByTitle(malicious)) {
        expect(button).toHaveAttribute("data-action", "topic");
        expect(button).toHaveAttribute("data-id", topicRef);
      }
    });
    expect(document.querySelector('[data-id="other"]')).toBeNull();
  });
  it("disables the explicit save action while its receipt is pending", async () => {
    const submitted = pendingResponse();
    let posts = 0;
    vi.stubGlobal(
      "fetch",
      vi.fn((path: string, init?: RequestInit) => {
        if (init?.method === "POST") {
          posts++;
          return submitted.pending;
        }
        return Promise.resolve(jsonResponse(defaultRead(path)));
      }),
    );
    page();
    await waitFor(() =>
      expect(
        document.querySelector('[data-action="manual-save"]'),
      ).not.toBeNull(),
    );
    const trigger = document.querySelector<HTMLButtonElement>(
      '[data-action="manual-save"]',
    );
    if (!trigger) throw new Error("No manual entry");
    await userEvent.click(trigger);
    await userEvent.type(
      screen.getByRole("textbox", { name: "切入角度" }),
      "明确合成来源",
    );
    await userEvent.type(
      screen.getByRole("textbox", { name: "这篇要回答的问题" }),
      "界定合成范围",
    );
    const submit = screen.getByRole("button", { name: "保存到我的备选" });
    await userEvent.click(submit);
    await waitFor(() => expect(submit).toBeDisabled());
    await userEvent.click(submit);
    expect(posts).toBe(1);
    await act(async () =>
      submitted.resolve(
        jsonResponse({
          receiptRef: "50000000-0000-4000-8000-000000000001",
          action: "saveAlternative",
          subjectRef: "60000000-0000-4000-8000-000000000001",
          revision: 1,
          persistedAt: "2026-10-16T00:00:00Z",
        }),
      ),
    );
    await screen.findByText(/已保存 · 回执/);
  });
  it.each(["pending", "failed"])(
    "keeps historical research reads and centralized settings independent of a %s first map",
    async (mode) => {
      const summary = pendingResponse();
      const progress = topicProgress();
      const writes: unknown[] = [];
      const fetcher = vi.fn((path: string, init?: RequestInit) => {
        if (init?.method === "POST") {
          const body: unknown = JSON.parse(String(init.body));
          writes.push(body);
          return Promise.resolve(jsonResponse({
            ...(typeof body === "object" && body !== null ? body : {}),
            state: "configured",
          }));
        }
        if (path.includes("/summary?"))
          return mode === "pending" ? summary.pending : Promise.resolve(
            jsonResponse({ code: "topic_map_read_unavailable" }, 503),
          );
        return Promise.resolve(jsonResponse(
          path === "/api/local/ai-settings" ? businessSettingsFixture() : path.includes("/context") ? topicContext
            : path.includes("topic-research") ? progress : topicSnapshot(),
        ));
      });
      vi.stubGlobal("fetch", fetcher);
      page();
      if (mode === "failed") await screen.findByText("当前读取未成功");
      const trigger = await screen.findByRole("button", {
        name: /主题研究|研究进度读取中/,
      });
      await userEvent.click(trigger);
      expect(await screen.findByRole("dialog", {
        name: "主题研究",
      })).toBeInTheDocument();
      await screen.findByRole("button", { name: "刷新阶段回执" });
      expect(writes).toEqual([]);
      const researchReads = () => fetcher.mock.calls.filter(
        ([path]) => path.includes("/topic-research/runs?"),
      ).length;
      const readsBefore = researchReads();
      await userEvent.click(screen.getByRole("button", { name: "刷新阶段回执" }));
      await waitFor(() => expect(researchReads()).toBeGreaterThan(readsBefore));
      expect(screen.getByRole("link", { name: "去 AI 设置" })).toHaveAttribute("href", "/settings/models?view=advanced&area=work&task=topic_research");
      expect(screen.queryByLabelText("已有模型配置")).not.toBeInTheDocument();
      expect(screen.queryByRole("button", { name: "保存明确设置" })).not.toBeInTheDocument();
      expect(writes).toEqual([]);
      await userEvent.click(screen.getByRole("button", {
        name: "关闭对话框，保留研究任务",
      }));
      await waitFor(() => expect(screen.queryByRole("dialog")).not.toBeInTheDocument());
      if (mode === "pending")
        await act(async () => summary.resolve(jsonResponse(topicSnapshot())));
      expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    },
  );
  it("changes the presentation plot platform without changing the material scope or refetching", async () => {
    const snapshot = topicSnapshot();
    const platform = snapshot.statistics.platforms[0];
    if (!platform) throw new Error("No synthetic platform");
    snapshot.statistics.platforms.push({
      ...platform,
      platform: "douyin",
      medianLikes: 77,
    });
    const get = vi.fn(async (path: string) =>
      jsonResponse(
        path.includes("/context")
          ? topicContext
          : path.includes("topic-research")
            ? topicProgress()
            : snapshot,
      ),
    );
    vi.stubGlobal("fetch", get);
    page();
    await waitFor(() =>
      expect(
        screen.getByRole("button", { name: "合成材料 0" }),
      ).toBeInTheDocument(),
    );
    await userEvent.click(
      screen.getByRole("button", { name: "结构与表现" }),
    );
    const before = get.mock.calls.length;
    const selector = screen.getAllByLabelText("表现图平台")[0];
    if (!selector) throw new Error("No plot platform selector");
    await userEvent.selectOptions(selector, "douyin");
    // Rebuild the chart so a merely changed native select cannot make this pass.
    const plotTabs = screen.getAllByRole("button", { name: "分布图" });
    const structurePlotTab = plotTabs[plotTabs.length - 1];
    if (!structurePlotTab) throw new Error("No structure plot tab");
    await userEvent.click(structurePlotTab);
    await waitFor(() => {
      for (const control of screen.getAllByLabelText("表现图平台"))
        expect(control).toHaveValue("douyin");
    });
    expect(get).toHaveBeenCalledTimes(before);
    expect(
      new URL(window.location.href).searchParams.get("platform"),
    ).toBeNull();
  });
  it("keeps materials readable and hides restricted alternative text and export", async () => {
    const alternative = {
      alternativeRef: "60000000-0000-4000-8000-000000000001",
      domainRef: topicDomain,
      topicRef: null,
      definitionRef: null,
      sourceState: "source_unavailable",
      title: null,
      angle: null,
      rationale: null,
      fragments: [],
      originalDefinition: null,
      methodVersion: "topic-map.manual.v1",
    };
    const body = { ...topicSnapshot(), alternatives: [alternative] };
    vi.stubGlobal(
      "fetch",
      vi.fn(async (path: string) =>
        jsonResponse(
          path.includes("/alternative?")
            ? alternative
            : path.includes("/context")
              ? topicContext
              : path.includes("topic-research")
                ? topicProgress()
                : body,
        ),
      ),
    );
    page();
    await waitFor(() =>
      expect(
        screen.getByRole("button", { name: "合成材料 0" }),
      ).toBeInTheDocument(),
    );
    await userEvent.click(
      screen.getByRole("button", { name: "我的备选" }),
    );
    await screen.findByText("保存记录的来源资格已变化");
    await userEvent.click(
      screen.getByRole("button", { name: "取回保存的原版本" }),
    );
    expect(
      await screen.findByText("保存记录的来源当前不可用"),
    ).toBeInTheDocument();
    expect(
      screen.queryByRole("button", { name: "导出引用与说明" }),
    ).not.toBeInTheDocument();
  });
});
