import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { act, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { createMemoryRouter, RouterProvider } from "react-router";
import { afterEach, describe, expect, it, vi } from "vitest";
import { WorkspacePage } from "./WorkspacePage";
import { workspaceFixture } from "./workspaceTestFixture";

function response(body: unknown, status = 200) {
  return new Response(JSON.stringify(body), {
    status,
    headers: { "Content-Type": "application/json" },
  });
}
function page(path = "/topics/workspace-proof") {
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  const router = createMemoryRouter(
    [{ path: "/topics/:canonical_key", element: <WorkspacePage /> }],
    { initialEntries: [path] },
  );
  return {
    ...render(
      <QueryClientProvider client={client}>
        <RouterProvider router={router} />
      </QueryClientProvider>,
    ),
    router,
    client,
  };
}
function deferredResponse() {
  let complete: (value: Response) => void = () => {
    throw new Error("uninitialized deferred");
  };
  const pending = new Promise<Response>((resolve) => {
    complete = resolve;
  });
  return { pending, complete };
}
afterEach(() => {
  vi.unstubAllGlobals();
});

describe("independent Topic workspace", () => {
  it("labels machine-produced candidates from runKind without claiming human adjudication", async () => {
    const input = workspaceFixture();
    input.topic.classificationRun.runKind = 'machine_proposed';
    input.topic.definition.lifecycleState = 'candidate';
    vi.stubGlobal('fetch',vi.fn(async () => response(input)));
    page();
    await screen.findByRole('heading',{name:'合成工作区 workspace-proof'});
    expect(document.getElementById('topic-run-kind')).toHaveTextContent('机器候选 / MACHINE PROPOSED');
    expect(screen.getByText('机器分类说明')).toBeVisible();
    expect(screen.queryByText('人工裁定说明')).not.toBeInTheDocument();
    expect(screen.queryByText('人工',{exact:true})).not.toBeInTheDocument();
    expect(screen.getByText('候选主题')).toBeVisible();
    expect(screen.getByRole('complementary', { name: 'Topic 研究导航' })).toHaveTextContent('候选主题');
    expect(screen.getAllByText('CANDIDATE TOPIC')).toHaveLength(2);
    expect(screen.getByText('已读取当前候选版本')).toBeVisible();
    expect(screen.getByRole('button', { name: '全部分类 3' })).toBeVisible();
    expect(screen.getByText('分类角色')).toBeVisible();
    expect(screen.queryByText(/PROVISIONAL TOPIC|已读取当前暂定版本|裁定角色/)).not.toBeInTheDocument();
  });

  it("renders definition, unknown sources, role filtering, Inspector and JSON identity", async () => {
    const fetcher = vi.fn(async () => response(workspaceFixture()));
    vi.stubGlobal("fetch", fetcher);
    page();
    expect(
      await screen.findByRole("heading", {
        name: "合成工作区 workspace-proof",
      }),
    ).toBeInTheDocument();
    expect(document.title).toBe("Topic 工作区 · Linggan Intelligence");
    expect(screen.getByText("VERSION 2")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "全部裁定 3" })).toHaveAttribute(
      "aria-pressed",
      "true",
    );
    expect(
      within(
        screen.getByRole("complementary", { name: "标题未知" }),
      ).getAllByText("未知"),
    ).toHaveLength(2);
    expect(
      screen.getByRole("link", { name: "打开 Work Resource JSON" }),
    ).toHaveAttribute(
      "href",
      "/api/local/topic-workspaces/workspace-proof/materials/10000000-0000-4000-8000-000000000001",
    );
    await userEvent.click(screen.getByRole("button", { name: "挑战材料 1" }));
    expect(screen.getAllByRole("option")).toHaveLength(1);
    expect(screen.getByRole("option")).toHaveAttribute("aria-selected", "true");
    expect(screen.getByText("平台原始相对时间")).toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: "全部裁定 3" }));
    expect(screen.getByRole("option", { selected: true })).toHaveAttribute(
      "data-ref",
      "10000000-0000-4000-8000-000000000002",
    );
    const third = screen.getAllByRole("option")[2];
    if (!third) throw new Error("third material missing");
    await userEvent.click(third);
    expect(
      screen.getByRole("link", { name: "打开 Work Resource JSON" }),
    ).toHaveAttribute(
      "href",
      "/api/local/topic-workspaces/workspace-proof/materials/10000000-0000-4000-8000-000000000003",
    );
    expect(fetcher).toHaveBeenCalledTimes(1);
  });
  it("keeps existing machine candidate workspaces readable", async () => {
    const key = "candidate-10000000000040008000000000000001";
    const value = workspaceFixture(key);
    value.topic.definition.lifecycleState = "candidate";
    value.topic.classificationRun.runKind = "machine_proposed";
    vi.stubGlobal("fetch", vi.fn(async () => response(value)));
    page(`/topics/${key}`);
    expect(await screen.findByRole("heading", {
      name: `合成工作区 ${key}`,
    })).toBeInTheDocument();
    expect(screen.getAllByRole("option")).toHaveLength(3);
  });
  it.each([404, 503])(
    "keeps unavailable %i distinct from zero",
    async (status) => {
      vi.stubGlobal(
        "fetch",
        vi.fn(async () =>
          response(
            {
              code:
                status === 404
                  ? "topic_workspace_not_found"
                  : "topic_workspace_unavailable",
            },
            status,
          ),
        ),
      );
      page();
      expect(
        await screen.findByText(/当前未读取 Topic（topic_workspace_/),
      ).toBeInTheDocument();
      expect(
        screen.getByRole("heading", { name: "当前未读取 Topic" }),
      ).toBeInTheDocument();
      expect(
        screen.getByRole("button", { name: "全部裁定 —" }),
      ).toBeInTheDocument();
      expect(screen.queryByRole("option")).not.toBeInTheDocument();
      expect(
        screen.queryByRole("link", { name: "打开 Work Resource JSON" }),
      ).not.toBeInTheDocument();
      expect(
        screen.queryByText(/当前裁定视角没有材料/),
      ).not.toBeInTheDocument();
    },
  );
  it("shows exact empty pack as zero and clears Inspector", async () => {
    const value = workspaceFixture();
    vi.stubGlobal(
      "fetch",
      vi.fn(async () =>
        response({
          ...value,
          topic: { ...value.topic, members: [] },
          materials: [],
        }),
      ),
    );
    page();
    expect(
      await screen.findByRole("button", { name: "全部裁定 0" }),
    ).toBeInTheDocument();
    expect(
      screen.getByText(/当前裁定视角没有材料；这不代表/),
    ).toBeInTheDocument();
    expect(
      screen.getByRole("heading", { name: "请选择一条材料" }),
    ).toBeInTheDocument();
  });
  it("rejects partial bodies without assumed source data", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn(async () =>
        response({ topic: workspaceFixture().topic, materials: [] }),
      ),
    );
    page();
    expect(await screen.findByText(/返回内容无法解析/)).toBeInTheDocument();
    expect(screen.queryByRole("option")).not.toBeInTheDocument();
  });
  it("renders source markup only as text", async () => {
    const value = workspaceFixture();
    const source = '<img src=x onerror="alert(1)"><script>bad()</script>';
    value.topic.definition.definitionText = source;
    value.materials.forEach((item) => {
      item.workResource.display.title = source;
    });
    vi.stubGlobal(
      "fetch",
      vi.fn(async () => response(value)),
    );
    page();
    expect(await screen.findAllByText(source)).toHaveLength(5);
    expect(document.querySelector('img[src="x"],script')).toBeNull();
  });
  it("cancels an old key and ignores its late answer; a new key resets filters", async () => {
    const first = deferredResponse();
    let oldSignal: AbortSignal | null | undefined;
    vi.stubGlobal(
      "fetch",
      vi.fn((path: string, init?: RequestInit) => {
        if (path.endsWith("/workspace-proof")) {
          oldSignal = init?.signal;
          return first.pending;
        }
        return Promise.resolve(
          response(workspaceFixture(path.split("/").pop())),
        );
      }),
    );
    const view = page();
    await waitFor(() => expect(oldSignal).toBeTruthy());
    await act(() => view.router.navigate("/topics/next-workspace"));
    expect(
      await screen.findByRole("heading", { name: "合成工作区 next-workspace" }),
    ).toBeInTheDocument();
    expect(oldSignal?.aborted).toBe(true);
    await userEvent.click(screen.getByRole("button", { name: "边界材料 1" }));
    await act(async () => {
      first.complete(response(workspaceFixture()));
      await first.pending;
    });
    expect(
      screen.queryByRole("heading", { name: "合成工作区 workspace-proof" }),
    ).not.toBeInTheDocument();
    await act(() => view.router.navigate("/topics/final-workspace"));
    expect(
      await screen.findByRole("heading", {
        name: "合成工作区 final-workspace",
      }),
    ).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "全部裁定 3" })).toHaveAttribute(
      "aria-pressed",
      "true",
    );
  });
  it("hides prior data while a current re-read is pending and after it fails", async () => {
    const next = deferredResponse();
    vi.stubGlobal(
      "fetch",
      vi
        .fn()
        .mockResolvedValueOnce(response(workspaceFixture()))
        .mockImplementationOnce(() => next.pending),
    );
    const view = page();
    expect(
      await screen.findByRole("heading", {
        name: "合成工作区 workspace-proof",
      }),
    ).toBeInTheDocument();
    await act(async () => {
      void view.client.invalidateQueries({
        queryKey: ["topics", "workspace", "workspace-proof"],
      });
    });
    await waitFor(() =>
      expect(screen.queryAllByRole("option")).toHaveLength(0),
    );
    await act(async () => {
      next.complete(response({ code: "topic_workspace_unavailable" }, 503));
      await next.pending;
    });
    expect(
      await screen.findByText(/topic_workspace_unavailable/),
    ).toBeInTheDocument();
    expect(screen.queryByRole("option")).not.toBeInTheDocument();
  });
});
