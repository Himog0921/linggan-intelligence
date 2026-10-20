import { useQuery } from '@tanstack/react-query';
import { useState, type FormEvent } from 'react';
import { isRecord } from '../../shared/api';
import { command, fetchCatalog } from './aiSettingsApi';
import { OUTPUT, THINKING, type AiModel, type AiSettings, type Connection } from './aiSettingsModel';
import { Field, SaveBar } from './AiSettingsFields';
import type { AiAction } from './useAiAction';
import { ActionButton } from '../../shared/ActionButton';
import styles from './AiSettings.module.css';
import { ProtocolFields } from './AiProtocolFields';
import { readProtocolFields } from './aiProtocolForm';
import { numeric } from './aiSettingsForms';

export function AiCapabilityForm({ data, model, connection, action }: { data: AiSettings; model: AiModel; connection: Connection; action: AiAction }) {
  const [catalogOpen, setCatalogOpen] = useState(false);
  const catalog = useQuery({ queryKey: ['ai-catalog', connection.api, model.modelId], queryFn: ({ signal }) => fetchCatalog(connection.api, model.modelId, signal), enabled: catalogOpen, retry: false });
  const [presetIndex, setPresetIndex] = useState('');
  const preset = catalog.data?.presets.find((_, index) => String(index) === presetIndex);
  const cap = model.capability;
  const spec = data.protocol.apis[connection.api];
  const draft = preset ?? cap;
  const compat = draft && isRecord(draft.compat) ? draft.compat : {};
  const [editedPreset, setEditedPreset] = useState(false);
  if (!spec) return <p>此协议只保留历史读取，不能在本页修改能力。</p>;
  async function save(event: FormEvent<HTMLFormElement>) {
    event.preventDefault(); const form = new FormData(event.currentTarget);
    try {
      const protocol = readProtocolFields(form, spec!);
      const payload = { identityRef: model.identityRef, endpointRef: model.endpointRef, expectedCapabilityRef: cap?.capabilityRef ?? null,
        source: preset && !editedPreset ? 'pi_catalog' : 'manual', sourceVersion: String(form.get('sourceVersion')), probeInvocationRef: null,
        limits: { contextTokens: numeric(form, 'contextTokens'), maxOutputTokens: numeric(form, 'maxOutputTokens'), maxTimeoutSeconds: numeric(form, 'maxTimeoutSeconds'), thinkingLevels: form.getAll('thinkingLevels') },
        ...protocol, outputMode: String(form.get('outputMode')), strictSchema: form.get('strictSchema') === 'on' };
      await action.run(`capability:${model.identityRef}`, payload, commandRef => command('capabilities', { ...payload, commandRef }), '能力声明已保存。这是声明来源，当前密钥的实际调用能力仍需单独测试。');
    } catch (error) { action.setNotice({ tone: 'error', text: error instanceof Error ? error.message : '请核对能力声明。' }); }
  }
  return <div className={styles.group}><div className={styles.row}><h3>能力声明</h3><ActionButton onClick={() => setCatalogOpen(true)}>读取本机离线模型目录</ActionButton></div>
    <p>当前来源：{cap ? `${cap.source} · ${cap.sourceVersion}` : '尚未声明'}。目录只提供参考，不证明当前地址可用。</p>
    {catalogOpen ? <><Field label="目录参考"><select value={presetIndex} onChange={event => { setPresetIndex(event.target.value); setEditedPreset(false); }}><option value="">手动声明</option>{catalog.data?.presets.map((p, i) => <option key={`${p.provider}:${p.modelId}`} value={i} disabled={!p.supported}>{p.provider} · {p.name}{p.supported ? '' : '（当前不支持）'}</option>)}</select></Field>{catalog.isPending ? <p>正在读取离线目录…</p> : catalog.isError ? <p role="alert">离线目录读取失败，可以继续手动填写。</p> : !catalog.data?.presets.length ? <p>未找到匹配的目录项，请手动声明。</p> : null}</> : null}
    <form key={`${model.identityRef}:${cap?.capabilityRef ?? ''}:${presetIndex}`} className={styles.form} onChange={() => setEditedPreset(true)} onSubmit={event => void save(event)}>
      <div className={styles.grid}><Field label="上下文容量"><input type="number" name="contextTokens" min={1024} max={data.resources.maxContextTokens} required defaultValue={preset?.limits.contextTokens ?? cap?.contextTokens} /></Field>
        <Field label="最大输出容量"><input type="number" name="maxOutputTokens" min={16} max={data.resources.maxOutputTokens} required defaultValue={preset?.limits.maxOutputTokens ?? cap?.maxOutputTokens} /></Field>
        <Field label="最长等待秒数" hint="目录不提供此值，请按测试情况填写。"><input type="number" name="maxTimeoutSeconds" min={1} max={data.resources.maxTimeoutSeconds} required defaultValue={cap?.maxTimeoutSeconds} /></Field>
        <Field label="声明依据及版本"><input name="sourceVersion" defaultValue={preset?.sourceVersion ?? cap?.sourceVersion ?? ''} maxLength={200} required placeholder="例如供应商文档日期或本次人工测试说明" /></Field></div>
      <fieldset><legend>已声明支持的思考等级</legend><div className={styles.checklist}>{data.protocol.thinkingLevels.map(level => <label key={level}><input type="checkbox" name="thinkingLevels" value={level} defaultChecked={(preset?.limits.thinkingLevels ?? cap?.thinkingLevels ?? []).includes(level)} />{THINKING[level] ?? level}</label>)}</div></fieldset>
      <div className={styles.grid}><Field label="输出方式"><select name="outputMode" defaultValue={draft?.outputMode ?? 'prompt_only'}>{spec.outputModes.map(mode => <option key={mode} value={mode}>{OUTPUT[mode] ?? mode}</option>)}</select></Field><label><input name="strictSchema" type="checkbox" defaultChecked={draft?.strictSchema ?? false} /> 严格结构约束</label></div>
      <p><small>{preset && !editedPreset ? '将按离线目录原值保存；修改字段后标为人工声明。' : '将按人工声明保存。'}</small></p><ProtocolFields spec={spec} compat={compat} mapping={draft && isRecord(draft.thinkingLevelMap) ? draft.thinkingLevelMap : {}} /><SaveBar busy={action.busy !== null}>保存能力声明</SaveBar>
    </form>
    <PriceForm key={model.price?.priceRef ?? model.identityRef} model={model} action={action} />
  </div>;
}
function PriceForm({ model, action }: { model: AiModel; action: AiAction }) {
  async function save(event: FormEvent<HTMLFormElement>) {
    event.preventDefault(); const form = new FormData(event.currentTarget);
    const prices = Object.fromEntries(['input', 'output', 'cachedRead', 'cachedWrite'].map(key => [key, String(form.get(key)) || null]));
    const payload = { identityRef: model.identityRef, endpointRef: model.endpointRef, expectedPriceRef: model.price?.priceRef ?? null, source: 'manual', sourceVersion: String(form.get('priceSource')), prices };
    await action.run(`price:${model.identityRef}`, payload, commandRef => command('prices', { ...payload, commandRef }), '价格已保存；历史报价保留原价格版本。');
  }
  return <form className={styles.form} onSubmit={event => void save(event)}><h3>价格</h3><p>单位：每百万 token 的微美元（1 美元 = 1,000,000 微美元）。留空表示未知，明确填写 0 才表示零价。</p><div className={styles.grid}>
    {Object.entries({ input: '输入价格', output: '输出价格', cachedRead: '缓存读取价格', cachedWrite: '缓存写入价格' }).map(([key, label]) => <Field key={key} label={label}><input name={key} inputMode="numeric" pattern="[0-9]*" defaultValue={model.price?.[key as 'input' | 'output' | 'cachedRead' | 'cachedWrite'] ?? ''} /></Field>)}
    <Field label="价格依据及版本"><input name="priceSource" required maxLength={200} defaultValue={model.price?.sourceVersion ?? ''} /></Field></div><SaveBar busy={action.busy !== null}>保存价格</SaveBar></form>;
}
