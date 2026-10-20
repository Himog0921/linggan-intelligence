import { useEffect, useId, useRef, useState, type ReactNode, type CSSProperties } from 'react';
import { createPortal } from 'react-dom';
import { discoveryLabel, moment, runStatus, sourceLabel, type Run } from './tasksApi';
import styles from './RunLedger.module.css';

const fields = [
  ['name', '任务名称'], ['id', '任务编号'], ['stage', '本轮状态'], ['domain', '领域'],
  ['source', '采集来源'], ['scope', '固定 / 上限'], ['mode', '评论范围'], ['rank', '搜索排序'],
  ['details', '可用详情'], ['comments', '可检索评论'], ['reused', '复用详情'],
  ['added', '新增评论'], ['cumulative', '累计评论'], ['next', '发现说明'],
  ['created', '发起时间'], ['updated', '最近进展'], ['action', '操作'],
] as const;
type Field = typeof fields[number][0];
const core: readonly Field[] = ['name', 'id', 'stage', 'action'];
const initial: readonly Field[] = fields.map(([field]) => field);
const numeric = new Set<Field>(['details', 'comments', 'reused', 'added', 'cumulative']);
const count = (value: number | null) => value === null ? '—' : String(value);

function value(run: Run, field: Field): string {
  switch (field) {
    case 'name': return run.name;
    case 'id': return run.run_ref;
    case 'stage': return runStatus(run);
    case 'domain': return run.domain_name;
    case 'source': return sourceLabel(run.source_kind);
    case 'scope': return `${run.scope_frozen_at || run.legacy ? count(run.selected_works) : '—'} / ≤${run.work_limit}`;
    case 'mode': return run.legacy ? '按原工单范围' : run.comment_mode === 'all_public' ? '尽量采全，含回复' : '每篇最多30条总样本';
    case 'rank': return '—'; // The current read contract does not project a ranking.
    case 'details': return count(run.available_details);
    case 'comments': return count(run.delivered_comments);
    case 'reused': return count(run.reused_details);
    case 'added': return count(run.new_comments);
    case 'cumulative': return count(run.cumulative_comments);
    case 'next': return discoveryLabel(run.discovery_reason);
    case 'created': return moment(run.created_at);
    case 'updated': return moment(run.latest_delivery_at ?? run.updated_at);
    case 'action': return '查看进展';
  }
}

/** Business adapter for Run, not a second fact source or a certified generic table. */
export function RunLedger({ runs, message, onOpen, summary, refresh }: { runs: readonly Run[]; message: string; onOpen: (run: Run) => void; summary?: string; refresh?: ReactNode }) {
  const [visible, setVisible] = useState<readonly Field[]>(initial);
  const [expanded, setExpanded] = useState(false);
  const panelId = useId();
  const trigger = useRef<HTMLDetailsElement>(null);
  const panel = useRef<HTMLFieldSetElement>(null);
  const [menu, setMenu] = useState({ above: false, height: '50dvh', right: 0, edge: 0 });
  useEffect(() => {
    if (!expanded) return;
    panel.current?.querySelector<HTMLInputElement>('input:not(:disabled)')?.focus({ preventScroll: true });
    const outside = (event: PointerEvent) => {
      if (event.target instanceof Node && !trigger.current?.contains(event.target) && !panel.current?.contains(event.target) && trigger.current) trigger.current.open = false;
    };
    const dismiss = () => { if (trigger.current) trigger.current.open = false; };
    const focus = (event: FocusEvent) => { if (event.target instanceof Node && !trigger.current?.contains(event.target) && !panel.current?.contains(event.target)) dismiss(); };
    window.addEventListener('resize', dismiss);
    document.addEventListener('focusin', focus);
    document.addEventListener('pointerdown', outside, true);
    return () => { window.removeEventListener('resize', dismiss); document.removeEventListener('focusin', focus); document.removeEventListener('pointerdown', outside, true); };
  }, [expanded]);
  const columns = fields.filter(([field]) => visible.includes(field));
  const tableWidth = `calc(${columns.map(([field]) => `var(--lgi-ledger-col-${field})`).join(' + ')})`;
  return <div className={styles.ledger}>
    <div className={styles.settings}>
      <span role="status">{summary ?? (runs.length > 0 ? `本页 ${runs.length} 次采集` : '')}</span>{refresh}
      <details ref={trigger} className={styles.fields} onToggle={event => {
        setExpanded(event.currentTarget.open);
        if (!event.currentTarget.open) return;
        const bounds = event.currentTarget.getBoundingClientRect();
        const gap = parseFloat(getComputedStyle(event.currentTarget).getPropertyValue('--lgi-space-4'));
        const below = window.innerHeight - bounds.bottom - gap;
        const above = bounds.top - gap;
        const opensAbove = above > below;
        setMenu({ above: opensAbove, height: `${Math.max(0, opensAbove ? above : below)}px`, right: Math.max(gap, window.innerWidth - bounds.right), edge: opensAbove ? window.innerHeight - bounds.top + gap / 2 : bounds.bottom + gap / 2 });
      }} onKeyDown={event => {
        if (event.key === 'Escape') { event.currentTarget.open = false; event.currentTarget.querySelector('summary')?.focus(); }
      }}>
        <summary aria-controls={panelId} aria-expanded={expanded}>字段 <b>{columns.length} / {fields.length}</b><span aria-hidden="true">⌄</span></summary>
        {expanded && createPortal(<fieldset id={panelId} ref={panel} className={styles.fieldPanel} style={{ maxHeight: menu.height, right: menu.right, ...(menu.above ? { bottom: menu.edge } : { top: menu.edge }) }}><legend>显示字段</legend>{fields.map(([field, label]) => <label key={field}>
          <input type="checkbox" aria-label={label} checked={visible.includes(field)} disabled={core.includes(field)} onChange={event => setVisible(current => event.target.checked ? [...current, field] : current.filter(v => v !== field))}/>
          {label}{core.includes(field) && <small>固定</small>}
        </label>)}<button type="button" onClick={() => setVisible(initial)}>恢复默认字段</button></fieldset>, document.body)}
      </details>
    </div>
    <div className={styles.scroll} role="region" aria-label="采集任务账本，可横向滚动" tabIndex={0}>
      <table className={styles.table} style={{ width: tableWidth } as CSSProperties}>
        <caption className="v7-sr-only">采集任务，每个任务一行；完整名称和编号可在单元格提示中核对</caption>
        <colgroup>{columns.map(([field]) => <col key={field} style={{ width: `var(--lgi-ledger-col-${field})` }}/>)}</colgroup>
        <thead><tr>{columns.map(([field, label]) => <th key={field} scope="col" data-field={field} className={numeric.has(field) ? styles.number : undefined}>{label}</th>)}</tr></thead>
        <tbody>{runs.map(run => <tr key={run.run_ref} data-run-row={run.run_ref}>{columns.map(([field]) => {
          const text = value(run, field);
          const originalTime = field === 'created' ? run.created_at : field === 'updated' ? run.latest_delivery_at ?? run.updated_at : null;
          const title = originalTime ?? (field === 'rank' ? '当前读取合同未提供搜索排序，请核对原执行记录' : text === '—' ? '尚未取得，不能据此记为0' : text);
          return <td key={field} data-field={field} className={numeric.has(field) ? styles.number : undefined} title={title}>
            {field === 'name' || field === 'action' ? <button type="button" className={field === 'name' ? styles.name : styles.open} data-open-run={run.run_ref} data-run-domain={run.domain_ref} onClick={() => onOpen(run)}>{text}</button> : field === 'stage' ? <span className={styles.stage} data-attention={run.needs_attention ? '' : undefined}>{text}</span> : originalTime !== null ? <time className={styles.data} dateTime={originalTime}>{text}</time> : <span className={field === 'id' ? styles.identifier : undefined}>{text}</span>}
          </td>;
        })}</tr>)}{runs.length === 0 && <tr><td colSpan={columns.length} className={styles.empty} role="status">{message}</td></tr>}</tbody>
      </table>
    </div>
  </div>;
}
