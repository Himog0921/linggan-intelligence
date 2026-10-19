import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { TopicLibrary } from "./TopicLibrary";
import { TopicDetail } from "./TopicDetail";
import * as api from "./workbenchApi";
import { newDocument } from "./workbenchModel";
vi.mock("./workbenchApi");
const id = "00000000-0000-4000-8000-000000000001";
const works = [
  {
    workId: id,
    domainRef: id,
    title: "真实标题",
    revisionId: id,
    savedAt: "2026-10-15T12:00:00Z",
  },
];
function library() {
  return render(
    <QueryClientProvider
      client={
        new QueryClient({ defaultOptions: { queries: { retry: false } } })
      }
    >
      <TopicLibrary section="works" works={works} onOpen={vi.fn()} />
    </QueryClientProvider>,
  );
}
beforeEach(() => vi.clearAllMocks());
describe("unknown values remain unknown on topic surfaces", () => {
  it("does not turn an unread draft into zero words or an unwritten match", async () => {
    vi.mocked(api.readWork).mockImplementation(
      () => new Promise(() => undefined),
    );
    library();
    expect(await screen.findByText("正在读取稿件详情…")).toBeInTheDocument();
    expect(screen.queryByText(/0 字/)).not.toBeInTheDocument();
    expect(screen.queryByText("尚未开始正文")).not.toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "待创作" }));
    expect(screen.queryByText("真实标题")).not.toBeInTheDocument();
    expect(screen.getByText(/筛选结果尚不完整/)).toBeInTheDocument();
    expect(screen.queryByText(/没有匹配的方案/)).not.toBeInTheDocument();
  });
  it("offers a retry for failed draft details without inventing progress", async () => {
    vi.mocked(api.readWork).mockRejectedValue(new Error("failed"));
    library();
    const retry = await screen.findByRole("button", { name: "重试读取方案" });
    expect(screen.getByText(/读取失败，进度未知/)).toBeInTheDocument();
    expect(screen.queryByText(/0 字/)).not.toBeInTheDocument();
    fireEvent.click(retry);
    await waitFor(() => expect(api.readWork).toHaveBeenCalledTimes(2));
  });
  it("only reports no accounts after a successful empty read and labels known platforms in Chinese", () => {
    const props = {
      document: newDocument("标题"),
      accounts: [],
      onRetryAccounts: vi.fn(),
      onChange: vi.fn(),
      onCompose: vi.fn(),
      onRead: vi.fn(),
      onWrite: vi.fn(),
    };
    const view = render(<TopicDetail {...props} accountsState="pending" />);
    expect(screen.getByText("正在读取所属账号…")).toBeInTheDocument();
    expect(screen.queryByText(/还没有运营账号/)).not.toBeInTheDocument();
    view.rerender(
      <TopicDetail {...props} accountsState="error" accountError="读取失败" />,
    );
    expect(
      screen.getByRole("button", { name: "重新读取账号" }),
    ).toBeInTheDocument();
    expect(screen.queryByText(/还没有运营账号/)).not.toBeInTheDocument();
    view.rerender(<TopicDetail {...props} accountsState="success" />);
    expect(screen.getByText(/还没有运营账号/)).toBeInTheDocument();
    view.rerender(
      <TopicDetail
        {...props}
        accountsState="success"
        accounts={[{ ref: id, name: "驭A低手裴妈", platform: "xhs" }]}
      />,
    );
    expect(
      screen.getByRole("option", { name: "驭A低手裴妈 · 小红书" }),
    ).toBeInTheDocument();
  });
});
