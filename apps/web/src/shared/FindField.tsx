import { useEffect, useImperativeHandle, useRef, useState, type ReactNode, type Ref } from 'react';
import { FIND_DEBOUNCE_MS, type FindTrigger } from './lidsComponents.generated';
import styles from './FindField.module.css';

export interface FindFieldHandle {
  flush: () => void;
  focus: () => void;
  read: () => string;
  cancel: () => void;
}
interface FindFieldProps {
  value: string;
  onCommit: (value: string) => void;
  label: string;
  placeholder?: string;
  trigger: FindTrigger;
  scope?: ReactNode;
  contextKey?: string;
  handle?: Ref<FindFieldHandle>;
  inputId?: string;
  formId?: string;
  name?: string;
  maxLength?: number;
  disabled?: boolean;
  busy?: boolean;
  title?: string;
  trim?: boolean;
  shortcut?: boolean;
  clearLabel?: string;
  submitLabel?: string;
  className?: string;
}

/** C16 owns draft/IME/commit mechanics; pages own scope, URL, results and commands. */
export function FindField({ value, onCommit, label, placeholder, trigger, scope, contextKey = '', handle,
  inputId, formId, name, maxLength, disabled = false, busy = false, title, trim = true,
  shortcut = false, clearLabel = '清空搜索', submitLabel, className }: FindFieldProps) {
  const [text, setText] = useState(value);
  const [seen, setSeen] = useState({ value, contextKey });
  if (seen.value !== value || seen.contextKey !== contextKey) {
    setSeen({ value, contextKey }); setText(value);
  }
  const input = useRef<HTMLInputElement>(null);
  const timer = useRef<number | undefined>(undefined);
  const composing = useRef(false);
  const latest = useRef({ value, onCommit, disabled, busy, trim });
  useEffect(() => { latest.current = { value, onCommit, disabled, busy, trim }; });
  useEffect(() => () => window.clearTimeout(timer.current), []);
  useEffect(() => { window.clearTimeout(timer.current); }, [value, contextKey, trigger, disabled, busy]);
  function commit(next: string, force = false) {
    window.clearTimeout(timer.current);
    const current = latest.current;
    if (composing.current || current.disabled || current.busy) return;
    const normalized = current.trim ? next.trim() : next;
    if (force || normalized !== current.value) current.onCommit(normalized);
  }
  function schedule(next: string) {
    window.clearTimeout(timer.current);
    if (trigger === 'debounce_250') timer.current = window.setTimeout(() => commit(next), FIND_DEBOUNCE_MS);
  }
  useImperativeHandle(handle, () => ({
    flush: () => commit(input.current?.value ?? text),
    focus: () => input.current?.focus(),
    read: () => input.current?.value ?? text,
    cancel: () => window.clearTimeout(timer.current),
  }));
  useEffect(() => {
    if (!shortcut || disabled) return;
    function key(event: KeyboardEvent) {
      if (event.defaultPrevented || event.key !== '/' || event.ctrlKey || event.metaKey || event.altKey || event.isComposing || document.querySelector('dialog[open]')) return;
      const target = event.target;
      if (target instanceof HTMLElement && (target.closest('input,textarea,select,[role="combobox"],[role="listbox"]') || target.isContentEditable)) return;
      event.preventDefault(); input.current?.focus();
    }
    document.addEventListener('keydown', key);
    return () => document.removeEventListener('keydown', key);
  }, [shortcut, disabled]);
  return <form id={formId} className={[styles.find, className].filter(Boolean).join(' ')} data-find data-lids-component="C16" role="search" aria-label={label} aria-busy={busy || undefined} title={title}
    onSubmit={event => { event.preventDefault(); commit(text, trigger === 'submit'); }}>
    <span className={styles.mark} data-find-mark aria-hidden="true">FIND</span>
    {scope}
    <input id={inputId} ref={input} type="search" name={name} aria-label={label} autoComplete="off" value={text} maxLength={maxLength} placeholder={placeholder} disabled={disabled} title={title}
      onChange={event => { setText(event.currentTarget.value); if (!composing.current) schedule(event.currentTarget.value); }}
      onCompositionStart={() => { composing.current = true; window.clearTimeout(timer.current); }}
      onCompositionEnd={event => { composing.current = false; schedule(event.currentTarget.value); }}
      onKeyDown={event => { if (event.key === 'Enter' && (composing.current || event.nativeEvent.isComposing || event.nativeEvent.keyCode === 229)) event.preventDefault(); }} />
    {text ? <button type="button" className={styles.clear} aria-label={clearLabel} disabled={disabled || busy}
      onClick={() => { window.clearTimeout(timer.current); setText(''); commit('', true); input.current?.focus(); }}>清空</button> : <kbd aria-hidden="true">/</kbd>}
    {submitLabel ? <button type="submit" className={styles.submit} disabled={disabled || busy}>{submitLabel}</button> : null}
  </form>;
}
