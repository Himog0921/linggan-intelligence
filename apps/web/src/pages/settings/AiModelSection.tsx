import { useState, type FormEvent } from 'react';
import { Link, useSearchParams } from 'react-router';
import { ActionButton } from '../../shared/ActionButton';
import { saveConnection, saveModel, setConnectionEnabled } from './aiConnectionApi';
import { command } from './aiSettingsApi';
import { MATERIAL, type AiModel, type AiSettings, type Connection } from './aiSettingsModel';
import { Field, Impact, Moment, Reference, SaveBar } from './AiSettingsFields';
import { LimitsFields } from './AiTaskSection';
import { readLimits } from './aiSettingsForms';
import { AiCapabilityForm } from './AiCapabilityForm';
import type { AiAction } from './useAiAction';
import styles from './AiSettings.module.css';

export function ConnectionForm({ connection, data, action }: { connection: Connection | null; data: AiSettings; action: AiAction }) {
  const [newReference] = useState(() => crypto.randomUUID());
  const [api, setApi] = useState(connection?.api ?? data.protocol.editableConnectionApis[0] ?? '');
  const [address, setAddress] = useState(connection?.baseUrl ?? '');
  async function save(event: FormEvent<HTMLFormElement>) {
    event.preventDefault(); const form = new FormData(event.currentTarget);
    const payload = { connectionRef: connection?.connectionRef ?? newReference, expectedRevision: connection?.revision ?? 0,
      name: String(form.get('name')), api, baseUrl: address, localEndpoint: form.get('localEndpoint') === 'on', apiKey: String(form.get('apiKey')) };
    await action.run(`connection:${payload.connectionRef}`, payload, versionRef => saveConnection({ ...payload, versionRef }), '连接已保存，未进行测试或创建任务。');
  }
  if (connection?.api === 'local-wemm-runtime') return <div className={styles.group}><h3>内置 WeMM Runtime（只读）</h3><p>此运行时由本机程序管理，使用 local:// 地址，不支持以 HTTP API 表单修改协议、地址或凭据。</p></div>;
  return <form className={styles.form} onSubmit={event => void save(event)}><h3>{connection ? '编辑连接' : '新建连接'}</h3><div className={styles.grid}>
    <Field label="连接名称"><input name="name" maxLength={100} required defaultValue={connection?.name ?? ''} autoComplete="off" /></Field>
    <Field label="协议"><select value={api} onChange={event => setApi(event.target.value)}>{data.protocol.editableConnectionApis.map(api => <option key={api}>{api}</option>)}</select></Field>
    <Field label="服务地址"><input type="url" value={address} required maxLength={2048} onChange={event => setAddress(event.target.value)} autoComplete="off" /></Field>
    <Field label="API 密钥" hint={connection ? '留空沿用本机已保存的密钥。换密钥保留当前地址许可，但当前密钥需要单独测试。' : '只保存在本机密钥存储中。'}><input type="password" name="apiKey" required={!connection} autoComplete="new-password" /></Field>
    <label><input type="checkbox" name="localEndpoint" defaultChecked={connection?.localEndpoint ?? false} /> 本机端点</label></div>
    {connection && (address !== connection.baseUrl || api !== connection.api) ? <div className={styles.impact}><h3>地址或协议将变更</h3><p>新地址批次的能力、价格和外发许可需重新确认；改回旧地址也不会恢复旧许可。当前已冻结的批次仍会重核最新许可。</p></div> : null}
    <SaveBar busy={action.busy !== null}>保存连接</SaveBar>
  </form>;
}
const PERMISSIONS: Record<string, string[]> = { ocr_recognition: ['manual_ocr_image'], model_probe: ['synthetic_probe', 'model_catalog'], note_interpretation: ['note_material'], work_comparison: ['note_material'], creator_research: ['note_material'], creator_account_research: ['note_material', 'public_author_bio'], topic_research: ['note_material', 'comment_material'], topic_research_extract_review: ['note_material', 'comment_material'], topic_research_resolve: ['note_material', 'comment_material'], topic_research_resolve_review: ['note_material', 'comment_material'], comment_study: ['note_material', 'comment_material'], comment_problem_resolution: ['note_material', 'comment_material'], comment_problem_pair_review: ['note_material', 'comment_material'], direction_list_draft: ['note_material'], direction_proposal: ['note_material'], author_org_identity: ['public_author_bio'], writing_assistant: ['selected_writing_material'], writing_legacy: ['selected_writing_material'], creation_title: ['selected_writing_material'] };
export function EgressForm({ data, connection, action }: { data: AiSettings; connection: Connection; action: AiAction }) {
  const [selection, setSelection] = useState('model_probe:synthetic_probe');
  const [taskKey = '', materialClass = ''] = selection.split(':');
  const current = data.egress.find(e => e.endpointRef === connection.endpointRef && e.taskKey === taskKey && e.materialClass === materialClass);
  const [reason, setReason] = useState('');
  const allowed = current?.allowed !== true;
  function change() {
    const payload = { endpointRef: connection.endpointRef, taskKey, materialClass, expectedEventRef: current?.eventRef ?? null, allowed, reason };
    void action.run(`egress:${connection.endpointRef}:${selection}`, payload, commandRef => command('egress', { ...payload, commandRef }), allowed ? '当前地址的该项材料许可已批准，未创建任务。' : '该项材料许可已撤回，后续派发会重新核对。');
  }
  return <div className={styles.group}><h3>当前地址的材料外发许可</h3><Field label="工作与材料"><select value={selection} onChange={event => setSelection(event.target.value)}>{data.tasks.flatMap(t => (PERMISSIONS[t.taskKey] ?? []).map(material => <option key={`${t.taskKey}:${material}`} value={`${t.taskKey}:${material}`}>{t.label} · {MATERIAL[material]}</option>))}</select></Field>
    <Impact title={current?.allowed ? '已批准' : current ? '已撤回' : '尚未批准'} label={allowed ? '批准这项材料外发' : '撤回这项材料外发'} action={change} disabled={action.busy !== null || !reason.trim()}>
      <p>{connection.name} · {connection.baseUrl}</p><p>仅授权所选工作与材料类别。测试也需要单独批准“合成测试内容”；保存与批准不自动发起请求。</p>
      <Field label="外发许可原因"><input value={reason} maxLength={300} onChange={e => setReason(e.target.value)} /></Field>
      {current ? <small>上次原因：{current.reason} · <Moment value={current.createdAt} /></small> : null}
    </Impact>
  </div>;
}
export function ProbeForm({ data, connection, model, action }: { data: AiSettings; connection: Connection; model: AiModel | null; action: AiAction }) {
  const [operation, setOperation] = useState<'connect' | 'discover' | 'probe'>(model ? 'probe' : 'connect');
  const task = data.tasks.find(task => task.taskKey === 'model_probe');
  async function enqueue(event: FormEvent<HTMLFormElement>) {
    event.preventDefault(); if (!task) return;
    const form = new FormData(event.currentTarget);
    try {
      if (operation === 'probe' && (!model?.modelRef || !model.capability)) throw new Error('请先为当前密钥登记模型并保存能力声明。');
      const profile = operation === 'probe' && model?.capability ? { capabilityRef: model.capability.capabilityRef, limits: readLimits(form) } : undefined;
      const payload = { taskKey: task.taskKey, expectedTaskRevision: task.revision, taskConfigRef: task.configuration.taskConfigRef, kind: 'probe', connectionVersionRef: connection.versionRef, operation, modelRef: operation === 'probe' ? model?.modelRef : null, profile };
      await action.run(`probe:${connection.versionRef}:${operation}:${model?.identityRef ?? ''}`, payload, commandRef => {
        const { connectionVersionRef, operation, modelRef, profile, ...batch } = payload;
        return command('batches', { ...batch, commandRef, items: [{ itemKey: 'synthetic-probe', input: { kind: 'probe', sampleRef: commandRef, connectionVersionRef, operation, modelRef, ...(profile ? { profile } : {}) } }] });
      }, receipt => `测试已接纳为批次 ${receipt.batchRef}，尚未完成。请在运行状况查看回执。`);
    } catch (error) { action.setNotice({ tone: 'error', text: error instanceof Error ? error.message : '测试未接纳。' }); }
  }
  return <form className={styles.form} onSubmit={event => void enqueue(event)}><h3>独立测试</h3><Field label="测试类型"><select value={operation} onChange={e => { if (e.target.value === 'connect' || e.target.value === 'discover' || e.target.value === 'probe') setOperation(e.target.value); }}><option value="connect">连接测试</option><option value="discover">读取模型列表</option>{model ? <option value="probe">所选模型能力测试</option> : null}</select></Field>
    {operation === 'probe' && model?.capability ? <LimitsFields key={model.identityRef} limits={null} max={{ ...model.capability, maxAttempts: data.resources.maxAttempts }} levels={model.capability.thinkingLevels} /> : null}
    <div className={styles.impact}><p>只使用合成输入或模型列表请求，可能产生供应商费用；需开启测试任务并批准当前地址的对应许可。此动作提交队列，完成结果另行回执。</p><ActionButton type="submit" variant="primary" disabled={action.busy !== null || !task}>提交本次测试</ActionButton></div>
  </form>;
}
export function AiModelSection({ data, action }: { data: AiSettings; action: AiAction }) {
  const [query] = useSearchParams();
  const connection = query.get('connection') === 'new' ? null : data.connections.find(c => c.connectionRef === query.get('connection')) ?? data.connections[0] ?? null;
  const models = data.models.filter(model => model.connectionRef === connection?.connectionRef);
  const model = models.find(m => m.identityRef === query.get('model')) ?? models[0] ?? null;
  async function addModel(event: FormEvent<HTMLFormElement>) {
    event.preventDefault(); if (!connection) return;
    const payload = { connectionVersionRef: connection.versionRef, modelId: String(new FormData(event.currentTarget).get('modelId')) };
    await action.run(`model:${connection.versionRef}`, payload, modelRef => saveModel({ ...payload, modelRef }), '模型标识已保存，尚未进行能力测试。');
  }
  return <section className={styles.section} aria-labelledby="models-title"><div className={styles.row}><h2 id="models-title">模型</h2><Link to="?section=models&connection=new">新建连接</Link></div>
    <nav className={styles.selector} aria-label="模型连接">{data.connections.map(c => <Link key={c.connectionRef} to={`?section=models&connection=${c.connectionRef}`} aria-current={c.connectionRef === connection?.connectionRef ? 'page' : undefined}><span>{c.name}</span><small>{c.enabled ? '已启用' : '已停用'} · {c.api}</small></Link>)}</nav>
    <ConnectionForm key={connection?.versionRef ?? 'new'} connection={connection} data={data} action={action} />
    {connection && connection.api !== 'local-wemm-runtime' ? <><Impact title={`连接${connection.enabled ? '已启用' : '已停用'}`} label={connection.enabled ? '停用此连接' : '启用此连接'} action={() => { const payload = { connectionRef: connection.connectionRef, expectedRevision: connection.revision, enabled: !connection.enabled }; void action.run(`connection-state:${connection.connectionRef}`, payload, () => setConnectionEnabled(payload), connection.enabled ? '连接已停用。' : '连接已启用，未发起请求。'); }} disabled={action.busy !== null}><p>连接开关与全局、执行器、工作开关分别生效。启用连接不批准材料外发。</p></Impact>
      <form className={styles.form} onSubmit={event => void addModel(event)}><Field label="登记模型标识" hint="按服务商提供的模型 ID 填写，不以名称猜测能力。"><input name="modelId" required maxLength={200} /></Field><ActionButton type="submit" disabled={action.busy !== null}>登记模型</ActionButton></form>
      <nav className={styles.selector} aria-label="已登记模型">{models.map(m => <Link key={m.identityRef} to={`?section=models&connection=${connection.connectionRef}&model=${m.identityRef}`} aria-current={m.identityRef === model?.identityRef ? 'page' : undefined}>{m.modelId}</Link>)}</nav>
      {model ? <><dl className={styles.facts}><dt>稳定模型身份</dt><dd><Reference value={model.identityRef} /></dd><dt>当前密钥测试</dt><dd>{model.probe ? `${model.probe.state} · ${model.probe.modelCallable === true ? '已响应' : model.probe.modelCallable === false ? '调用未通过' : '调用资格未知'}` : '当前密钥尚未测试'}{model.probe?.failureCode ? ` · ${model.probe.failureCode}` : ''}</dd></dl><AiCapabilityForm key={`${model.identityRef}:${model.endpointRef}`} data={data} model={model} connection={connection} action={action} /></> : <p>尚未登记模型。先登记模型标识，再声明能力。</p>}
      <EgressForm key={connection.endpointRef} data={data} connection={connection} action={action} /><ProbeForm key={`${connection.versionRef}:${model?.identityRef ?? ''}`} data={data} connection={connection} model={model} action={action} />
    </> : null}
  </section>;
}
