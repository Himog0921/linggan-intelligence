import { useState, type FormEvent } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { ActionButton } from "../../shared/ActionButton";
import { ApiError } from "../../shared/api";
import * as api from "./api";
import { Dialog } from "./ui";
import { c } from "./styles";

function responsibilityError(code: string | null) {
  switch (code) {
    case "ops_native_forbidden": return "当前权限无法完成这次操作。请由管理员处理。";
    case "ops_native_responsibility_version_stale": return "账号负责人已变化。关闭窗口并刷新账号列表后重试。";
    case "ops_native_responsibility_target_inactive":
    case "ops_native_responsibility_target_not_found": return "接任成员已不可用。关闭窗口并刷新成员列表后重试。";
    default: return "未能保存账号负责人。请核对账号和接任成员后重试。";
  }
}

export function AccountResponsibilityDialog({
  account, members, close, suggestedRecipient, canUnassign, onSaved,
}: {
  account: api.Account;
  members: api.ActiveMember[];
  close: () => void;
  suggestedRecipient?: string;
  canUnassign: boolean;
  onSaved: (message: string) => void;
}) {
  const client = useQueryClient();
  const [recipient, setRecipient] = useState(
    members.find(member => member.memberRef === suggestedRecipient)?.memberRef
      ?? account.currentResponsibleMemberRef ?? "",
  );
  const [reason, setReason] = useState("");
  const [submitting, setSubmitting] = useState(false);
  const [pending, setPending] = useState<{
    command: api.NativeAccountResponsibilityCommand & { requestId: string };
    recipientName: string | null;
  } | null>(null);
  const [message, setMessage] = useState("");
  const currentLabel = account.currentResponsibleName || (account.pendingFromMemberRef ? "待接管" : "未分配");
  const action = account.currentResponsibleMemberRef ? "交接账号" : account.pendingFromMemberRef ? "接管账号" : "分配账号";
  const selectedMember = members.find(member => member.memberRef === recipient);
  const mayUnassign = canUnassign && !!account.currentResponsibleMemberRef && recipient === "unassigned";
  const valid = !!reason.trim() && (mayUnassign || !!selectedMember && recipient !== account.currentResponsibleMemberRef);

  async function submit(event: FormEvent) {
    event.preventDefault();
    if (submitting || (!pending && !valid)) return;
    const request = pending ?? {
      command: {
        requestId: crypto.randomUUID(),
        nextMemberRef: selectedMember?.memberRef ?? null,
        expectedVersion: account.responsibilityVersion,
        reason: reason.trim(),
      },
      recipientName: selectedMember?.displayName ?? null,
    };
    setPending(request);
    setSubmitting(true);
    setMessage("");
    try {
      const receipt = await api.transferNativeAccountResponsibility(account.ref, request.command);
      if (receipt.outcome !== "accepted" && receipt.outcome !== "unchanged") {
        setPending(null);
        setMessage(responsibilityError(receipt.code));
        return;
      }
      onSaved(request.recipientName ? `${account.name}的负责人已更新为${request.recipientName}。` : `${account.name}已取消分配。`);
      void Promise.all([
        client.invalidateQueries({ queryKey: ["ops", "accounts"] }),
        client.invalidateQueries({ queryKey: ["ops", "publications"] }),
        client.invalidateQueries({ queryKey: ["ops-auth", "members"] }),
      ]);
      close();
    } catch (error) {
      if (error instanceof ApiError && error.failure.kind === "http" && error.failure.status < 500) {
        setPending(null);
        setMessage(error.failure.status === 401 ? "登录已失效。请重新登录后核对账号状态。" : responsibilityError(error.failure.code));
      } else {
        setMessage("结果暂未确认。再次提交会使用同一请求核对结果。");
      }
    } finally {
      setSubmitting(false);
    }
  }
  return <Dialog label={action} className="ops-responsibility-dialog" onClose={() => { if (!submitting) close(); }}>
    <header><h2>{action}</h2><p className={c("ops-responsibility-account")}>{account.name}</p></header>
    <form onSubmit={submit}>
      <div className={c("ops-dialog-body", "ops-responsibility-fields")}>
        <p className={c("ops-note")}>当前负责人 · {currentLabel}</p>
        <label>接任成员
          <select value={recipient} required onChange={event => setRecipient(event.target.value)} disabled={!!pending}>
            <option value="" disabled>选择接任成员</option>
            {members.map(member => <option key={member.memberRef} value={member.memberRef}>
              {member.displayName}（{member.role === "admin" ? "管理员" : member.role === "lead" ? "组长" : "运营"}）
            </option>)}
            {canUnassign && account.currentResponsibleMemberRef ? <option value="unassigned">取消分配</option> : null}
          </select>
        </label>
        <label>变更原因
          <input required maxLength={1000} value={reason} onChange={event => setReason(event.target.value)} disabled={!!pending} />
        </label>
        <p className={c("ops-note")}>历史发布继续显示原发布人。</p>
        {message ? <p className={c("ops-form-error")} role="alert">{message}</p> : null}
      </div>
      <footer>
        <ActionButton onClick={close} disabled={submitting}>取消</ActionButton>
        <ActionButton variant="primary" type="submit" busy={submitting} disabled={!pending && !valid}>{pending ? "核对并重试" : `确认${action}`}</ActionButton>
      </footer>
    </form>
  </Dialog>;
}
