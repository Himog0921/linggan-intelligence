import { useRef, useState } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { ApiError } from "../../shared/api";
import { ActionButton } from "../../shared/ActionButton";
import * as api from "./api";
import { Dialog } from "./ui";
import { c } from "./styles";

function localMinute(date = new Date()) {
  const offset = date.getTimezoneOffset() * 60_000;
  return new Date(date.getTime() - offset).toISOString().slice(0, 16);
}

/** A manual view report is append-only; clearing means unknown, never zero. */
export function NativeViewDialog({ publication, close }: { publication: api.Publication; close: () => void }) {
  const client = useQueryClient();
  const requestId = useRef<string | null>(null);
  const [kind, setKind] = useState<"value" | "cleared">("value");
  const [viewCount, setViewCount] = useState(publication.views.value?.toString() || "");
  const [asOf, setAsOf] = useState(localMinute);
  const [submitting, setSubmitting] = useState(false);
  const [retrySameRequest, setRetrySameRequest] = useState(false);
  const [message, setMessage] = useState("");
  const locked = submitting || retrySameRequest;
  function requestClose() {
    if (retrySameRequest) {
      setMessage("本次结果尚未确认，请先重试同一请求；字段已锁定，避免生成重复阅读量报告。");
      return;
    }
    close();
  }
  async function submit(event: React.FormEvent) {
    event.preventDefault();
    if (submitting) return;
    const parsed = Number(viewCount);
    if (kind === "value" && (!Number.isSafeInteger(parsed) || parsed < 0)) {
      setMessage("阅读量只能填写非负整数；0 是有效的实际值。");
      return;
    }
    const asOfInstant = new Date(asOf);
    if (Number.isNaN(asOfInstant.getTime())) { setMessage("请选择有效的采样时间。"); return; }
    const id = requestId.current || crypto.randomUUID();
    requestId.current = id;
    setSubmitting(true); setMessage("");
    try {
      const result = await api.recordNativeView(publication.ref, {
        requestId: id, reportKind: kind, viewCount: kind === "value" ? parsed : null, asOf: asOfInstant.toISOString(),
      });
      requestId.current = null; setRetrySameRequest(false);
      if (result.outcome === "accepted" || result.outcome === "unchanged") {
        await Promise.all([
          client.invalidateQueries({ queryKey: ["ops", "publications"] }),
          client.invalidateQueries({ queryKey: ["ops", "daily"] }),
          client.invalidateQueries({ queryKey: ["ops", "summary"] }),
          client.invalidateQueries({ queryKey: ["ops", "overview"] }),
          client.invalidateQueries({ queryKey: ["ops", "trend"] }),
        ]);
        close(); return;
      }
      setMessage(`服务器拒绝：${result.code}`);
    } catch (error) {
      if (error instanceof ApiError && !error.answered) {
        setRetrySameRequest(true);
        setMessage("提交结果未确认。请保持字段不变并重试同一请求，系统会按同一请求身份返回既有结果。");
      } else {
        requestId.current = null; setRetrySameRequest(false);
        setMessage(`服务器拒绝：${api.reason(error)}`);
      }
    } finally { setSubmitting(false); }
  }
  return <Dialog label="补充手工阅读量" onClose={requestClose}><form className={c("ops-dialog-body")} onSubmit={submit}>
    <h2>补充阅读量</h2>
    <p className={c("ops-note")}>{publication.title || "无标题发布"} · 这会追加一条手工报告；最新有效报告才决定当前显示。</p>
    <fieldset disabled={locked}><legend>报告内容</legend>
      <label><input type="radio" checked={kind === "value"} onChange={() => setKind("value")} />填写实际值</label>
      <label><input type="radio" checked={kind === "cleared"} onChange={() => setKind("cleared")} />清除为未知</label>
    </fieldset>
    <label>阅读量<input type="number" min="0" step="1" inputMode="numeric" required={kind === "value"} value={viewCount} onChange={event => setViewCount(event.target.value)} disabled={locked || kind === "cleared"} /></label>
    <label>采样时间<input type="datetime-local" required value={asOf} onChange={event => setAsOf(event.target.value)} disabled={locked} /></label>
    {message ? <p role="status">{message}</p> : null}
    <footer><ActionButton type="button" onClick={requestClose} disabled={submitting}>取消</ActionButton><ActionButton type="submit" disabled={submitting}>{submitting ? "正在提交…" : retrySameRequest ? "重试同一请求" : "保存阅读量"}</ActionButton></footer>
  </form></Dialog>;
}
