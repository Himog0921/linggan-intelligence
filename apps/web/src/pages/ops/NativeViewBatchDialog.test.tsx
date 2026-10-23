import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { expect, it, vi } from "vitest";
import * as api from "./api";
import { NativeViewBatchDialog } from "./NativeViewBatchDialog";

vi.mock("./api", async () => ({ ...(await vi.importActual<typeof import("./api")>("./api")), recordNativeViewBatch: vi.fn() }));

const publication = (ref: string, title: string) => ({
  ref, source: "native" as const, businessDate: "2026-10-23", dateState: "as_recorded" as const, platform: "xhs" as const, title, url: `https://example.test/${ref}`,
  publisher: null, account: null, views: { state: "pending" as const, value: null, hit: false, overdue: false }, likes: null, collects: null, comments: null,
  coverClickRate: null, body: null, systemCheck: "not_checked" as const, cover: { available: false }, duplicates: [], sync: { state: "not_requested" as const, requestRef: null, decision: null },
  media: { state: "not_requested" as const, observed: 0, available: 0, failed: 0, unavailable: 0 },
  fieldAdoption: { title: "not_provided" as const, body: "not_provided" as const, likes: "not_provided" as const, collects: "not_provided" as const, comments: "not_provided" as const, publishedAt: "not_provided" as const },
});

it("retains only answered failed view reports for a corrected targeted retry", async () => {
  vi.mocked(api.recordNativeViewBatch).mockResolvedValue({ items: [
    { ordinal: 1, status: 201, requestRef: "one", outcome: "accepted" as const, code: "accepted", replayed: false },
    { ordinal: 2, status: 422, requestRef: "two", outcome: "rejected" as const, code: "ops_native_view_count_invalid", replayed: false },
  ] });
  render(<QueryClientProvider client={new QueryClient()}><NativeViewBatchDialog publications={[publication("one", "第一条"), publication("two", "第二条")]} close={vi.fn()} /></QueryClientProvider>);
  fireEvent.click(screen.getByLabelText("选择：第一条"));
  fireEvent.change(screen.getByLabelText("阅读量：第一条"), { target: { value: "0" } });
  fireEvent.click(screen.getByLabelText("选择：第二条"));
  fireEvent.change(screen.getByLabelText("阅读量：第二条"), { target: { value: "12" } });
  fireEvent.click(screen.getByRole("button", { name: "保存批量阅读量" }));
  await waitFor(() => expect(api.recordNativeViewBatch).toHaveBeenCalledOnce());
  expect(screen.getByRole("status")).toHaveTextContent("已写入 1 条；失败项已保留");
  expect(screen.getByLabelText("选择：第一条")).not.toBeChecked();
  expect(screen.getByLabelText("选择：第二条")).toBeChecked();
  expect(screen.getByLabelText("批量阅读量结果")).toHaveTextContent("ops_native_view_count_invalid");
});
