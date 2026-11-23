import { useRef, useState } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { ApiError } from "../../shared/api";
import { ActionButton } from "../../shared/ActionButton";
import * as api from "./api";
import { Dialog } from "./ui";
import { c } from "./styles";

type PublishingAccount = Pick<api.Account, "ref" | "name" | "platform">;
type DraftRegistration = NonNullable<api.NativePublicationCommand["creation"]> & {
  title: string;
  onRegistered: (publicationRef: string) => void;
};

const draftErrors: Record<string, string> = {
  ops_native_draft_not_found: "这篇稿件不可用。请返回稿件重新打开。",
  ops_native_draft_revision_conflict: "稿件已更新。请关闭弹窗，从最新稿件重新登记。",
  ops_native_draft_account_conflict: "当前版本的发布账号不匹配。请返回稿件核对账号。",
  ops_native_account_inactive: "发布账号已停用。",
  ops_native_forbidden: "你当前没有该账号的发布登记权限。",
};

export function NativePublicationDialog({ accounts, close, draft }: {
  accounts: readonly PublishingAccount[];
  close: () => void;
  draft?: DraftRegistration;
}) {
  const client = useQueryClient();
  const pending = useRef<readonly (api.NativePublicationCommand & { requestId: string })[] | null>(null);
  const [retrySameRequest, setRetrySameRequest] = useState(false);
  const [batchMode, setBatchMode] = useState(false);
  function requestClose() {
    if (pending.current) { setMessage("提交结果未确认，请重试同一请求。"); return; }
    close();
  }
  const [accountRef, setAccountRef] = useState(accounts[0]?.ref || "");
  const [publicationUrl, setPublicationUrl] = useState("");
  const [publishedOn, setPublishedOn] = useState("");
  const [title, setTitle] = useState(draft?.title ?? "");
  const [batchUrls, setBatchUrls] = useState("");
  const [batchResults, setBatchResults] = useState<{ ordinal: number; status: number; code: string }[]>([]);
  const [submitting, setSubmitting] = useState(false); const [message, setMessage] = useState("");
  async function submit(event: React.FormEvent) {
    event.preventDefault(); if (submitting) return; setSubmitting(true); setMessage("");
    const urls = draft ? [] : batchUrls.split("\n").map(value => value.trim()).filter(Boolean);
    const isBatch = pending.current ? batchMode : urls.length > 0;
    setBatchMode(isBatch);
    const commands = pending.current || (isBatch ? urls : [publicationUrl]).map(url => ({
      requestId: crypto.randomUUID(), accountRef, publicationUrl: url,
      canonicalUrl: null, publishedOn: publishedOn || null, title: isBatch ? null : title || null, body: null,
      ...(draft ? { creation: { workId: draft.workId, expectedRevision: draft.expectedRevision } } : {}),
    }));
    pending.current = commands;
    try {
      if (isBatch) {
        const result = await api.createNativePublicationBatch(commands);
        pending.current = null; setRetrySameRequest(false);
        const failed = result.items.filter(item => item.outcome !== "accepted" && item.outcome !== "unchanged");
        setBatchResults(result.items.map(item => ({ ordinal: item.ordinal, status: item.status, code: item.code })));
        setBatchUrls(failed.map(item => commands[item.ordinal - 1]!.publicationUrl).join("\n"));
        if (!failed.length) { await Promise.all([client.invalidateQueries({ queryKey: ["ops", "publications"] }), client.invalidateQueries({ queryKey: ["ops", "daily"] }), client.invalidateQueries({ queryKey: ["ops", "summary"] })]); close(); }
        else setMessage(`已登记 ${result.items.length - failed.length} 条；失败行保留，可修正后仅重试它们。`);
        return;
      }
      const result = await api.createNativePublication(commands[0]!);
      const saved = result.outcome === "accepted" || result.outcome === "unchanged";
      if (saved && !result.recordRef) throw new Error("publication receipt missing record");
      pending.current = null; setRetrySameRequest(false);
      if (!saved) { setMessage(draftErrors[result.code] ?? `未登记：${result.code}`); return; }
      await Promise.all([client.invalidateQueries({ queryKey: ["ops", "publications"] }), client.invalidateQueries({ queryKey: ["ops", "daily"] }), client.invalidateQueries({ queryKey: ["ops", "summary"] })]);
      if (draft && result.recordRef) draft.onRegistered(result.recordRef);
      close();
    } catch (error) {
      if (!(error instanceof ApiError) || !error.answered || (error.failure.kind === "http" && error.failure.status >= 500)) {
        setRetrySameRequest(true);
        setMessage("提交结果未确认，请保持字段不变并重试同一请求。");
      } else {
        pending.current = null; setRetrySameRequest(false);
        setMessage(`服务器拒绝：${api.reason(error)}`);
      }
    } finally { setSubmitting(false); }
  }
  return <Dialog label="新增原生发布" onClose={requestClose}><form className={c("ops-dialog-body")} onSubmit={submit}>
    <h2>{draft ? "从稿件登记发布" : "新增发布"}</h2>
    <p className={c("ops-note")}>{draft ? "填写已发布的链接和日期，登记会关联本次保存的稿件版本。" : "只知道发布日期时不填时刻；平台与账号一致性由服务器复核。"}</p>
    <label>账号<select required value={accountRef} onChange={e => setAccountRef(e.target.value)} disabled={submitting || retrySameRequest || !!draft}>{accounts.map(a => <option key={a.ref} value={a.ref}>{a.name} · {a.platform}</option>)}</select></label>
    <label>发布链接<input type="url" required={!batchUrls.trim()} value={publicationUrl} onChange={e => setPublicationUrl(e.target.value)} disabled={submitting || retrySameRequest || !!batchUrls.trim()} /></label>
    <label>发布日期<input type="date" required value={publishedOn} onChange={e => setPublishedOn(e.target.value)} disabled={submitting || retrySameRequest} /></label>
    <label>标题（可选）<input maxLength={1000} value={title} onChange={e => setTitle(e.target.value)} disabled={submitting || retrySameRequest || !!batchUrls.trim()} /></label>
    {draft ? null : <label>批量发布链接（可选，每行一条；同账号、同发布日期）<textarea value={batchUrls} onChange={e => { setBatchUrls(e.target.value); setBatchResults([]); }} disabled={submitting || retrySameRequest} rows={5} /></label>}
    {batchResults.length ? <ul aria-label="批量登记结果">{batchResults.map(item => <li key={item.ordinal}>第 {item.ordinal} 行：HTTP {item.status} · {item.code}</li>)}</ul> : null}
    {message ? <p role="status">{message}</p> : null}<footer><ActionButton type="button" onClick={requestClose} disabled={submitting || retrySameRequest}>取消</ActionButton><ActionButton type="submit" disabled={submitting}>{submitting ? "正在提交…" : retrySameRequest ? "重试同一请求" : "登记发布"}</ActionButton></footer>
  </form></Dialog>;
}
