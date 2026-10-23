import { useState } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { ActionButton } from "../../shared/ActionButton";
import * as api from "./api";
import { Dialog } from "./ui";
import { c } from "./styles";

export function NativePublicationDialog({ accounts, close }: { accounts: readonly api.Account[]; close: () => void }) {
  const client = useQueryClient();
  const [accountRef, setAccountRef] = useState(accounts[0]?.ref || "");
  const [publicationUrl, setPublicationUrl] = useState("");
  const [publishedOn, setPublishedOn] = useState("");
  const [title, setTitle] = useState("");
  const [submitting, setSubmitting] = useState(false); const [message, setMessage] = useState("");
  async function submit(event: React.FormEvent) {
    event.preventDefault(); if (submitting) return; setSubmitting(true); setMessage("");
    try {
      const result = await api.createNativePublication({ requestId: crypto.randomUUID(), accountRef, publicationUrl,
        canonicalUrl: null, publishedOn: publishedOn || null, title: title || null, body: null });
      if (result.outcome !== "accepted" && result.outcome !== "unchanged") { setMessage(`未登记：${result.code}`); return; }
      await Promise.all([client.invalidateQueries({ queryKey: ["ops", "publications"] }), client.invalidateQueries({ queryKey: ["ops", "daily"] }), client.invalidateQueries({ queryKey: ["ops", "summary"] })]); close();
    } catch (error) { setMessage(`提交未确认：${api.reason(error)}`); } finally { setSubmitting(false); }
  }
  return <Dialog label="新增原生发布" onClose={close}><form className={c("ops-dialog-body")} onSubmit={submit}>
    <h2>新增发布</h2><p className={c("ops-note")}>只知道发布日期时不填时刻；平台与账号一致性由服务器复核。</p>
    <label>账号<select required value={accountRef} onChange={e => setAccountRef(e.target.value)} disabled={submitting}>{accounts.map(a => <option key={a.ref} value={a.ref}>{a.name} · {a.platform}</option>)}</select></label>
    <label>发布链接<input type="url" required value={publicationUrl} onChange={e => setPublicationUrl(e.target.value)} disabled={submitting} /></label>
    <label>发布日期<input type="date" required value={publishedOn} onChange={e => setPublishedOn(e.target.value)} disabled={submitting} /></label>
    <label>标题（可选）<input maxLength={1000} value={title} onChange={e => setTitle(e.target.value)} disabled={submitting} /></label>
    {message ? <p role="status">{message}</p> : null}<footer><ActionButton type="button" onClick={close} disabled={submitting}>取消</ActionButton><ActionButton type="submit" disabled={submitting}>{submitting ? "正在提交…" : "登记发布"}</ActionButton></footer>
  </form></Dialog>;
}
