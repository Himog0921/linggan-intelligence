import { useEffect, useRef, useState } from 'react';

interface Scope { domainRef: string; name: string; accountCount?: number }
/** Domain identity stays in the context rail; data and creation retain their own eligible scopes. */
export function OpsScopePicker({ options, value, onChange, disabled, loading, failed, creation = false }: {
  options: readonly Scope[]; value: string; onChange: (value: string) => void;
  disabled?: boolean; loading?: boolean; failed?: boolean; creation?: boolean;
}) {
  const [open, setOpen] = useState(false);
  const ref = useRef<HTMLElement>(null);
  const root = useRef<HTMLDetailsElement>(null);
  useEffect(() => {
    if (!open) return;
    const outside = (event: PointerEvent) => {
      if (event.target instanceof Node && !root.current?.contains(event.target)) setOpen(false);
    };
    document.addEventListener('pointerdown', outside, true);
    return () => document.removeEventListener('pointerdown', outside, true);
  }, [open]);
  const selected = options.find(option => option.domainRef === value);
  const name = selected?.name ?? (creation ? value ? '领域不可用' : '选择领域' : '全部业务');
  const unavailable = disabled || loading || failed;
  return <details ref={root} className="v7-domain-picker" open={open && !unavailable} onToggle={event => setOpen(event.currentTarget.open)}
    onBlur={event => { if (!event.currentTarget.contains(event.relatedTarget)) setOpen(false); }}
    onKeyDown={event => { if (event.key === 'Escape') { event.preventDefault(); setOpen(false); ref.current?.focus(); } }}>
    <summary ref={ref} aria-label={creation ? '创作领域' : '切换业务'} aria-disabled={unavailable || undefined}
      onClick={event => { if (unavailable) event.preventDefault(); }}>
      <span>{loading ? '读取领域…' : failed ? '领域读取失败' : name}</span>
      <small>{creation ? '创作领域' : selected ? `${selected.accountCount ?? '—'} 个账号` : '全部账号'}</small><i aria-hidden="true"/>
    </summary>
    <nav aria-label={creation ? '可选创作领域' : '可选业务'}>
      {(!creation ? [{ domainRef: 'all', name: '全部业务' }, ...options] : options).map(option => <button type="button" key={option.domainRef}
        aria-current={value === option.domainRef ? 'page' : undefined} disabled={unavailable}
        onClick={() => { if (unavailable) return; onChange(option.domainRef); setOpen(false); ref.current?.focus(); }}>
        <span>{option.name}</span>{'accountCount' in option && option.accountCount !== undefined ? <small>{option.accountCount} 个账号</small> : null}
      </button>)}
      {creation && !options.length ? <span>暂无可用创作领域</span> : null}
    </nav>
  </details>;
}
