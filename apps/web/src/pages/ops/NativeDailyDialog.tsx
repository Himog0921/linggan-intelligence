import { useRef, useState } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { ApiError } from "../../shared/api";
import { ActionButton } from "../../shared/ActionButton";
import * as api from "./api";
import { Dialog } from "./ui";
import { c } from "./styles";

const MAX_I32 = 2_147_483_647;

function optionalCount(value: string): number | null | undefined {
  if (value.trim() === "") return null;
  const parsed = Number(value);
  return Number.isSafeInteger(parsed) && parsed >= 0 && parsed <= MAX_I32
    ? parsed
    : undefined;
}

/**
 * A blank metric intentionally becomes null.  It is never coerced to zero:
 * zero is an explicit operational fact with different reporting semantics.
 */
export function NativeDailyDialog({ close, item }: { close: () => void; item?: api.Daily }) {
  const client = useQueryClient();
  const requestId = useRef<string | null>(null);
  const editing = item !== undefined;
  const [businessDate, setBusinessDate] = useState(item?.businessDate || "");
  const [linksOpened, setLinksOpened] = useState(item?.linksOpened?.toString() || "");
  const [followersGained, setFollowersGained] = useState(item?.followersGained?.toString() || "");
  const [submitting, setSubmitting] = useState(false);
  const [retrySameRequest, setRetrySameRequest] = useState(false);
  const [message, setMessage] = useState("");
  const locked = submitting || retrySameRequest;

  function requestClose() {
    if (retrySameRequest) {
      setMessage("本次结果尚未确认，请先重试同一请求；字段已锁定，避免换请求身份后重复登记。");
      return;
    }
    close();
  }

  async function submit(event: React.FormEvent) {
    event.preventDefault();
    if (submitting) return;
    const linksOpenedCount = optionalCount(linksOpened);
    const followersGainedCount = optionalCount(followersGained);
    if (linksOpenedCount === undefined || followersGainedCount === undefined) {
      setMessage("打开和加微只能填写 0 到 2147483647 的整数；留空表示未填写。");
      return;
    }
    setSubmitting(true);
    setMessage("");
    const id = requestId.current || crypto.randomUUID();
    requestId.current = id;
    try {
      const result = await api.recordNativeDaily({
        requestId: id,
        dailyRef: item?.ref || null,
        businessDate,
        linksOpenedCount,
        followersGained: followersGainedCount,
        expectedVersion: item?.nativeVersion || null,
      });
      requestId.current = null;
      setRetrySameRequest(false);
      if (result.outcome === "accepted" || result.outcome === "unchanged") {
        await Promise.all([
          client.invalidateQueries({ queryKey: ["ops", "daily"] }),
          client.invalidateQueries({ queryKey: ["ops", "summary"] }),
          client.invalidateQueries({ queryKey: ["ops", "overview"] }),
        ]);
        close();
        return;
      }
      if (result.outcome === "stale") {
        await client.invalidateQueries({ queryKey: ["ops", "daily"] });
        setMessage(editing ? "这条日报已被更新，未覆盖原记录。列表已刷新，请核对后再编辑。" : "该日期已有你的日报，未覆盖原记录。刷新后可进入编辑。");
        return;
      }
      setMessage(`服务器拒绝：${result.code}`);
    } catch (error) {
      if (error instanceof ApiError && !error.answered) {
        setRetrySameRequest(true);
        setMessage("提交结果未确认。请保持字段不变并重试同一请求，系统会按同一请求身份返回既有结果。");
      } else {
        requestId.current = null;
        setRetrySameRequest(false);
        setMessage(`服务器拒绝：${api.reason(error)}`);
      }
    } finally {
      setSubmitting(false);
    }
  }

  return <Dialog label={editing ? "修正原生日报" : "填写原生日报"} onClose={requestClose}><form className={c("ops-dialog-body")} onSubmit={submit}>
    <h2>{editing ? "修正日报" : "填写日报"}</h2>
    <p className={c("ops-note")}>留空是“未填写”，填 0 才是“当天为零”。{editing ? "本次按读到的版本保存；若他人已更新，服务端会拒绝覆盖。" : "日报归属当前会话的运营人，服务端会拒绝覆盖已有版本。"}</p>
    <label>日期<input type="date" required value={businessDate} onChange={event => setBusinessDate(event.target.value)} disabled={locked || editing} /></label>
    <label>打开（可选）<input type="number" min="0" max={MAX_I32} step="1" inputMode="numeric" value={linksOpened} onChange={event => setLinksOpened(event.target.value)} disabled={locked} /></label>
    <label>加微（可选）<input type="number" min="0" max={MAX_I32} step="1" inputMode="numeric" value={followersGained} onChange={event => setFollowersGained(event.target.value)} disabled={locked} /></label>
    {message ? <p role="status">{message}</p> : null}
    <footer><ActionButton type="button" onClick={requestClose} disabled={submitting}>取消</ActionButton><ActionButton type="submit" disabled={submitting}>{submitting ? "正在提交…" : retrySameRequest ? "重试同一请求" : editing ? "保存修正" : "登记日报"}</ActionButton></footer>
  </form></Dialog>;
}
