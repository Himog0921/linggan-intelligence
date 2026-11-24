import { useEffect, useId, useRef, useState } from 'react';

export interface PickerAccount { ref: string; name: string }

/**
 * The account of the draft, in the editor head (the demo's account button). The name is a button: it opens the accounts of
 * the domain with a search, so a draft that has none yet can be given one right where it is written.
 */
export function AccountPicker({ accounts, value, mark, name, onPick }: {
  accounts: PickerAccount[]; value: string | null; mark: string; name: string; onPick: (ref: string | null) => void;
}) {
  const id = useId();
  const [open, setOpen] = useState(false);
  const [query, setQuery] = useState('');
  const root = useRef<HTMLSpanElement>(null);
  const trigger = useRef<HTMLButtonElement>(null);
  const search = useRef<HTMLInputElement>(null);
  useEffect(() => {
    if (!open) return;
    search.current?.focus({ preventScroll: true });
    const outside = (event: PointerEvent) => { if (event.target instanceof Node && !root.current?.contains(event.target)) setOpen(false); };
    document.addEventListener('pointerdown', outside, true);
    return () => document.removeEventListener('pointerdown', outside, true);
  }, [open]);
  const needle = query.trim().toLocaleLowerCase();
  const shown = accounts.filter(account => account.name.toLocaleLowerCase().includes(needle));
  const close = () => { setOpen(false); trigger.current?.focus({ preventScroll: true }); };
  const pick = (ref: string | null) => { if (ref !== value) onPick(ref); close(); };
  return <span className="acc-pick" ref={root} onKeyDown={event => { if (event.key === 'Escape' && open) { event.stopPropagation(); close(); } }}>
    <button ref={trigger} type="button" className="acc-btn" aria-haspopup="true" aria-expanded={open} aria-controls={open ? id : undefined} aria-label={`写作账号：${name}`}
      onClick={() => { setQuery(''); setOpen(current => !current); }}>
      <span className="avatar">{mark}</span>{name}
    </button>
    {open ? <div className="acc-panel" id={id} role="group" aria-label="选择写作账号">
      <div className="find">
        <svg viewBox="0 0 20 20" aria-hidden="true"><circle cx="8.5" cy="8.5" r="5.5" /><path d="m13 13 4 4" /></svg>
        <input ref={search} aria-label="搜索账号" placeholder="搜账号" value={query} autoComplete="off" onChange={event => setQuery(event.target.value)}
          onKeyDown={event => { if (event.key === 'Enter') { event.preventDefault(); if (shown.length === 1) pick(shown[0]!.ref); } }} />
      </div>
      {accounts.length === 0 ? <p className="aux">这个领域还没有可用的账号。</p>
        : shown.length === 0 ? <p className="aux">没有匹配“{query.trim()}”的账号。</p>
        : <div className="acc-picks">
          {shown.map(account => <button key={account.ref} type="button" aria-pressed={account.ref === value} onClick={() => pick(account.ref)}>{account.name}</button>)}
        </div>}
      {value ? <button type="button" className="btn quiet sub" onClick={() => pick(null)}>不指定账号</button> : null}
    </div> : null}
  </span>;
}
