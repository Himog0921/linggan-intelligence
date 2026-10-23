import { useEffect, useState } from "react";
import { ActionButton } from "../../shared/ActionButton";
import * as api from "./api";
import { c } from "./styles";
import { Dialog, Tag } from "./ui";

const decisionLabel: Record<api.PublicationSyncReceipt["items"][number]["decision"], string> = {
  accepted: "已建立采集工作",
  existing: "复用现有采集工作",
  blocked: "未建立工作",
  forbidden: "无此操作权限",
  invalid: "发布记录不可用",
  conflict: "当前记录冲突",
};
const executionLabel: Record<api.PublicationSyncReceipt["items"][number]["execution"]["state"], string> = {
  not_requested: "尚未提交同步",
  not_submitted: "未进入采集链",
  queued: "等待分派",
  waiting_execution: "等待执行工位",
  awaiting_collection: "已分派，等待采集",
  collecting: "正在采集",
  package_pending_receipt: "回包等待核验",
  receipt_accepted_projection_pending: "回执合格，等待材料关联",
  receipt_lost_authority: "回执失去授权",
  receipt_accepted: "回执已接受",
  material_projected: "已关联材料（字段尚未采用）",
  no_qualified_result: "未得到合格结果",
  stopped: "执行已停止",
  chain_unavailable: "执行链状态暂不可读",
};
function tone(item: api.PublicationSyncReceipt["items"][number]) {
  if (item.execution.state === "material_projected") return "success";
  if (["blocked", "forbidden", "invalid", "conflict"].includes(item.decision) || ["receipt_lost_authority", "stopped", "no_qualified_result"].includes(item.execution.state)) return "warning";
  return undefined;
}
function ref(value: string | null) { return value ? value.slice(0, 8) : null; }

export function PublicationSyncDialog({ publications, requestRef, close }: { publications: readonly api.Publication[]; requestRef?: string; close: () => void }) {
  const [selected, setSelected] = useState<readonly string[]>([]);
  const [submitting, setSubmitting] = useState(false);
  const [refreshing, setRefreshing] = useState(false);
  const [receipt, setReceipt] = useState<api.PublicationSyncReceipt | null>(null);
  const [message, setMessage] = useState("");
  useEffect(() => {
    if (!requestRef) return;
    let live = true;
    setRefreshing(true); setMessage("");
    void api.publicationSync(requestRef).then(value => {
      if (live) setReceipt(value);
    }).catch(error => {
      if (live) setMessage(`状态暂时无法读取：${api.reason(error)}`);
    }).finally(() => {
      if (live) setRefreshing(false);
    });
    return () => { live = false; };
  }, [requestRef]);
  function toggle(publicationRef: string) {
    setSelected(current => current.includes(publicationRef) ? current.filter(ref => ref !== publicationRef) : [...current, publicationRef]);
  }
  async function submit(event: React.FormEvent) {
    event.preventDefault();
    if (!selected.length || submitting) return;
    setSubmitting(true); setMessage("");
    try {
      setReceipt(await api.createPublicationSync(selected));
    } catch (error) {
      setMessage(`命令未确认：${api.reason(error)}`);
    } finally { setSubmitting(false); }
  }
  async function refresh() {
    if (!receipt || refreshing) return;
    setRefreshing(true); setMessage("");
    try { setReceipt(await api.publicationSync(receipt.requestRef)); }
    catch (error) { setMessage(`状态暂时无法读取：${api.reason(error)}`); }
    finally { setRefreshing(false); }
  }
  return <Dialog label="同步已登记发布" onClose={close} className="ops-sync-dialog">
    <form onSubmit={submit}>
      <header>
        <h2>同步已登记发布</h2>
        <small>只为选中的精确发布申请采集；不会自动发现、不会改写台账字段。</small>
      </header>
      <div className={c("ops-dialog-body")}>
        {!receipt && !requestRef ? <>
          <p className={c("ops-note", "ops-sync-intro")}>先选择本页记录。提交只会写入受控命令；是否能够排入既有采集链，由服务端逐条给出决定。</p>
          <fieldset className={c("ops-sync-list")} disabled={submitting}>
            <legend>本页发布记录（最多选择 200 条）</legend>
            {publications.map(item => <label key={item.ref} className={c("ops-sync-choice")}>
              <input type="checkbox" checked={selected.includes(item.ref)} onChange={() => toggle(item.ref)} />
              <span><strong>{item.title || "无标题发布"}</strong><small>{item.businessDate} · {item.platform === "xhs" ? "小红书" : "抖音"} · {item.account?.name || "账号未知"}</small></span>
            </label>)}
          </fieldset>
          {message ? <p role="status" className={c("ops-sync-error")}>{message}</p> : null}
        </> : receipt ? <SyncResult receipt={receipt} refresh={refresh} refreshing={refreshing} /> : <p role="status" className={c("ops-note")}>{refreshing ? "正在读取同步状态…" : "同步状态暂时无法读取。"}</p>}
        {receipt && message ? <p role="status" className={c("ops-sync-error")}>{message}</p> : null}
      </div>
      <footer>
        <ActionButton type="button" onClick={close} disabled={submitting}>{receipt ? "关闭" : "取消"}</ActionButton>
        {!receipt && !requestRef ? <ActionButton type="submit" disabled={!selected.length} busy={submitting}>{selected.length ? `提交 ${selected.length} 条` : "提交"}</ActionButton> : <ActionButton type="button" onClick={refresh} disabled={!receipt} busy={refreshing}>刷新执行状态</ActionButton>}
      </footer>
    </form>
  </Dialog>;
}

function SyncResult({ receipt, refresh, refreshing }: { receipt: api.PublicationSyncReceipt; refresh: () => void; refreshing: boolean }) {
  return <section className={c("ops-sync-result")} aria-live="polite">
    <p className={c("ops-sync-receipt")}>命令回执 {ref(receipt.requestRef)} · {receipt.replayed ? "已重放原命令" : "已写入本次命令"} · 共 {receipt.selectedCount} 条</p>
    <p className={c("ops-note")}>“已关联材料”只表示 Receipt→Material 关联成立；运营字段和媒体仍未采用。{refreshing ? " 正在刷新…" : ""}</p>
    <ul>
      {receipt.items.map(item => <li key={item.publicationRef}>
        <div><Tag tone={tone(item)}>{decisionLabel[item.decision]}</Tag><strong>{executionLabel[item.execution.state]}</strong></div>
        <small>{item.code}{item.workOrderRef ? ` · 工作单 ${ref(item.workOrderRef)}` : ""}{item.execution.receiptRef ? ` · 回执 ${ref(item.execution.receiptRef)}` : ""}</small>
      </li>)}
    </ul>
  </section>;
}
