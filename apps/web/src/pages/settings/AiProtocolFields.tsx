import { useState } from 'react';
import { ActionButton } from '../../shared/ActionButton';
import { Field } from './AiSettingsFields';
import { THINKING, type AiSettings, type Json } from './aiSettingsModel';
import styles from './AiSettings.module.css';

type Entry = { id: number; key: string; kind: string; value: string; omit: boolean };
function entries(value: Json | undefined): Entry[] {
  if (!value || typeof value !== 'object' || Array.isArray(value)) return [];
  return Object.entries(value).map(([key, v], id) => ({ id, key, kind: v && typeof v === 'object' && !Array.isArray(v) && typeof v.$var === 'string' ? v.$var : typeof v,
    value: v === null ? '' : typeof v === 'object' ? '' : String(v), omit: v !== null && typeof v === 'object' && !Array.isArray(v) && v.omitWhenOff === true }));
}
function TemplateFields({ name, value }: { name: string; value: Json | undefined }) {
  const [rows, setRows] = useState(() => entries(value));
  const [nextId, setNextId] = useState(rows.length);
  return <fieldset><legend>{name} 扩展参数</legend>{rows.map(row => <div className={styles.grid} key={row.id}>
    <Field label="参数名"><input name={`template:${name}:${row.id}:key`} required maxLength={80} defaultValue={row.key} /></Field>
    <Field label="参数类型"><select name={`template:${name}:${row.id}:kind`} defaultValue={row.kind}><option value="string">文本</option><option value="number">数值</option><option value="boolean">布尔值</option><option value="thinking.enabled">引用是否开启思考</option><option value="thinking.effort">引用思考等级</option><option value="thinking.budget">引用思考预算</option></select></Field>
    <Field label="固定参数值" hint="布尔值填 true 或 false；引用类型忽略此值。"><input name={`template:${name}:${row.id}:value`} defaultValue={row.value} maxLength={200} /></Field>
    <label><input name={`template:${name}:${row.id}:omit`} type="checkbox" defaultChecked={row.omit} /> 思考关闭时省略引用参数</label>
    <ActionButton onClick={() => setRows(rows.filter(r => r.id !== row.id))}>删除参数 {row.key || row.id + 1}</ActionButton>
  </div>)}<ActionButton onClick={() => { setRows([...rows, { id: nextId, key: '', kind: 'string', value: '', omit: false }]); setNextId(nextId + 1); }}>添加 {name} 参数</ActionButton></fieldset>;
}
export function ProtocolFields({ spec, compat, mapping }: { spec: AiSettings['protocol']['apis'][string]; compat: Record<string, Json>; mapping: Record<string, Json> }) {
  return <details><summary>协议兼容与思考参数</summary><div className={styles.grid}>
    {spec.booleans.map(key => <Field key={key} label={key}><select name={`compat:${key}`} defaultValue={typeof compat[key] === 'boolean' ? String(compat[key]) : ''}><option value="">未指定</option><option value="true">是</option><option value="false">否</option></select></Field>)}
    {Object.entries(spec.enums).map(([key, options]) => <Field key={key} label={key}><select name={`compat:${key}`} defaultValue={typeof compat[key] === 'string' ? compat[key] : ''}><option value="">未指定</option>{options.map(v => <option key={v}>{v}</option>)}</select></Field>)}
    {Object.entries(spec.numbers).map(([key, range]) => <Field key={key} label={key}><input type="number" name={`compat:${key}`} min={range.min} max={range.max} defaultValue={typeof compat[key] === 'number' ? compat[key] : ''} /></Field>)}
    {Object.entries(THINKING).map(([level, label]) => <Field key={level} label={`${label}的协议值`}><input name={`thinking:${level}`} maxLength={80} defaultValue={typeof mapping[level] === 'string' ? mapping[level] : ''} placeholder="留空时由适配器默认映射" /></Field>)}
  </div><div className={styles.group}>{spec.templates.map(key => <TemplateFields key={key} name={key} value={compat[key]} />)}</div><p><small>字段来自当前协议合同；扩展参数只影响请求格式，不改变地址、密钥或许可。</small></p></details>;
}
