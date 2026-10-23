import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { expect, it, vi } from "vitest";
import * as api from "./api";
import { NativeViewDialog } from "./NativeViewDialog";

vi.mock("./api", async () => ({ ...(await vi.importActual<typeof import("./api")>("./api")), recordNativeView: vi.fn() }));

const publication = {
  ref: "publication", source: "native" as const, businessDate: "2026-10-20", dateState: "as_recorded" as const,
  platform: "xhs" as const, title: "测试发布", body: null, url: "https://example.test/post", account: null, publisher: null,
  views: { state: "pending" as const, value: null, overdue: false, hit: false }, likes: null, collects: null, comments: null,
  coverClickRate: null, systemCheck: "not_checked" as const, duplicates: [], cover: { available: false },
};

it("records an explicit zero as a value rather than clearing the reading", async () => {
  const close = vi.fn();
  vi.mocked(api.recordNativeView).mockResolvedValue({ requestRef: "request", outcome: "accepted", code: "accepted", recordRef: "publication", resultingVersion: 1, replayed: false });
  render(<QueryClientProvider client={new QueryClient()}><NativeViewDialog publication={publication} close={close} /></QueryClientProvider>);
  fireEvent.change(screen.getByLabelText("阅读量"), { target: { value: "0" } });
  fireEvent.click(screen.getByRole("button", { name: "保存阅读量" }));
  await waitFor(() => expect(api.recordNativeView).toHaveBeenCalledOnce());
  expect(api.recordNativeView).toHaveBeenCalledWith("publication", expect.objectContaining({ reportKind: "value", viewCount: 0, asOf: expect.stringMatching(/Z$/) }));
  await waitFor(() => expect(close).toHaveBeenCalledOnce());
});
