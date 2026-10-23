import { useId, useState } from 'react';
import { ActionButton } from '../../../shared/ActionButton';
import type { DossierBlock, Speaker } from './dossierTypes';
import styles from './Dossier.module.css';
const KIND: Record<DossierBlock['kind'], string> = { heading: '标题', paragraph: '段落', quote: '引用', gap: '缺口', list: '列表', table: '表格', dialogue: '对话' };
function SpeakerInput({ value, onChange }: { value: Speaker; onChange: (value: Speaker) => void }) {
  return <select aria-label="表达者" value={value} onChange={event => onChange(event.target.value as Speaker)}><option value="author">作品作者</option><option value="other">他人表述</option><option value="unconfirmed">未确认</option></select>;
}
function EditList({ block, onChange }: { block: Extract<DossierBlock, { kind: 'list' }>; onChange: (value: DossierBlock) => void }) {
  return <><label className={styles.checkLabel}><input type="checkbox" checked={block.ordered} onChange={event => onChange({ ...block, ordered: event.target.checked })} />有序列表</label>
    {block.ordered ? <label>起始编号<input type="number" min={1} value={block.start ?? 1} onChange={event => { const start = Number(event.target.value); if (Number.isSafeInteger(start) && start >= 1) onChange({ ...block, start }); }} /></label> : null}
    {block.items.map((text, index) => <div key={index}><label>{`第 ${index + 1} 项`}<textarea value={text} onChange={event => onChange({ ...block, items: block.items.map((item, n) => n === index ? event.target.value : item) })} /></label><button type="button" disabled={block.items.length <= 1} onClick={() => onChange({ ...block, items: block.items.filter((_, n) => n !== index) })}>移除此项</button></div>)}
    <button type="button" onClick={() => onChange({ ...block, items: [...block.items, ''] })}>添加列表项</button></>;
}
function EditTable({ block, onChange }: { block: Extract<DossierBlock, { kind: 'table' }>; onChange: (value: DossierBlock) => void }) {
  return <><div className={styles.editorGrid}>{block.columns.map((text, index) => <div key={index}><label>{`第 ${index + 1} 列标题`}<textarea value={text} onChange={event => onChange({ ...block, columns: block.columns.map((item, n) => n === index ? event.target.value : item) })} /></label><button type="button" disabled={block.columns.length <= 1} onClick={() => onChange({ ...block, columns: block.columns.filter((_, n) => n !== index), rows: block.rows.map(row => row.filter((_, n) => n !== index)) })}>移除此列</button></div>)}</div>
    {block.rows.map((row, index) => <div key={index}><div className={styles.editorGrid}>{row.map((text, column) => <label key={column}>{`第 ${index + 1} 行，第 ${column + 1} 列`}<textarea value={text} onChange={event => onChange({ ...block, rows: block.rows.map((cells, n) => n === index ? cells.map((item, c) => c === column ? event.target.value : item) : cells) })} /></label>)}</div><button type="button" onClick={() => onChange({ ...block, rows: block.rows.filter((_, n) => n !== index) })}>移除此行</button></div>)}
    <div className={styles.toolbar}><button type="button" onClick={() => onChange({ ...block, rows: [...block.rows, block.columns.map(() => '')] })}>添加表格行</button><button type="button" onClick={() => onChange({ ...block, columns: [...block.columns, ''], rows: block.rows.map(row => [...row, '']) })}>添加表格列</button></div></>;
}
function EditContent({ block, onChange, id }: { block: DossierBlock; onChange: (value: DossierBlock) => void; id: string }) {
  if (block.kind === 'list') return <EditList block={block} onChange={onChange} />;
  if (block.kind === 'table') return <EditTable block={block} onChange={onChange} />;
  if (block.kind === 'dialogue') return <>{block.turns.map((turn, index) => <div key={index}><SpeakerInput value={turn.speaker} onChange={speaker => onChange({ ...block, turns: block.turns.map((item, n) => n === index ? { ...item, speaker } : item) })} /><label>{`第 ${index + 1} 轮发言`}<textarea value={turn.text} onChange={event => onChange({ ...block, turns: block.turns.map((item, n) => n === index ? { ...item, text: event.target.value } : item) })} /></label><button type="button" disabled={block.turns.length <= 1} onClick={() => onChange({ ...block, turns: block.turns.filter((_, n) => n !== index) })}>移除此轮</button></div>)}<button type="button" onClick={() => onChange({ ...block, turns: [...block.turns, { speaker: 'unconfirmed', text: '' }] })}>添加对话轮次</button></>;
  return <>{block.kind === 'heading' ? <label>原文标题层级<select value={block.level ?? ''} onChange={event => { const level = event.target.value === '' ? undefined : Number(event.target.value); const next = { ...block }; if (level === undefined) delete next.level; else next.level = level; onChange(next); }}><option value="">未标记层级</option>{[1,2,3,4,5,6].map(level => <option key={level} value={level}>{level}</option>)}</select></label> : null}<label htmlFor={id}>{KIND[block.kind]}文字<textarea id={id} value={block.text} onChange={event => onChange({ ...block, text: event.target.value })} /></label></>;
}
function newBlock(kind: DossierBlock['kind']): DossierBlock {
  const base = { ref: `edit-${crypto.randomUUID()}`, speaker: 'unconfirmed' as const };
  if (kind === 'list') return { ...base, kind, ordered: false, items: [''] };
  if (kind === 'table') return { ...base, kind, columns: [''], rows: [['']] };
  if (kind === 'dialogue') return { ...base, kind, turns: [{ speaker: 'unconfirmed', text: '' }] };
  return { ...base, kind, text: '' };
}
/** Structure edits never manufacture source coordinates or timestamps. */
export function DossierBlockEditor({ blocks, onChange, disabled }: { blocks: readonly DossierBlock[]; onChange: (blocks: DossierBlock[]) => void; disabled: boolean }) {
  const id = useId(); const [kind, setKind] = useState<DossierBlock['kind']>('paragraph');
  function move(index: number, by: number) { const next = [...blocks]; [next[index], next[index + by]] = [next[index + by]!, next[index]!]; onChange(next); }
  return <fieldset className={styles.blockEditor} disabled={disabled}><legend>逐块修正完整读本</legend>{blocks.map((block, index) => <section key={block.ref} className={styles.editorBlock}>
    <h4>{`${index + 1} · ${KIND[block.kind]}`}</h4>
    {block.kind !== 'dialogue' ? <SpeakerInput value={block.speaker ?? 'unconfirmed'} onChange={speaker => onChange(blocks.map((value, n) => n === index ? { ...value, speaker } : value))} /> : null}
    <EditContent id={`${id}-${index}`} block={block} onChange={value => onChange(blocks.map((item, n) => n === index ? value : item))} />
    <div className={styles.toolbar}><button type="button" disabled={index === 0} onClick={() => move(index, -1)}>上移区块</button><button type="button" disabled={index === blocks.length - 1} onClick={() => move(index, 1)}>下移区块</button><button type="button" onClick={() => onChange(blocks.filter((_, n) => n !== index))}>移除此区块</button></div>
  </section>)}<div className={styles.toolbar}><label>新块类型<select value={kind} onChange={event => setKind(event.target.value as DossierBlock['kind'])}>{(Object.keys(KIND) as DossierBlock['kind'][]).map(key => <option key={key} value={key}>{KIND[key]}</option>)}</select></label><ActionButton type="button" onClick={() => onChange([...blocks, newBlock(kind)])}>添加区块</ActionButton></div></fieldset>;
}
