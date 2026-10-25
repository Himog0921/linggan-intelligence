import { useRef, useState } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { ActionButton } from "../../shared/ActionButton";
import { ApiError } from "../../shared/api";
import * as api from "./api";
import { Dialog } from "./ui";
import { c } from "./styles";

function nullable(value: string) { return value.trim() || null; }
function count(value: string): number | null | undefined {
  if (!value.trim()) return null;
  const parsed = Number(value);
  return Number.isSafeInteger(parsed) && parsed >= 0 ? parsed : undefined;
}
function useReceiptCommand(close: () => void) {
  const client = useQueryClient();
  const requestId = useRef<string | null>(null);
  const [submitting, setSubmitting] = useState(false);
  const [retry, setRetry] = useState(false);
  const [message, setMessage] = useState("");
  async function submit(run: (requestId: string) => ReturnType<typeof api.updateNativeAccount>) {
    if (submitting) return;
    const id = requestId.current || crypto.randomUUID(); requestId.current = id;
    setSubmitting(true); setMessage("");
    try {
      const receipt = await run(id);
      if (receipt.outcome === "accepted" || receipt.outcome === "unchanged") {
        requestId.current = null; setRetry(false);
        await Promise.all([client.invalidateQueries({ queryKey: ["ops", "accounts"] }), client.invalidateQueries({ queryKey: ["ops", "summary"] })]);
        close(); return;
      }
      requestId.current = null; setRetry(false);
      setMessage(receipt.outcome === "stale" ? "记录已被其他操作更新，未覆盖。列表已刷新，请重新核对。" : `服务器拒绝：${receipt.code}`);
      if (receipt.outcome === "stale") await client.invalidateQueries({ queryKey: ["ops", "accounts"] });
    } catch (error) {
      if (error instanceof ApiError && !error.answered) {
        setRetry(true); setMessage("提交结果未确认。字段已锁定，请重试同一请求以避免重复操作。");
      } else { requestId.current = null; setRetry(false); setMessage(`操作未完成：${api.reason(error)}`); }
    } finally { setSubmitting(false); }
  }
  return { submitting, retry, message, locked: submitting || retry, submit };
}

export function NativeAccountEditDialog({ account, close }: { account: api.Account; close: () => void }) {
  const [name, setName] = useState(account.name); const [profileUrl, setProfileUrl] = useState(account.profileUrl || "");
  const [handle, setHandle] = useState(account.handle || ""); const [status, setStatus] = useState<"active" | "warming" | "paused">(account.status === "warming" || account.status === "paused" ? account.status : "active");
  const [retained, setRetained] = useState(account.isRetained ?? true); const [followers, setFollowers] = useState(account.followerCount?.toString() || "");
  const [deviceCode, setDeviceCode] = useState(account.deviceCode || ""); const [ipLocation, setIpLocation] = useState(account.ipLocation || ""); const [notes, setNotes] = useState(account.notes || "");
  const command = useReceiptCommand(close);
  function onSubmit(event: React.FormEvent) {
    event.preventDefault(); const followerCount = count(followers);
    if (followerCount === undefined) return;
    void command.submit(requestId => api.updateNativeAccount(account.ref, { requestId, expectedVersion: account.nativeVersion!, accountName: name, profileUrl: nullable(profileUrl), handle: nullable(handle), status, isRetained: retained, followerCount, deviceCode: nullable(deviceCode), ipLocation: nullable(ipLocation), notes: nullable(notes) }));
  }
  return <Dialog label="编辑原生账号" onClose={close}><form className={c("ops-dialog-body")} onSubmit={onSubmit}>
    <h2>编辑账号资料</h2><p className={c("ops-note")}>平台和平台账号身份不能在普通编辑中变更；这会避免把历史归属或外部身份一起改掉。</p>
    <label>账号名称<input required maxLength={512} value={name} onChange={e => setName(e.target.value)} disabled={command.locked} /></label>
    <label>主页链接<input type="url" value={profileUrl} onChange={e => setProfileUrl(e.target.value)} disabled={command.locked} /></label><label>账号号 / Handle<input maxLength={512} value={handle} onChange={e => setHandle(e.target.value)} disabled={command.locked} /></label>
    <label>账号状态<select value={status} onChange={e => setStatus(e.target.value as typeof status)} disabled={command.locked}><option value="active">运营中</option><option value="warming">养号中</option><option value="paused">暂停</option></select></label>
    <label>粉丝数（可选）<input type="number" min="0" step="1" value={followers} onChange={e => setFollowers(e.target.value)} disabled={command.locked} /></label><label>设备编号<input maxLength={512} value={deviceCode} onChange={e => setDeviceCode(e.target.value)} disabled={command.locked} /></label><label>IP 所在地<input maxLength={512} value={ipLocation} onChange={e => setIpLocation(e.target.value)} disabled={command.locked} /></label>
    <label><input type="checkbox" checked={retained} onChange={e => setRetained(e.target.checked)} disabled={command.locked} /> 保留账号</label><label>备注<textarea maxLength={4000} value={notes} onChange={e => setNotes(e.target.value)} disabled={command.locked} /></label>
    {command.message ? <p role="status">{command.message}</p> : null}<footer><ActionButton type="button" onClick={close} disabled={command.submitting}>取消</ActionButton><ActionButton type="submit" disabled={command.submitting}>{command.submitting ? "正在保存…" : command.retry ? "重试同一请求" : "保存资料"}</ActionButton></footer>
  </form></Dialog>;
}

export function NativeAccountArchiveDialog({ account, restore, close }: { account: api.Account; restore: boolean; close: () => void }) {
  const [reason, setReason] = useState(""); const command = useReceiptCommand(close);
  function onSubmit(event: React.FormEvent) { event.preventDefault(); void command.submit(requestId => (restore ? api.restoreNativeAccount : api.archiveNativeAccount)(account.ref, { requestId, expectedVersion: account.nativeVersion!, reason })); }
  const verb = restore ? "恢复" : "归档";
  return <Dialog label={`${verb}原生账号`} onClose={close}><form className={c("ops-dialog-body")} onSubmit={onSubmit}>
    <h2>{verb}账号</h2><p className={c("ops-note")}>{restore ? "恢复后可再次登记发布；不会补造历史发布或责任记录。" : "归档后不再可登记发布或编辑；历史记录和原生回执保留。"}</p>
    <label>{verb}原因<input required maxLength={1000} value={reason} onChange={e => setReason(e.target.value)} disabled={command.locked} /></label>{command.message ? <p role="status">{command.message}</p> : null}
    <footer><ActionButton type="button" onClick={close} disabled={command.submitting}>取消</ActionButton><ActionButton type="submit" disabled={command.submitting}>{command.submitting ? "正在提交…" : command.retry ? "重试同一请求" : `确认${verb}`}</ActionButton></footer>
  </form></Dialog>;
}
