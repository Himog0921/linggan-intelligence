import { useState } from "react";
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

type Values = Record<string, string>;

/**
 * A batch represents multiple independently durable reports, not one bulk
 * fact. It is deliberately limited to the visible page so every selected
 * publication is identifiable to the operator without exposing internal refs.
 */
export function NativeViewBatchDialog({ publications, close }: { publications: readonly api.Publication[]; close: () => void }) {
  const client = useQueryClient();
  const [selected, setSelected] = useState<Values>({});
  const [requestIds, setRequestIds] = useState<Values>({});
  const [asOf, setAsOf] = useState(localMinute);
  const [submitting, setSubmitting] = useState(false);
  const [retrySameRequest, setRetrySameRequest] = useState(false);
  const [message, setMessage] = useState("");
  const [results, setResults] = useState<{ ordinal: number; status: number; code: string }[]>([]);
  const locked = submitting || retrySameRequest;
  const entries = publications.filter(item => Object.hasOwn(selected, item.ref));

  function toggle(item: api.Publication, checked: boolean) {
    if (locked) return;
    setSelected(current => {
      const next = { ...current };
      if (checked) next[item.ref] = item.views.value?.toString() || "";
      else delete next[item.ref];
      return next;
    });
    setResults([]); setMessage("");
  }
  function changeValue(ref: string, value: string) {
    if (locked) return;
    setSelected(current => ({ ...current, [ref]: value }));
    setResults([]); setMessage("");
  }
  function requestClose() {
    if (retrySameRequest) {
      setMessage("本次结果尚未确认，请保持选择和数值不变并重试同一批请求。");
      return;
    }
    close();
  }
  async function submit(event: React.FormEvent) {
    event.preventDefault();
    if (submitting) return;
    if (!entries.length) { setMessage("请至少选择一条当前页发布记录。"); return; }
    if (entries.length > 100) { setMessage("单次最多补充 100 条阅读量。请缩小筛选范围后继续。 "); return; }
    const asOfInstant = new Date(asOf);
    if (Number.isNaN(asOfInstant.getTime())) { setMessage("请选择有效的采样时间。"); return; }
    const invalid = entries.find(item => !selected[item.ref]!.trim() || !Number.isSafeInteger(Number(selected[item.ref])) || Number(selected[item.ref]) < 0);
    if (invalid) { setMessage(`“${invalid.title || "无标题发布"}”的阅读量必须是非负整数；0 是有效实际值。`); return; }
    const ids = { ...requestIds };
    for (const item of entries) ids[item.ref] ||= crypto.randomUUID();
    setRequestIds(ids); setSubmitting(true); setMessage("");
    try {
      const result = await api.recordNativeViewBatch(entries.map(item => ({
        publicationRef: item.ref, requestId: ids[item.ref]!, reportKind: "value",
        viewCount: Number(selected[item.ref]), asOf: asOfInstant.toISOString(),
      })));
      const failed = result.items.filter(item => item.outcome !== "accepted" && item.outcome !== "unchanged");
      setResults(result.items.map(item => ({ ordinal: item.ordinal, status: item.status, code: item.code })));
      if (!failed.length) {
        await invalidateViewFacts(client);
        close(); return;
      }
      const failedRefs = new Set(failed.map(item => entries[item.ordinal - 1]!.ref));
      setSelected(current => Object.fromEntries(Object.entries(current).filter(([ref]) => failedRefs.has(ref))));
      // The server has answered these attempts. A correction must be a new fact
      // request rather than mutating the durable rejected request identity.
      setRequestIds({}); setRetrySameRequest(false);
      setMessage(`已写入 ${entries.length - failed.length} 条；失败项已保留，可修正后仅重试它们。`);
    } catch (error) {
      if (!(error instanceof ApiError) || !error.answered || (error.failure.kind === "http" && error.failure.status >= 500)) {
        setRetrySameRequest(true);
        setMessage("提交结果未确认。请保持选择、数值和采样时间不变并重试同一批请求，系统会按每条原请求身份返回既有结果。");
      } else {
        setRequestIds({}); setRetrySameRequest(false);
        setMessage(`服务器拒绝：${api.reason(error)}`);
      }
    } finally { setSubmitting(false); }
  }

  return <Dialog label="批量补充阅读量" onClose={requestClose}><form className={c("ops-dialog-body")} onSubmit={submit}>
    <h2>批量补充阅读量</h2>
    <p className={c("ops-note")}>仅对当前筛选页的已展示记录生效。每条都是独立、可追溯的手工报告；这里只能填写实际值，清除为未知请逐条操作。</p>
    <fieldset disabled={locked}><legend>选择发布记录并填写阅读量</legend>
      <ul aria-label="当前页阅读量录入">
        {publications.map(item => {
          const label = item.title || item.url;
          const checked = Object.hasOwn(selected, item.ref);
          return <li key={item.ref}>
            <label><input type="checkbox" checked={checked} onChange={event => toggle(item, event.target.checked)} />选择：{label}</label>
            <label>阅读量：{label}<input type="number" required={checked} min="0" step="1" inputMode="numeric" value={selected[item.ref] || ""} onChange={event => changeValue(item.ref, event.target.value)} disabled={!checked || locked} /></label>
          </li>;
        })}
      </ul>
    </fieldset>
    <label>统一采样时间<input type="datetime-local" required value={asOf} onChange={event => setAsOf(event.target.value)} disabled={locked} /></label>
    {results.length ? <ul aria-label="批量阅读量结果">{results.map(item => <li key={item.ordinal}>第 {item.ordinal} 条：HTTP {item.status} · {item.code}</li>)}</ul> : null}
    {message ? <p role="status">{message}</p> : null}
    <footer><ActionButton type="button" onClick={requestClose} disabled={submitting}>取消</ActionButton><ActionButton type="submit" disabled={submitting}>{submitting ? "正在提交…" : retrySameRequest ? "重试同一批请求" : "保存批量阅读量"}</ActionButton></footer>
  </form></Dialog>;
}

async function invalidateViewFacts(client: ReturnType<typeof useQueryClient>) {
  await Promise.all([
    client.invalidateQueries({ queryKey: ["ops", "publications"] }),
    client.invalidateQueries({ queryKey: ["ops", "daily"] }),
    client.invalidateQueries({ queryKey: ["ops", "summary"] }),
    client.invalidateQueries({ queryKey: ["ops", "overview"] }),
    client.invalidateQueries({ queryKey: ["ops", "trend"] }),
  ]);
}
