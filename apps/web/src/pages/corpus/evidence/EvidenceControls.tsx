import { useEffect, useId, useRef, useState, type FormEvent, type ReactNode } from 'react';
import { ActionButton } from '../../../shared/ActionButton';
import { SelectField } from '../../../shared/SelectField';
import { LANES, LANE_STATES, LAYOUTS, MEDIA_KINDS, VIEWS, activeFilterCount, type EvidenceQuery, type LayoutKey, type ViewKey } from './evidenceQuery';
import styles from './Controls.module.css';

/** 来自页面的改动入口：replace 为真时替换当前历史，否则新增一条。 */
export type Change = (update: (current: EvidenceQuery) => EvidenceQuery, replace?: boolean) => void;
interface EvidenceControlsProps {
  query: EvidenceQuery;
  hasDomain: boolean;
  scope?: string | null;
  busy?: boolean;
  onChange: Change;
  /** 手动提交；相同条件也重新读取，不改成自动检索。 */
  onSubmit: (text: string) => void;
  onClearFilters: () => void;
  banners?: ReactNode;
  children?: ReactNode;
}
const NO_DOMAIN = '请先选择研究领域，再读取统一作品材料。';
type FilterName = 'lane' | 'laneState' | 'mediaKind';
const FILTERS: readonly { name: FilterName; label: string; allLabel: string; options: readonly (readonly [string, string])[] }[] = [
  { name: 'lane', label: '材料通道', allLabel: '全部材料通道', options: LANES },
  { name: 'laneState', label: '通道状态', allLabel: '全部状态', options: LANE_STATES },
  { name: 'mediaKind', label: '媒体类型', allLabel: '全部类型', options: MEDIA_KINDS },
];

/** B检索组合。筛选即选即读，没有待应用草稿；对象与地址合同仍由页面持有。 */
export function EvidenceControls({ query, hasDomain, scope, busy = false, onChange, onSubmit, onClearFilters, banners, children }: EvidenceControlsProps) {
  const panelId = useId();
  const [text, setText] = useState(query.q);
  const [seen, setSeen] = useState({ q: query.q, scope });
  if (seen.q !== query.q || seen.scope !== scope) { setSeen({ q: query.q, scope }); setText(query.q); }
  const [open, setOpen] = useState(false);
  const toggle = useRef<HTMLButtonElement>(null);
  const anchor = useRef<HTMLDivElement>(null);
  const input = useRef<HTMLInputElement>(null);
  const composing = useRef(false);
  const reason = hasDomain ? undefined : NO_DOMAIN;

  useEffect(() => {
    const shortcut = (event: KeyboardEvent) => {
      if (event.key !== '/' || event.ctrlKey || event.metaKey || event.altKey || !hasDomain || document.querySelector('dialog[open]')) return;
      const target = event.target;
      if (target instanceof HTMLElement && (target.closest('input,textarea,select,[role="combobox"]') || target.isContentEditable)) return;
      event.preventDefault(); input.current?.focus();
    };
    document.addEventListener('keydown', shortcut);
    return () => document.removeEventListener('keydown', shortcut);
  }, [hasDomain]);

  useEffect(() => {
    if (!open) return;
    const outside = (event: PointerEvent) => {
      if (event.target instanceof Node && !anchor.current?.contains(event.target)) setOpen(false);
    };
    document.addEventListener('pointerdown', outside);
    return () => document.removeEventListener('pointerdown', outside);
  }, [open]);

  function submit(event: FormEvent) {
    event.preventDefault();
    if (!hasDomain || composing.current || busy) return;
    setOpen(false); onSubmit(text);
  }
  const count = activeFilterCount(query);
  return <section aria-label="材料检索与筛选" className={styles.deck}>
    {banners}
    <div className={styles.band}>
      <form className={styles.form} role="search" onSubmit={submit}>
        <div className={styles.find}>
          <span className={styles.findMark} aria-hidden="true">FIND</span>
          <input ref={input} type="search" name="q" autoComplete="off" aria-label="检索作品与证据" placeholder="标题、作者或原声片段……"
            value={text} disabled={!hasDomain} title={reason} onChange={event => setText(event.currentTarget.value)}
            onCompositionStart={() => { composing.current = true; }} onCompositionEnd={() => { composing.current = false; }}
            onKeyDown={event => { if (event.key === 'Enter' && (composing.current || event.nativeEvent.isComposing || event.nativeEvent.keyCode === 229)) event.preventDefault(); }} />
          {text ? <button type="button" className={styles.clear} aria-label="清空检索" disabled={!hasDomain || busy}
            onClick={() => { setText(''); onSubmit(''); input.current?.focus(); }}>×</button> : <kbd aria-hidden="true">/</kbd>}
        </div>
        <div className={styles.tools}>
          <div ref={anchor} className={styles.anchor} onKeyDown={event => {
            if (event.key === 'Escape' && open) { event.preventDefault(); event.stopPropagation(); setOpen(false); toggle.current?.focus(); }
          }}>
            <ActionButton ref={toggle} variant="outline" className={styles.filterToggle} aria-expanded={open} aria-controls={panelId}
              data-active={count > 0} disabled={!hasDomain} title={reason} onClick={() => setOpen(value => !value)}>
              筛选 <b aria-label={`已选 ${count} 项筛选`}>{String(count).padStart(2, '0')}</b>
            </ActionButton>
            {open ? <div id={panelId} role="group" aria-label="筛选条件" className={styles.panel}>
              <header><strong>筛选材料</strong><span>选择后立即读取</span></header>
              {FILTERS.map(({ name, label, allLabel, options }) => <div key={name} className={styles.field}>
                <span className={styles.fieldLabel}>{label}</span>
                <SelectField label={label} value={query[name]} options={[{ value: '', label: allLabel }, ...options.map(([value, label]) => ({ value, label }))]}
                  onChange={value => onChange(current => ({ ...current, [name]: value, work: null }))} />
              </div>)}
              {query.view !== 'all' ? <p className={styles.scopeNote}>当前系统视图会优先采用其预设条件；需自由筛选时请选择「全部材料」。</p> : null}
              <footer className={styles.panelFoot}>
                <ActionButton variant="quiet" onClick={onClearFilters}>清空筛选</ActionButton>
                <ActionButton variant="outline" onClick={() => { setOpen(false); toggle.current?.focus(); }}>完成</ActionButton>
              </footer>
            </div> : null}
          </div>
          <ActionButton type="submit" variant="primary" disabled={!hasDomain} busy={busy} title={reason}>读取材料</ActionButton>
        </div>
      </form>
      <div className={styles.views} role="group" aria-label="按材料状态快速筛选">
        {VIEWS.map(([key, label]) => <button key={key} type="button" aria-pressed={query.view === key} disabled={!hasDomain} title={reason}
          onClick={() => onChange(current => ({ ...current, view: key satisfies ViewKey, work: null }))}>{label}</button>)}
      </div>
    </div>
    <div className={styles.foot}>
      {children}
      <div className={styles.layouts} role="group" aria-label="结果排版">
        {LAYOUTS.map(([key, label]) => <button key={key} type="button" aria-pressed={query.layout === key}
          onClick={() => onChange(current => ({ ...current, layout: key satisfies LayoutKey }))}>{label}</button>)}
      </div>
    </div>
  </section>;
}
