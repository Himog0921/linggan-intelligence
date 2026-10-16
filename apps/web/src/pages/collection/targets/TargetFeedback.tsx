import { createPortal } from 'react-dom';
import { useEffect, useRef } from 'react';
import styles from './TargetPending.module.css';
import type { Commands } from './useWorkspace';
import { targetFeedback } from './targetFeedbackCopy';
export function TargetFeedback({ commands, code, ahead = null }: {
  commands: Commands;
  code?: string | null;
  ahead?: string | null;
}) {
  const f = commands.feedback;
  const verify = useRef<HTMLButtonElement>(null);
  useEffect(() => {
    if (!f?.unknown) return;
    const previous = document.activeElement instanceof HTMLElement ? document.activeElement : null;
    verify.current?.focus();
    queueMicrotask(() => verify.current?.focus());
    return () => previous?.focus();
  }, [f?.unknown, f?.requestRef]);
  if (f?.unknown) return createPortal(
    <div className={`c-tg-batch-overlay ${styles.pendingOverlay}`}>
      <section className="c-tg-batch-dialog" role="alertdialog" aria-modal="true" aria-labelledby="target-pending-title" onKeyDown={(event) => { if (event.key === 'Tab') { event.preventDefault(); verify.current?.focus(); } }}>
        <div role="status"><h2 id="target-pending-title">提交结果未确认</h2><p>{f.text}</p><p>请求编号：{f.requestRef}</p></div>
        <button className="c-btn-secondary" type="button" ref={verify} onClick={() => { void commands.verify(); }}>核实本次请求</button>
      </section>
    </div>, document.body,
  );
  if (!f) {
    const saved = targetFeedback(code ?? null, ahead);
    return saved ? (
      <p
        className={saved.className === 'c-src-feedback c-src-feedback-ok' ? 'c-tg-toast' : saved.className}
        role="status"
      >
        <b>{saved.title}</b>
        {saved.explanation}
      </p>
    ) : null;
  }
  const copy = targetFeedback(f.code, ahead);
  const className = copy?.className === 'c-src-feedback c-src-feedback-ok' || (f.applied && !copy)
      ? 'c-tg-toast'
      : (copy?.className ?? 'c-src-failure');
  return (
    <div className={className} role="status" key={f.requestRef}>
      <b>{copy?.title ?? (f.applied ? '操作已取得回执' : '没有完成')}</b>
      {copy?.explanation ?? f.text}
    </div>
  );
}
