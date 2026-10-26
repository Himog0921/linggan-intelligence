import { useState } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { ActionButton } from "../../shared/ActionButton";
import * as api from "./api";
import { Dialog } from "./ui";
import { c } from "./styles";

/**
 * The roster is a convenience selector only.  The server recomputes the actor's current group,
 * the account version and the selected member's active state inside the transfer transaction.
 */
export function AccountResponsibilityDialog({
  account,
  members,
  close,
}: {
  account: api.Account;
  members: api.ActiveMember[];
  close: () => void;
}) {
  const client = useQueryClient();
  const [nextMemberRef, setNextMemberRef] = useState(account.currentResponsibleMemberRef || "");
  const [reason, setReason] = useState("");
  const [submitting, setSubmitting] = useState(false);
  const [message, setMessage] = useState("");
  const currentLabel = account.currentResponsibleName || "待接管";
  async function submit(event: React.FormEvent) {
    event.preventDefault();
    if (submitting) return;
    setSubmitting(true); setMessage("");
    try {
      const receipt = await api.transferNativeAccountResponsibility(account.ref, {
        requestId: crypto.randomUUID(),
        nextMemberRef: nextMemberRef || null,
        expectedVersion: account.responsibilityVersion,
        reason,
      });
      if (receipt.outcome !== "accepted" && receipt.outcome !== "unchanged") {
        setMessage(`未完成交接：${receipt.code}`); return;
      }
      await client.invalidateQueries({ queryKey: ["ops", "accounts"] });
      close();
    } catch (error) {
      setMessage(`交接未确认：${api.reason(error)}`);
    } finally { setSubmitting(false); }
  }
  return <Dialog label="交接账号当前负责人" onClose={close}>
    <form className={c("ops-dialog-body")} onSubmit={submit}>
      <h2>交接账号当前负责人</h2>
      <p className={c("ops-note")}>
        当前负责人：{currentLabel}。历史运营人、历史发布人和业务归属不会随此操作改变。
      </p>
      <label>接任成员
        <select value={nextMemberRef} onChange={event => setNextMemberRef(event.target.value)} disabled={submitting}>
          <option value="">待接管（仅管理员）</option>
          {members.map(member => <option key={member.memberRef} value={member.memberRef}>
            {member.displayName}（{member.role === "admin" ? "管理员" : member.role === "lead" ? "组长" : "运营"}）
          </option>)}
        </select>
      </label>
      <label>交接原因
        <input required maxLength={1000} value={reason} onChange={event => setReason(event.target.value)} disabled={submitting} />
      </label>
      <p className={c("ops-note")}>保存时会重新核对当前权限、同组边界和责任版本；若期间已有交接，本次不会覆盖。</p>
      {message ? <p role="status">{message}</p> : null}
      <footer><ActionButton type="button" onClick={close} disabled={submitting}>取消</ActionButton><ActionButton type="submit" disabled={submitting}>{submitting ? "正在提交…" : "确认交接"}</ActionButton></footer>
    </form>
  </Dialog>;
}
