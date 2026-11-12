import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { expect, it, vi } from "vitest";
import { ApiError } from "../../shared/api";
import * as api from "./api";
import { AccountResponsibilityDialog } from "./AccountResponsibilityDialog";

vi.mock("./api", async () => ({
  ...await vi.importActual<typeof import("./api")>("./api"),
  transferNativeAccountResponsibility: vi.fn(),
}));

it("replays the original assignment when the recipient disappears from the list after a lost response", async () => {
  const account: api.Account = {
    ref: "account", source: "imported", platform: "xhs", name: "育儿日记", profileUrl: null,
    deviceCode: null, followerCount: null, ipLocation: null, currentResponsibleMemberRef: null,
    currentResponsibleName: null, pendingFromMemberRef: null, responsibilityVersion: 1,
    nativeVersion: null, nativeState: null, handle: null, operator: null, status: null,
    health: "not_filled", isRetained: null, notes: null,
  };
  const transfer = vi.mocked(api.transferNativeAccountResponsibility);
  transfer.mockRejectedValueOnce(new ApiError({ kind: "network" })).mockResolvedValueOnce({
    requestRef: "request", outcome: "accepted", code: "accepted", recordRef: "account", resultingVersion: 2, replayed: true,
  });
  const client = new QueryClient();
  const close = vi.fn();
  const onSaved = vi.fn();
  const page = (members: api.ActiveMember[]) => <QueryClientProvider client={client}>
    <AccountResponsibilityDialog account={account} members={members} suggestedRecipient="operator"
      canUnassign={false} close={close} onSaved={onSaved} />
  </QueryClientProvider>;
  const { rerender } = render(page([{ memberRef: "operator", displayName: "小周", role: "operator" }]));
  fireEvent.change(screen.getByLabelText("变更原因"), { target: { value: "分配测试" } });
  fireEvent.click(screen.getByRole("button", { name: "确认分配账号" }));
  await screen.findByRole("alert");
  rerender(page([]));
  fireEvent.click(screen.getByRole("button", { name: "核对并重试" }));
  await waitFor(() => expect(close).toHaveBeenCalledOnce());
  expect(transfer.mock.calls[1]).toEqual(transfer.mock.calls[0]);
  expect(onSaved).toHaveBeenCalledWith("育儿日记的负责人已更新为小周。");
});
