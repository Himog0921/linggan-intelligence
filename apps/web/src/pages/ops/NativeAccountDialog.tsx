import { useState } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { ActionButton } from "../../shared/ActionButton";
import * as api from "./api";
import { Dialog } from "./ui";
import { c } from "./styles";

/** Minimal, receipt-bound account entry. More fields remain server-owned until their edit flows exist. */
export function NativeAccountDialog({ domainRef, close }: { domainRef: string; close: () => void }) {
  const client = useQueryClient();
  const [platform, setPlatform] = useState<"xhs" | "douyin">("xhs");
  const [accountName, setAccountName] = useState("");
  const [profileUrl, setProfileUrl] = useState("");
  const [platformUserId, setPlatformUserId] = useState("");
  const [submitting, setSubmitting] = useState(false);
  const [message, setMessage] = useState("");
  async function submit(event: React.FormEvent) {
    event.preventDefault();
    if (submitting) return;
    setSubmitting(true); setMessage("");
    try {
      const result = await api.createNativeAccount({
        requestId: crypto.randomUUID(), domainRef, platform, accountName,
        profileUrl: profileUrl.trim() || null, platformUserId: platformUserId.trim() || null,
        handle: null, status: null, isRetained: null, followerCount: null,
        deviceCode: null, ipLocation: null, notes: null,
      });
      if (result.outcome !== "accepted" && result.outcome !== "unchanged") {
        setMessage(`未创建：${result.code}`); return;
      }
      await Promise.all([
        client.invalidateQueries({ queryKey: ["ops", "accounts"] }),
        client.invalidateQueries({ queryKey: ["ops", "businesses"] }),
        client.invalidateQueries({ queryKey: ["ops", "summary"] }),
      ]);
      close();
    } catch (error) {
      setMessage(`提交未确认：${api.reason(error)}`);
    } finally { setSubmitting(false); }
  }
  return <Dialog label="新增原生账号" onClose={close}>
    <form className={c("ops-dialog-body")} onSubmit={submit}>
      <h2>新增账号</h2>
      <p className={c("ops-note")}>创建后会保留原生来源；账号归属与权限由服务器按当前会话判定。</p>
      <label>平台<select value={platform} onChange={e => setPlatform(e.target.value as "xhs" | "douyin")} disabled={submitting}><option value="xhs">小红书</option><option value="douyin">抖音</option></select></label>
      <label>账号名称<input required maxLength={512} value={accountName} onChange={e => setAccountName(e.target.value)} disabled={submitting} /></label>
      <label>主页链接<input type="url" value={profileUrl} onChange={e => setProfileUrl(e.target.value)} disabled={submitting} /></label>
      <label>平台用户编号<input value={platformUserId} onChange={e => setPlatformUserId(e.target.value)} disabled={submitting} /></label>
      {message ? <p role="status">{message}</p> : null}
      <footer><ActionButton type="button" onClick={close} disabled={submitting}>取消</ActionButton><ActionButton type="submit" disabled={submitting}>{submitting ? "正在提交…" : "创建账号"}</ActionButton></footer>
    </form>
  </Dialog>;
}
