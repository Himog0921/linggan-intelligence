import { cloneElement, useId, type ReactElement, type ReactNode } from 'react';
import { ActionButton } from '../../shared/ActionButton';
import { humanMoment } from '../../shared/formatting';
import type { Notice } from './useAiAction';
import styles from './AiSettings.module.css';

export function Field({ label, hint, children }: { label: string; hint?: string; children: ReactElement<{ id?: string; 'aria-describedby'?: string }> }) {
  const id = useId();
  return <div className={styles.field}><label htmlFor={id}>{label}</label>{cloneElement(children, { id, ...(hint ? { 'aria-describedby': `${id}-hint` } : {}) })}{hint ? <small id={`${id}-hint`}>{hint}</small> : null}</div>;
}
export function SaveBar({ busy, children = '保存配置' }: { busy: boolean; children?: ReactNode }) {
  return <footer className={styles.save}><span>保存后用于后续批次，当前批次保留原配置。</span><ActionButton type="submit" variant="primary" busy={busy}>{children}</ActionButton></footer>;
}
export function NoticeLine({ notice }: { notice: Notice | null }) {
  return <div role="status" aria-live="polite" className={styles.notice}>{notice ? <p data-tone={notice.tone}><b>{notice.tone === 'success' ? '已收到回执' : notice.tone === 'unknown' ? '结果未确认' : '未完成'}</b> {notice.text}{notice.code ? <small> · {notice.code}</small> : null}</p> : null}</div>;
}
export function Impact({ title, children, label, action, disabled = false }: { title: string; children: ReactNode; label: string; action: () => void; disabled?: boolean }) {
  const id = useId();
  return <section className={styles.impact} aria-labelledby={id}><h3 id={id}>{title}</h3><div>{children}</div><ActionButton variant="outline" disabled={disabled} onClick={action}>{label}</ActionButton></section>;
}
export function Moment({ value }: { value: string | null }) { return value ? <time dateTime={value}>{humanMoment(value)}</time> : <>—</>; }
export function Reference({ value }: { value: string | null }) { return value ? <code className={styles.reference}>{value}</code> : <>—</>; }
