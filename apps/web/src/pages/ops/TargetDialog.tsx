import { useEffect, useRef, useState, useSyncExternalStore } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import * as api from "./api";
import { number } from "./format";
import { Dialog } from "./ui";
import { c } from "./styles";
import { pendingTarget, saveTarget, subscribeTargets } from "./targetSave";
export function TargetDialog({
  data,
  initial,
  close,
  saved,
}: {
  data: api.Overview;
  initial: api.PeriodType;
  close: () => void;
  saved: (note: string) => void;
}) {
  const [type, setType] = useState(initial);
  const periodKey =
    type === "month" ? data.window.thisMonth : data.window.thisWeek;
  return (
    <Dialog title="ops-target-title" onClose={close}>
      <div>
        <TargetForm
          key={`${type}:${periodKey}`}
          type={type}
          periodKey={periodKey}
          setType={setType}
          data={data}
          close={close}
          saved={saved}
        />
      </div>
    </Dialog>
  );
}
function TargetForm({
  type,
  periodKey,
  setType,
  data,
  close,
  saved,
}: {
  type: api.PeriodType;
  periodKey: string;
  setType: (v: api.PeriodType) => void;
  data: api.Overview;
  close: () => void;
  saved: (note: string) => void;
}) {
  const query = useQuery({
    queryKey: ["ops", "target-history", type, periodKey],
    queryFn: ({ signal }) =>
      api.get(
        api.url("targets", { periodType: type, periodKey }),
        api.history,
        signal,
      ),
    staleTime: 0,
    gcTime: 0,
  });
  const [value, setValue] = useState<string | null>(null);
  const [current, setCurrent] = useState<api.Revision | null | undefined>(
    undefined,
  );
  const [error, setError] = useState("");
  const [saving, setSaving] = useState(false);
  const pending = useSyncExternalStore(subscribeTargets, () =>
    pendingTarget(type, periodKey),
  );
  const uncertain = Boolean(pending && !pending.sending);
  const client = useQueryClient();
  const wasSending = useRef(Boolean(pending?.sending));
  useEffect(() => {
    if (wasSending.current && !pending) {
      void client.invalidateQueries({
        queryKey: ["ops", "target-history", type, periodKey],
      });
      void client.invalidateQueries({ queryKey: ["ops", "overview"] });
    }
    wasSending.current = Boolean(pending?.sending);
  }, [pending, client, type, periodKey]);
  const live = useRef(true);
  useEffect(() => {
    live.current = true;
    return () => {
      live.current = false;
    };
  }, []);
  const shown =
    value ??
    (pending
      ? String(pending.command.targetValue)
      : query.data?.current
        ? String(query.data.current.targetValue)
        : "");
  const revision = current === undefined ? query.data?.current : current;
  async function submit(event: React.FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const targetValue = Number(shown);
    if (
      !Number.isInteger(targetValue) ||
      targetValue < 1 ||
      targetValue > 1000000
    ) {
      setError("请输入 1 到 1000000 之间的整数");
      return;
    }
    if (revision === undefined && !pending) {
      setError("当前目标还没读到，不能保存");
      return;
    }
    setSaving(true);
    setError("");
    try {
      const answer = await saveTarget({
        periodType: type,
        periodKey,
        targetValue,
        expectedRevision: revision?.revisionRef || null,
      });
      if (!live.current) return;
      if (answer.outcome === "stale") {
        setCurrent(answer.current);
        setError("目标刚被修改过，已显示最新值；确认后再保存");
        void query.refetch();
        return;
      }
      saved(answer.outcome === "unchanged" ? "目标没有变化。" : "");
    } catch (error) {
      if (!live.current) return;
      setError(`保存失败（${api.reason(error)}），输入已保留，可以再试`);
    } finally {
      if (live.current) setSaving(false);
    }
  }
  return (
    <form id="ops-target-form" onSubmit={(event) => void submit(event)}>
      <header>
        <h2 id="ops-target-title">设定加微目标</h2>
        <small>只能在本机设；每次保存都留修改记录</small>
      </header>
      <div className={c("ops-dialog-body")}>
        <fieldset className={c("ops-choice")} disabled={saving}>
          <legend>周期</legend>
          {(["week", "month"] as const).map((t) => (
            <label key={t}>
              <input
                type="radio"
                name="periodType"
                value={t}
                checked={type === t}
                onChange={() => setType(t)}
              />{" "}
              <span>
                {t === "week"
                  ? `本周（${data.window.thisWeek}）`
                  : `本月（${data.window.thisMonth}）`}
              </span>
            </label>
          ))}
        </fieldset>
        <p className={c("ops-note-line")}>
          {query.isError
            ? `当前目标读取失败（${api.reason(query.error)}），请关闭后重试`
            : revision === undefined
              ? "当前：读取中"
              : revision
                ? `当前：${number(revision.targetValue)}（${revision.setAtBeijing} · 本机）`
                : "当前：未设定"}
        </p>
        <label>
          目标值（加微人数）
          <input
            type="number"
            name="targetValue"
            min={1}
            max={1000000}
            step={1}
            inputMode="numeric"
            required
            value={shown}
            disabled={saving || uncertain || pending?.sending}
            onChange={(event) => setValue(event.target.value)}
          />
        </label>
        <p className={c("ops-form-error")} role="alert">
          {error ||
            (uncertain ? "上次保存结果未确认；再次保存将重试原请求。" : "")}
        </p>
        <details className={c("ops-history")}>
          <summary>修改记录</summary>
          <ol>
            {query.data?.revisions.length ? (
              query.data.revisions.map((r) => (
                <li key={r.revisionRef}>
                  {r.setAtBeijing} 改为 {number(r.targetValue)} · 本机
                </li>
              ))
            ) : (
              <li>
                {query.isPending
                  ? "读取中"
                  : query.isError
                    ? "修改记录读取失败，关闭后重新打开可再读"
                    : "还没有修改记录"}
              </li>
            )}
          </ol>
        </details>
      </div>
      <footer>
        <button type="button" onClick={close}>
          取消
        </button>
        <button
          type="submit"
          className={c("ops-primary")}
          disabled={saving || Boolean(pending?.sending)}
        >
          保存目标
        </button>
      </footer>
    </form>
  );
}
