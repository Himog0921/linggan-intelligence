import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { expect, it, vi } from "vitest";
import * as api from "./api";
import { NativeDailyDialog } from "./NativeDailyDialog";

vi.mock("./api", async () => ({
  ...(await vi.importActual<typeof import("./api")>("./api")),
  recordNativeDaily: vi.fn(),
}));

it("sends blank daily metrics as null and an explicit zero as zero", async () => {
  const close = vi.fn();
  vi.mocked(api.recordNativeDaily).mockResolvedValue({
    requestRef: "request", outcome: "accepted", code: "accepted", recordRef: "daily", resultingVersion: 1, replayed: false,
  });
  render(<QueryClientProvider client={new QueryClient()}><NativeDailyDialog close={close} /></QueryClientProvider>);
  fireEvent.change(screen.getByLabelText("日期"), { target: { value: "2026-10-23" } });
  fireEvent.change(screen.getByLabelText("加微（可选）"), { target: { value: "0" } });
  fireEvent.click(screen.getByRole("button", { name: "登记日报" }));
  await waitFor(() => expect(api.recordNativeDaily).toHaveBeenCalledOnce());
  expect(api.recordNativeDaily).toHaveBeenCalledWith(expect.objectContaining({
    businessDate: "2026-10-23", linksOpenedCount: null, followersGained: 0, expectedVersion: null,
  }));
  await waitFor(() => expect(close).toHaveBeenCalledOnce());
});

it("does not overwrite an existing daily row after a stale receipt", async () => {
  vi.mocked(api.recordNativeDaily).mockResolvedValue({
    requestRef: "request", outcome: "stale", code: "ops_native_version_stale", recordRef: "daily", resultingVersion: 2, replayed: false,
  });
  render(<QueryClientProvider client={new QueryClient()}><NativeDailyDialog close={vi.fn()} /></QueryClientProvider>);
  fireEvent.change(screen.getByLabelText("日期"), { target: { value: "2026-10-23" } });
  fireEvent.click(screen.getByRole("button", { name: "登记日报" }));
  expect(await screen.findByRole("status")).toHaveTextContent("未覆盖原记录");
});

it("uses the displayed native version when correcting a daily report", async () => {
  vi.mocked(api.recordNativeDaily).mockResolvedValue({
    requestRef: "request", outcome: "accepted", code: "accepted", recordRef: "daily", resultingVersion: 3, replayed: false,
  });
  const item = { ref: "daily", source: "native" as const, nativeVersion: 2, businessDate: "2026-10-23", weekday: "五", operator: "甲", published: 0, linksOpened: null, followersGained: 0, addRate: null, hits: 0, monthFollowersGained: 0, monthPublished: 0, execution: "not_published" as const };
  render(<QueryClientProvider client={new QueryClient()}><NativeDailyDialog item={item} close={vi.fn()} /></QueryClientProvider>);
  expect(screen.getByLabelText("日期")).toBeDisabled();
  expect(screen.getByLabelText("加微（可选）")).toHaveValue(0);
  fireEvent.click(screen.getByRole("button", { name: "保存修正" }));
  await waitFor(() => expect(api.recordNativeDaily).toHaveBeenCalledOnce());
  expect(api.recordNativeDaily).toHaveBeenCalledWith(expect.objectContaining({ businessDate: "2026-10-23", linksOpenedCount: null, followersGained: 0, expectedVersion: 2 }));
});
