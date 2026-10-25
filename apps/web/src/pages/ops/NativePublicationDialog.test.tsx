import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { expect, it, vi } from "vitest";
import * as api from "./api";
import { NativePublicationDialog } from "./NativePublicationDialog";

vi.mock("./api", async () => ({
  ...(await vi.importActual<typeof import("./api")>("./api")),
  createNativePublicationBatch: vi.fn(),
}));

const account = {
  ref: "account", source: "native" as const, platform: "xhs" as const, name: "合成账号",
  profileUrl: null, deviceCode: null, followerCount: null, ipLocation: null,
  currentResponsibleMemberRef: null, currentResponsibleName: null, responsibilityVersion: 1,
  nativeVersion: 1, nativeState: "active" as const, handle: null,
  operator: null, status: null, health: "not_filled" as const, isRetained: null, notes: null,
};

it("retains only failed publication batch rows for a targeted retry", async () => {
  vi.mocked(api.createNativePublicationBatch).mockResolvedValue({ items: [
    { ordinal: 1, status: 201, requestRef: "one", outcome: "accepted" as const, code: "accepted", replayed: false },
    { ordinal: 2, status: 422, requestRef: "two", outcome: "rejected" as const, code: "ops_native_publication_date_invalid", replayed: false },
  ] });
  render(<QueryClientProvider client={new QueryClient()}><NativePublicationDialog accounts={[account]} close={vi.fn()} /></QueryClientProvider>);
  fireEvent.change(screen.getByLabelText("发布日期"), { target: { value: "2026-10-23" } });
  const batch = screen.getByLabelText(/批量发布链接/);
  fireEvent.change(batch, { target: { value: "https://example.test/one\nhttps://example.test/two" } });
  fireEvent.click(screen.getByRole("button", { name: "登记发布" }));
  await waitFor(() => expect(api.createNativePublicationBatch).toHaveBeenCalledOnce());
  expect(screen.getByRole("status")).toHaveTextContent("已登记 1 条；失败行保留");
  expect(batch).toHaveValue("https://example.test/two");
  expect(screen.getByLabelText("批量登记结果")).toHaveTextContent("ops_native_publication_date_invalid");
});
