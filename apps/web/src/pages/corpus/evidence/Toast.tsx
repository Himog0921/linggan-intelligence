import { useCallback, useEffect, useMemo, useRef, useState, type ReactNode } from 'react';
import { createPortal } from 'react-dom';
import { ToastContext, type ToastNotice } from './toastContext';
import styles from './Toast.module.css';

interface StoredNotice extends ToastNotice { id: number }

const MAX_VISIBLE = 2;
const DURATION_MS = 5000;

/** 原生 modal 位于顶层；通知宿主跟着它走，才不会被 ::backdrop 遮住。 */
function useTopLayerHost(fallback: HTMLElement): HTMLElement {
  const [host, setHost] = useState<HTMLElement>(fallback);
  useEffect(() => {
    const sync = () => setHost(document.querySelector<HTMLElement>('dialog[open]') ?? fallback);
    sync();
    const observer = new MutationObserver(sync);
    observer.observe(document.body, { subtree: true, childList: true, attributes: true, attributeFilter: ['open'] });
    return () => observer.disconnect();
  }, [fallback]);
  return host;
}

function ToastItem({ notice, onDismiss }: { notice: StoredNotice; onDismiss: (id: number) => void }) {
  const remaining = useRef(DURATION_MS);
  const [hovered, setHovered] = useState(false);
  const [focused, setFocused] = useState(false);
  const paused = hovered || focused;
  useEffect(() => {
    if (paused) return;
    const started = Date.now();
    const timer = window.setTimeout(() => onDismiss(notice.id), remaining.current);
    return () => {
      window.clearTimeout(timer);
      remaining.current = Math.max(0, remaining.current - (Date.now() - started));
    };
  }, [paused, notice.id, onDismiss]);
  return (
    <div
      className={styles.toast}
      data-tone={notice.tone}
      role="status"
      aria-live="polite"
      aria-atomic="true"
      onPointerEnter={() => setHovered(true)}
      onPointerLeave={() => setHovered(false)}
      onFocus={() => setFocused(true)}
      onBlur={event => { if (!event.currentTarget.contains(event.relatedTarget as Node | null)) setFocused(false); }}
    >
      <i className={styles.mark} aria-hidden="true" />
      <div className={styles.body}>
        {notice.context ? <span className={styles.context} title={notice.context}>{notice.context}</span> : null}
        <p className={styles.copy}>{notice.text}</p>
      </div>
      <button type="button" className={styles.close} aria-label="关闭通知" onClick={() => onDismiss(notice.id)}>
        <span aria-hidden="true">×</span>
      </button>
    </div>
  );
}

interface ToastProviderProps {
  children: ReactNode;
  /** 底部选择浮层出现时，整组通知上移，保留间隔。 */
  lifted?: boolean;
}

/**
 * 证据库局部轻通知：右下角最多两条，约五秒自动关闭；悬停或键盘停留暂停计时，不抢焦点。
 * 只在用户动作的回执里调用一次，轮询、切页签与重绘不会重复播报。
 */
export function ToastProvider({ children, lifted = false }: ToastProviderProps) {
  const [notices, setNotices] = useState<StoredNotice[]>([]);
  const nextId = useRef(0);
  const [layer] = useState(() => document.createElement('div'));
  useEffect(() => {
    document.body.appendChild(layer);
    return () => layer.remove();
  }, [layer]);
  const host = useTopLayerHost(layer);
  const inDialog = host !== layer;
  const dismiss = useCallback((id: number) => setNotices(current => current.filter(notice => notice.id !== id)), []);
  const notify = useCallback((notice: ToastNotice) => {
    const stored = { ...notice, id: ++nextId.current };
    setNotices(current => [...current.filter(item => item.key !== notice.key), stored].slice(-MAX_VISIBLE));
  }, []);
  const api = useMemo(() => ({ notify }), [notify]);
  return (
    <ToastContext.Provider value={api}>
      {children}
      {createPortal(
        <div className={styles.host} data-lifted={lifted ? '' : undefined} data-in-dialog={inDialog ? '' : undefined} role="region" aria-label="操作通知">
          {notices.map(notice => <ToastItem key={`${notice.key}:${notice.id}`} notice={notice} onDismiss={dismiss} />)}
        </div>,
        host,
      )}
    </ToastContext.Provider>
  );
}
