import { useEffect, useRef, useState, type FormEvent, type ReactNode } from 'react';
import { ApiError } from '../../../shared/api';
import { RequestIdentities } from '../../../shared/requestIdentity';
import { executeCollectionCommand, readCollectionCommandReceipt } from '../collectionCommand';
import styles from './Management.module.css';

function rejectionText(code: string | null): string {
  const explanations: Record<string, string> = {
    database_unavailable: '数据库当前不可用；本次操作没有写入。',
    read_model_not_connected: '数据库当前不可用；本次操作没有写入。',
    name_taken: '已有同名领域；本次操作没有写入。',
    name_required: '请填写领域名称；本次操作没有写入。',
    station_rejected: '工位没有登记成功。最常见的原因是名字与某台在册工位重复——名字要能让你一眼认出是哪台机器，因此不允许重名。',
    station_name_rejected: '工位名称没有更新。名字不能为空，且不能与另一台在册工位重复。',
    claim_window_rejected: '认领窗口没有打开。该工位可能已被停用。',
    close_window_rejected: '认领窗口没有关闭。它可能已经自己过期了。',
    retire_rejected: '工位没有停用成功。它可能已经处于停用状态。',
    claim_rejected: '认领没有成功。这台工位可能已有在岗安装，或该安装已被取代。',
  };
  return `${code ? explanations[code] ?? '本次操作没有完成。' : '本次操作没有完成。'}输入已保留。${code ? `（${code}）` : ''}`;
}

interface PendingIntent { fields: Record<string, unknown>; requestRef: string; draft: Record<string, string> }
// Only unsettled management metadata lives here, in this document's memory. No local/session
// storage, source material, account identity digest, or credential is retained.
const unsettled = new Map<string, PendingIntent>();
const unknownText = '结果未确认。请先读取回执核实，当前请求不会自动重发。';
function commandScope(action: string, fields: Record<string, unknown>): string {
  const objects = ['domain_ref', 'station_ref', 'installation_ref', 'target_ref'].map(name => [name, fields[name] ?? null]);
  return JSON.stringify({ action, objects, relationAction: fields.action ?? null });
}
function restoreDraft(form: HTMLFormElement, draft: Record<string, string>) {
  for (const element of form.elements) {
    if (element instanceof HTMLInputElement || element instanceof HTMLSelectElement || element instanceof HTMLTextAreaElement) {
      const value = draft[element.name]; if (value !== undefined) element.value = value;
    }
  }
}

interface Props {
  action: string;
  children: ReactNode;
  className?: string;
  fields?: Record<string, unknown>;
  onWritten: () => void;
  onApplied?: () => void;
  returnTo?: string;
  hidden?: boolean;
  resetOnApplied?: boolean;
}
function ScopedCommandForm({ action, children, className, fields = {}, onWritten, onApplied, returnTo, hidden, resetOnApplied = false, scope }: Props & { scope: string }) {
  const [identities] = useState(() => new RequestIdentities());
  const [restored] = useState(() => unsettled.get(scope) ?? null);
  const [busy, setBusy] = useState(false);
  const [unknown, setUnknown] = useState(restored !== null);
  const [feedback, setFeedback] = useState<string | null>(restored ? unknownText : null);
  const form = useRef<HTMLFormElement>(null);
  const pending = useRef<PendingIntent | null>(restored);
  const inFlight = useRef(false);
  const mounted = useRef(false);
  useEffect(() => {
    mounted.current = true;
    if (restored && form.current) restoreDraft(form.current, restored.draft);
    return () => { mounted.current = false; };
  }, [restored]);
  function settle(payload: PendingIntent) {
    if (unsettled.get(scope)?.requestRef === payload.requestRef) unsettled.delete(scope);
    identities.settle(action); pending.current = null;
  }
  async function send(payload: PendingIntent, verify = false) {
    if (inFlight.current) return;
    inFlight.current = true; setBusy(true); setFeedback(null);
    try {
      const receipt = verify ? await readCollectionCommandReceipt(action, payload.requestRef)
        : await executeCollectionCommand(action, payload.fields, payload.requestRef);
      settle(payload);
      if (!mounted.current) return;
      setUnknown(false);
      if (receipt.outcome === 'applied') {
        setFeedback('已收到服务器回执。'); if (resetOnApplied) form.current?.reset(); onWritten(); onApplied?.();
      } else setFeedback(rejectionText(receipt.code));
    } catch (error) {
      if (error instanceof ApiError && error.answered) {
        settle(payload);
        if (!mounted.current) return;
        setUnknown(false);
        setFeedback(rejectionText(error.failure.kind === 'http' ? error.failure.code : null));
      } else {
        if (!mounted.current) return;
        setUnknown(true);
        setFeedback(unknownText);
      }
    } finally { inFlight.current = false; if (mounted.current) setBusy(false); }
  }
  function submit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault(); if (busy || unknown || inFlight.current) return;
    const existing = unsettled.get(scope);
    if (existing) {
      pending.current = existing; setUnknown(true); setFeedback(unknownText); restoreDraft(event.currentTarget, existing.draft);
      return;
    }
    const values: Record<string, unknown> = Object.fromEntries(new FormData(event.currentTarget, event.nativeEvent instanceof SubmitEvent ? event.nativeEvent.submitter : null));
    const payload = { ...values, ...fields, ...(returnTo ? { return_to: returnTo } : {}) };
    const requestRef = identities.keyFor(action, JSON.stringify(payload));
    const draft: Record<string, string> = {};
    for (const [name, value] of new FormData(event.currentTarget)) if (typeof value === 'string') draft[name] = value;
    const intent = { fields: payload, requestRef, draft };
    pending.current = intent; unsettled.set(scope, intent);
    void send(intent);
  }
  return <form ref={form} className={className} onSubmit={submit} aria-busy={busy} hidden={hidden} data-command-unconfirmed={unknown}>
    <fieldset className={styles.fields} disabled={busy || unknown}>{children}</fieldset>
    {busy && <span role="status">正在等待服务器回执…</span>}
    {feedback && <p role={unknown ? 'alert' : 'status'} className="domain-feedback">{feedback}</p>}
    {unknown && <button className="c-btn-quiet" type="button" disabled={busy} onClick={() => { if (pending.current) void send(pending.current, true); }}>核实同一请求</button>}
  </form>;
}

export function CommandForm(props: Props) {
  const scope = commandScope(props.action, props.fields ?? {});
  return <ScopedCommandForm key={scope} {...props} scope={scope} />;
}
