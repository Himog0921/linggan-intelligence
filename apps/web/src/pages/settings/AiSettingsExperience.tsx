import { useEffect, useState, type FormEvent, type ReactNode } from 'react';
import { createPortal } from 'react-dom';
import { Link, useSearchParams } from 'react-router';
import { useQuery } from '@tanstack/react-query';
import { ActionButton } from '../../shared/ActionButton';
import { humanMoment } from '../../shared/formatting';
import { saveConnection, saveModel, setConnectionEnabled } from './aiConnectionApi';
import { command, fetchHistory } from './aiSettingsApi';
import { AiCapabilityForm } from './AiCapabilityForm';
import { NoticeLine } from './AiSettingsFields';
import { ConnectionForm, EgressForm, ProbeForm } from './AiModelSection';
import { AiTaskSection, ControlAction } from './AiTaskSection';
import { AiSkillSection } from './AiSkillSection';
import { BATCH_STATE, MATERIAL, TASK_STATE, budgetText, count, type AiSettings, type Task } from './aiSettingsModel';
import type { AiAction } from './useAiAction';
import styles from './AiSettingsExperience.module.css';

const ORDER = ['writing_legacy', 'note_interpretation', 'author_org_identity', 'direction_list_draft', 'direction_proposal', 'writing_assistant', 'comment_study', 'legacy_creator_discovery', 'legacy_topic_map'];
const COPY: Record<string, { group: string; description: string }> = {
  writing_legacy: { group: '内容工作台', description: '基于选中的作品材料，提供可审阅的写作建议与修改方案。' },
  note_interpretation: { group: '内容理解', description: '提炼笔记观点、表达方式与值得验证的关键信息。' },
  author_org_identity: { group: '创作者研究', description: '辅助判断公开作者简介与账号定位，结论需要保留来源证据。' },
  direction_list_draft: { group: '主题研究', description: '从已有证据中整理候选研究方向，供人工审查。' },
  direction_proposal: { group: '市场洞察', description: '辅助提出值得验证的新切入点，而非自动立项。' },
  writing_assistant: { group: '内容工作台', description: '为内容工作台提供辅助写作支持。' },
  comment_study: { group: '评论研究', description: '从真实讨论中研究需求、冲突和问题。' },
  comment_embedding: { group: '评论研究', description: '为评论语义召回提供本机向量支持。' },
  legacy_creator_discovery: { group: '历史工作', description: '旧版自动分析已退役；历史数据按现有规则保留。' },
  legacy_topic_map: { group: '历史工作', description: '旧主题研究已退役，仅供查看历史记录。' },
};
const STATES: Record<string, { name: string; tone: string; note: string }> = {
  active: { name: '已接入', tone: 'live', note: '仍需核对运行条件' },
  pending: { name: '待接入', tone: 'pending', note: '尚不能执行' },
  frozen: { name: '已冻结', tone: 'frozen', note: '暂不允许新任务' },
  retired: { name: '已退役', tone: 'history', note: '仅保留历史' },
  legacy: { name: '保留历史', tone: 'history', note: '仅保留历史' },
};
const MATERIAL_BY_TASK: Record<string, string> = {
  note_interpretation: 'note_material', direction_list_draft: 'note_material', direction_proposal: 'note_material',
  author_org_identity: 'public_author_bio', writing_assistant: 'selected_writing_material',
  writing_legacy: 'selected_writing_material', model_probe: 'synthetic_probe',
};
const fmt = (value: string | number | null | undefined) => value == null ? '—' : count(value);
function Status({ stage }: { stage: string }) {
  const status = STATES[stage] ?? { name: '状态未知', tone: 'pending', note: '等待核对' };
  return <span className={styles.status} data-tone={status.tone}><i aria-hidden="true" />{status.name}</span>;
}
function Glyph({ name }: { name: 'arrow' | 'close' | 'search' | 'grid' | 'list' | 'pulse' | 'refresh' | 'shield' }) {
  const path = {
    arrow: <><path d="M4 12h15m-6-6 6 6-6 6"/></>,
    close: <><path d="m5 5 14 14M19 5 5 19"/></>,
    search: <><circle cx="10.5" cy="10.5" r="6.5"/><path d="m16 16 5 5"/></>,
    grid: <><rect x="3" y="3" width="7" height="7"/><rect x="14" y="3" width="7" height="7"/><rect x="3" y="14" width="7" height="7"/><rect x="14" y="14" width="7" height="7"/></>,
    list: <><path d="M8 6h13M8 12h13M8 18h13M3 6h1M3 12h1M3 18h1"/></>,
    pulse: <><path d="M2 12h5l3-7 4 14 3-7h5"/></>,
    refresh: <><path d="M20 7v5h-5M4 17v-5h5"/><path d="M5 9A8 8 0 0 1 19 7m0 8A8 8 0 0 1 5 17"/></>,
    shield: <><path d="M12 22s8-4 8-10V5l-8-3-8 3v7c0 6 8 10 8 10Z"/><path d="m9 12 2 2 4-4"/></>,
  }[name];
  return <svg aria-hidden="true" width="16" height="16" viewBox="0 0 24 24" stroke="currentColor" fill="none" strokeWidth="1.6" strokeLinecap="square" strokeLinejoin="miter">{path}</svg>;
}
function Drawer({ title, onClose, children }: { title: string; onClose: () => void; children: ReactNode }) {
  useEffect(() => {
    function escape(event: KeyboardEvent) { if (event.key === 'Escape') onClose(); }
    window.addEventListener('keydown', escape);
    return () => window.removeEventListener('keydown', escape);
  }, [onClose]);
  return createPortal(<div className={styles.layer} role="presentation" onMouseDown={event => { if (event.target === event.currentTarget) onClose(); }}>
    <section role="dialog" aria-modal="true" aria-label={title} className={styles.drawer}>
      <header className={styles.drawerHeader}><div><small>INTELLIGENCE / INSPECTOR</small><h2>{title}</h2></div><button type="button" className={styles.iconButton} aria-label="关闭详情" onClick={onClose}><Glyph name="close"/></button></header>
      <div className={styles.drawerBody}>{children}</div>
    </section>
  </div>, document.body);
}
function Modal({ title, onClose, children }: { title: string; onClose: () => void; children: ReactNode }) {
  useEffect(() => {
    function escape(event: KeyboardEvent) { if (event.key === 'Escape') onClose(); }
    window.addEventListener('keydown', escape);
    return () => window.removeEventListener('keydown', escape);
  }, [onClose]);
  return createPortal(<div className={styles.modalLayer} role="presentation" onMouseDown={event => { if (event.currentTarget === event.target) onClose(); }}>
    <section role="dialog" aria-modal="true" aria-label={title} className={styles.modal}>
      <header className={styles.drawerHeader}><h2>{title}</h2><button type="button" aria-label="关闭弹窗" className={styles.iconButton} onClick={onClose}><Glyph name="close"/></button></header>
      <div className={styles.modalBody}>{children}</div>
    </section>
  </div>, document.body);
}
function groupTasks(data: AiSettings) {
  return data.tasks.filter(task => task.taskKey !== 'model_probe' && task.taskKey !== 'comment_embedding' && task.taskKey !== 'legacy_unclassified')
    .sort((a, b) => {
      const ai = ORDER.indexOf(a.taskKey), bi = ORDER.indexOf(b.taskKey);
      return (ai < 0 ? 999 : ai) - (bi < 0 ? 999 : bi);
    });
}
type Check = { title: string; okay: boolean | null; detail: string };
function checksFor(data: AiSettings, task: Task): Check[] {
  const configuration = task.configuration;
  const model = data.models.find(entry => entry.identityRef === configuration.model?.identityRef);
  const connection = data.connections.find(entry => entry.connectionRef === model?.connectionRef);
  const requiredControls = data.controls.filter(entry => entry.scope === 'global' || entry.scope === 'executor' || entry.taskKey === task.taskKey);
  const requiredScopes = ['global', 'executor', 'task:' + task.taskKey];
  const controlsComplete = requiredScopes.every(scope => requiredControls.some(control => control.scope === scope));
  const material = MATERIAL_BY_TASK[task.taskKey];
  const license = material && model ? data.egress.find(eg => eg.endpointRef === model.endpointRef && eg.taskKey === task.taskKey && eg.materialClass === material) : null;
  const hasModel = Boolean(configuration.model && model?.capability && model?.selectionModelRef && connection?.enabled && model.probe?.state === 'succeeded' && model.probe.modelCallable === true);
  return [
    { title: '业务执行接入', okay: task.lifecycle === 'active' && Boolean(task.handlerContract) && task.handlerContract === configuration.executionContract, detail: task.lifecycle === 'active' ? '执行处理器与配置版本需匹配' : TASK_STATE[task.lifecycle] ?? '接入状态未知' },
    { title: '全局、执行器和工作开关', okay: controlsComplete ? requiredControls.every(control => control.enabled) : null, detail: controlsComplete ? requiredControls.filter(control => !control.enabled).map(control => control.scope === 'global' ? '全局已关闭' : control.scope === 'executor' ? '执行器已关闭' : '工作已暂停').join('、') || '三个开关已启用' : '开关记录尚未齐全' },
    { title: '当前模型与调用资格', okay: hasModel, detail: configuration.model ? (model?.probe?.modelCallable === true && model.probe.state === 'succeeded' ? '模型已响应，仍由服务端复核' : '当前模型的有效调用测试尚未通过') : '未分配已验证的模型' },
    { title: '每日额度', okay: configuration.budget.mode !== 'unconfigured', detail: budgetText(configuration.budget) },
    { title: '固定做法版本', okay: !task.requiresSkill || (Boolean(configuration.skillVersionRef) && configuration.skillVersionRef === task.activeSkillVersionRef), detail: task.requiresSkill ? (task.activeSkillVersionRef ? '需与当前已启用做法一致' : '尚未启用必须的做法') : '该工作不强制绑定做法' },
    { title: '当前地址的材料许可', okay: material ? (model ? license?.allowed === true : null) : null, detail: material ? (license?.allowed ? (MATERIAL[material] ?? material) + '已批准' : '需核对端点、工作与材料许可') : '当前业务的许可类别需要服务端确认' },
  ];
}
function CapabilityDrawer({ data, task, onClose }: { data: AiSettings; task: Task; onClose: () => void }) {
  const checks = checksFor(data, task);
  const failed = checks.filter(c => c.okay !== true).length;
  return <Drawer title={task.label} onClose={onClose}>
    <div className={styles.drawerIntro}><Status stage={task.lifecycle}/><p>{COPY[task.taskKey]?.description ?? '当前业务工作，具体能力以实际接入合同为准。'}</p></div>
    <div className={styles.drawerSummary}><b>{task.lifecycle !== 'active' ? '目前不能执行新工作' : failed ? '尚未满足全部已知运行条件' : '已知配置通过，执行仍需服务端复核'}</b><small>{failed} 项尚未确认或通过 · 前端诊断不代替最终运行准入</small></div>
    <h3 className={styles.blockTitle}>运行条件</h3><div className={styles.checkList}>{checks.map(check => <div className={styles.checkRow} key={check.title}><span className={styles.checkMark} data-ok={String(check.okay)}>{check.okay === true ? '✓' : check.okay === false ? '!' : '?'}</span><div><b>{check.title}</b><small>{check.detail}</small></div></div>)}</div>
    <h3 className={styles.blockTitle}>现有配置</h3><dl className={styles.definition}><dt>模型</dt><dd>{task.configuration.model?.modelId ?? '未配置'}</dd><dt>做法版本</dt><dd>{task.configuration.skillVersionRef ? '已选择固定版本' : '未绑定'}</dd><dt>每日额度</dt><dd>{budgetText(task.configuration.budget)}</dd><dt>工作开关</dt><dd>{data.controls.find(c => c.taskKey === task.taskKey)?.enabled ? '已启用' : '未启用'}</dd></dl>
    <div className={styles.drawerFooter}><Link className={styles.actionLink} to={'?view=advanced&area=work&task=' + encodeURIComponent(task.taskKey)} onClick={onClose}>查看工作配置 <Glyph name="arrow"/></Link><Link className={styles.subtleLink} to={'?view=runs&task=' + encodeURIComponent(task.taskKey)} onClick={onClose}>查看调用记录</Link></div>
  </Drawer>;
}
export function AiCapabilities({ data }: { data: AiSettings }) {
  const [filter, setFilter] = useState('all');
  const [search, setSearch] = useState('');
  const [layout, setLayout] = useState<'grid' | 'list'>('grid');
  const [detail, setDetail] = useState<string | null>(null);
  const [diagnostics, setDiagnostics] = useState(false);
  const tasks = groupTasks(data);
  const visible = tasks.filter(task => (filter === 'all' || (filter === 'history' ? ['retired', 'legacy'].includes(task.lifecycle) : task.lifecycle === filter))
    && (task.label + ' ' + (COPY[task.taskKey]?.description ?? '') + ' ' + (COPY[task.taskKey]?.group ?? '')).toLowerCase().includes(search.toLowerCase().trim()));
  const service = data.execution.service;
  const observed = service.observedState;
  const online = ['running', 'ready', 'active'].includes(observed);
  const control = data.controls.find(c => c.scope === 'global');
  const runner = data.controls.find(c => c.scope === 'executor');
  const selected = tasks.find(task => task.taskKey === detail);
  return <div className={styles.experience}>
    <div className={styles.signalBar}><div className={styles.signalLead}><div className={styles.signalIcon}><Glyph name="pulse"/></div><div><b>{online ? '执行器已报告运行状态' : observed === 'not_started' ? '执行器尚未启动' : '执行器状态需核对'}</b><small>{online ? '执行器心跳可见，但具体工作仍需满足完整准入条件。' : '当前读取未证明执行器正在运行。已接入工作不等于现在可执行。'}</small></div></div><div className={styles.signalMeta}><span>GLOBAL SWITCH <b>{control ? control.enabled ? '已启用' : '已关闭' : '未知'}</b></span><span>RUNNER <b>{runner?.enabled ? '开关已启用' : '未启用'}</b></span><button type="button" onClick={() => setDiagnostics(true)}>查看受阻原因 <Glyph name="arrow"/></button></div></div>
    <div className={styles.statStrip}>{[
      ['已接入工作', tasks.filter(t => t.lifecycle === 'active').length, '仍需核对运行资格'],
      ['尚待接入', tasks.filter(t => t.lifecycle === 'pending').length, '技术执行尚未连接'],
      ['已冻结', tasks.filter(t => t.lifecycle === 'frozen').length, '暂不允许新任务'],
      ['历史工作', tasks.filter(t => ['retired', 'legacy'].includes(t.lifecycle)).length, '只提供历史查询'],
    ].map(([label, value, subtitle], i) => <div className={styles.stat} key={String(label)}><strong className={i === 0 ? styles.accent : undefined}>{String(value).padStart(2, '0')}</strong><div><b>{label}</b><small>{subtitle}</small></div></div>)}</div>
    <div className={styles.sectionHeading}><div><h2>工作能力</h2><small>{visible.length} 项匹配 · 选择工作可查看配置与受阻原因</small></div><div className={styles.toolbar}>
      <label className={styles.search}><Glyph name="search"/><input type="search" value={search} onChange={event => setSearch(event.target.value)} placeholder="搜索 AI 工作" aria-label="搜索 AI 工作"/></label>
      <div className={styles.segments} aria-label="筛选工作">{([['all','全部'],['active','已接入'],['pending','待接入'],['frozen','冻结'],['history','历史']] as const).map(([key, title]) => <button key={key} type="button" data-active={filter === key} onClick={() => setFilter(key)}>{title}</button>)}</div>
      <div className={styles.viewPick}><button type="button" aria-label="卡片视图" aria-pressed={layout === 'grid'} onClick={() => setLayout('grid')}><Glyph name="grid"/></button><button type="button" aria-label="列表视图" aria-pressed={layout === 'list'} onClick={() => setLayout('list')}><Glyph name="list"/></button></div>
    </div></div>
    <div className={styles.capGrid} data-layout={layout}>{visible.map((task, index) => <button type="button" key={task.taskKey} className={styles.capCard} data-stage={task.lifecycle} onClick={() => setDetail(task.taskKey)}><span className={styles.capTop}><small>CAPABILITY / {String(index + 1).padStart(2, '0')}</small><Status stage={task.lifecycle}/></span><span className={styles.capMiddle}><b>{task.label}</b><small>{COPY[task.taskKey]?.description ?? '查看当前业务能力与运行配置。'}</small></span><span className={styles.capBottom}><small>{COPY[task.taskKey]?.group ?? '其他工作'} · {STATES[task.lifecycle]?.note ?? '当前状态待核对'}</small><b>查看详情 <Glyph name="arrow"/></b></span></button>)}
      {!visible.length ? <div className={styles.empty}>没有符合条件的工作，试着调整搜索或筛选。</div> : null}</div>
    <p className={styles.footnote}>业务工作与基础模型测试分开管理，连接状态和运行状态均来自当前数据库回执。</p>
    {selected ? <CapabilityDrawer task={selected} data={data} onClose={() => setDetail(null)}/> : null}
    {diagnostics ? <Drawer title="系统运行诊断" onClose={() => setDiagnostics(false)}><div className={styles.checkList}>{[
      ['独立执行器', online, observed === 'not_started' ? '数据库暂无执行器实例回执' : '当前状态：' + observed],
      ['全局 AI 开关', control?.enabled ?? null, control ? '当前开关回执：' + (control.enabled ? '已启用' : '已关闭') : '开关回执缺失'],
      ['执行器开关', runner?.enabled ?? null, runner ? '当前开关回执：' + (runner.enabled ? '已启用' : '已关闭') : '开关回执缺失'],
    ].map(([label, status, desc]) => <div className={styles.checkRow} key={String(label)}><span className={styles.checkMark} data-ok={String(status)}>{status ? '✓' : '!'}</span><div><b>{label}</b><small>{desc}</small></div></div>)}</div><p className={styles.callout}>启用开关不会自动创建任务。每项工作还需通过服务端的连接、模型、额度、材料及做法版本检查。</p><Link className={styles.actionLink} to="?view=advanced" onClick={() => setDiagnostics(false)}>前往高级配置 <Glyph name="arrow"/></Link></Drawer> : null}
  </div>;
}
function ConnectionWizard({ data, action, onClose, onSaved }: { data: AiSettings; action: AiAction; onClose: () => void; onSaved: (id: string) => void }) {
  const [step, setStep] = useState(1);
  const [kind, setKind] = useState<'external' | 'localhost'>('external');
  const [name, setName] = useState('');
  const [api, setApi] = useState(data.protocol.editableConnectionApis[0] ?? 'openai-completions');
  const [baseUrl, setBaseUrl] = useState('');
  const [apiKey, setApiKey] = useState('');
  const [modelId, setModelId] = useState('');
  const [saved, setSaved] = useState<{ connectionRef: string; versionRef: string } | null>(null);
  const [connectionRef] = useState(() => crypto.randomUUID());
  async function save(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (kind === 'external' && !baseUrl.startsWith('https://')) { action.setNotice({ tone: 'error', text: '外部模型地址必须使用 HTTPS。' }); return; }
    if (kind === 'localhost' && !/^http:\/\/(localhost|127\.0\.0\.1|\[::1\])(?::\d+)?(\/|$)/.test(baseUrl)) { action.setNotice({ tone: 'error', text: '本机服务只支持 localhost、127.0.0.1 或 [::1] 的 HTTP 地址。' }); return; }
    const payload = { connectionRef, expectedRevision: 0, name: name.trim(), api, baseUrl: baseUrl.trim(), apiKey, localEndpoint: kind === 'localhost' };
    const result = await action.run('new-connection:' + connectionRef, payload, versionRef => saveConnection({ ...payload, versionRef }), '连接已保存，尚未测试，也不会自动发送材料。');
    if (result) { setSaved({ connectionRef: result.connectionRef, versionRef: result.versionRef }); setStep(3); }
  }
  async function register(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (!saved) return;
    const result = await action.run('register-model:' + saved.versionRef, { connectionVersionRef: saved.versionRef, modelId }, modelRef => saveModel({ connectionVersionRef: saved.versionRef, modelId: modelId.trim(), modelRef }), '模型已登记。请继续声明能力并提交独立测试。');
    if (result) onSaved(saved.connectionRef);
  }
  return <Modal title="接入模型" onClose={onClose}><NoticeLine notice={action.notice}/><div className={styles.wizardSteps}><span data-current={step === 1}>01 选择来源</span><span data-current={step === 2}>02 保存连接</span><span data-current={step === 3}>03 登记模型</span></div>
    {step === 1 ? <div className={styles.wizardOptions}><button type="button" onClick={() => { setKind('external'); setStep(2); }}>外部语言模型服务<small>例如 DeepSeek、兼容 OpenAI/Anthropic 的服务，需使用 HTTPS 和服务商密钥</small></button><button type="button" onClick={() => { setKind('localhost'); setStep(2); }}>本机 HTTP 模型服务<small>连接已经在本机启动的兼容 API，不能用于内置 WeMM 运行时</small></button><p>内置 WeMM 已由系统注册，使用独立本机运行时，不需要在这里重复创建连接。</p></div> : null}
    {step === 2 ? <form className={styles.form} onSubmit={event => void save(event)}><label>连接名称<input required maxLength={100} value={name} onChange={event => setName(event.target.value)} placeholder="例如 DeepSeek"/></label><label>协议<select value={api} onChange={event => setApi(event.target.value)}>{data.protocol.editableConnectionApis.map(value => <option key={value} value={value}>{value}</option>)}</select></label><label>服务地址<input required type="url" value={baseUrl} onChange={event => setBaseUrl(event.target.value)} placeholder={kind === 'external' ? 'https://…' : 'http://127.0.0.1:…'}/></label><label>API 密钥<input type="password" autoComplete="new-password" required={kind === 'external'} value={apiKey} onChange={event => setApiKey(event.target.value)} placeholder={kind === 'localhost' ? '本机可留空' : '仅保存在本机密钥存储'}/></label><p className={styles.callout}>保存不会测试、批准材料许可或创建调用批次。连接地址或协议变化后必须重新确认当前地址的许可。</p><div className={styles.formFooter}><ActionButton onClick={() => setStep(1)}>上一步</ActionButton><ActionButton type="submit" variant="primary" busy={action.busy !== null}>保存连接并继续</ActionButton></div></form> : null}
    {step === 3 ? <form className={styles.form} onSubmit={event => void register(event)}><p>连接已保存，仍需登记该服务商提供的真实模型 ID，随后声明能力并提交测试。</p><label>模型 ID<input required maxLength={200} value={modelId} onChange={event => setModelId(event.target.value)} placeholder="按服务商提供的模型 ID 填写"/></label><div className={styles.formFooter}><ActionButton onClick={() => saved && onSaved(saved.connectionRef)}>暂不登记，查看连接</ActionButton><ActionButton type="submit" variant="primary" busy={action.busy !== null}>登记模型</ActionButton></div></form> : null}
  </Modal>;
}
export function AiConnections({ data, action }: { data: AiSettings; action: AiAction }) {
  const [query, setQuery] = useSearchParams();
  const [tab, setTab] = useState<'overview' | 'technical' | 'permission'>('overview');
  const [wizard, setWizard] = useState(false);
  const [selectedModel, setSelectedModel] = useState<string | null>(null);
  const connection = data.connections.find(c => c.connectionRef === query.get('connection')) ?? data.connections.find(c => c.api !== 'local-wemm-runtime') ?? data.connections[0] ?? null;
  const local = connection?.api === 'local-wemm-runtime';
  const models = data.models.filter(m => m.connectionRef === connection?.connectionRef);
  const model = models.find(m => m.identityRef === selectedModel) ?? models[0] ?? null;
  const [confirm, setConfirm] = useState(false);
  const sorted = [...data.connections].sort((a, b) => Number(a.api === 'local-wemm-runtime') - Number(b.api === 'local-wemm-runtime'));
  const capability = model?.capability;
  const testOk = model?.probe?.state === 'succeeded' && model.probe.modelCallable === true;
  function chooseConnection(ref: string) { setQuery({ view: 'models', connection: ref }); setTab('overview'); setSelectedModel(null); }
  async function enableConnection() {
    if (!connection || local) return;
    const payload = { connectionRef: connection.connectionRef, expectedRevision: connection.revision, enabled: !connection.enabled };
    const result = await action.run('connection-state:' + connection.connectionRef, payload, () => setConnectionEnabled(payload), payload.enabled ? '连接开关已启用；未开始测试或运行。' : '连接已停用；未修改历史回执。');
    if (result) setConfirm(false);
  }
  async function addModel(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (!connection) return;
    const input = String(new FormData(event.currentTarget).get('modelId') ?? '').trim();
    const result = await action.run('model:' + connection.versionRef, { connectionVersionRef: connection.versionRef, modelId: input }, modelRef => saveModel({ connectionVersionRef: connection.versionRef, modelId: input, modelRef }), '模型已登记，还需声明能力并独立测试。');
    if (result) event.currentTarget.reset();
  }
  return <div className={styles.experience}><div className={styles.sectionHeading}><div><h2>模型连接</h2><small>语言模型与本机向量运行时独立管理</small></div><ActionButton variant="primary" onClick={() => setWizard(true)}>接入模型</ActionButton></div>
    <div className={styles.modelGrid}><aside className={styles.modelSidebar}><div className={styles.subhead}>连接列表 <small>{sorted.length}</small></div><small className={styles.sourceLabel}>模型 API</small>
      {sorted.filter(c => c.api !== 'local-wemm-runtime').map(c => <button type="button" className={styles.modelItem} data-current={connection?.connectionRef === c.connectionRef} onClick={() => chooseConnection(c.connectionRef)} key={c.connectionRef}><span className={styles.avatar}>{c.name.slice(0, 2).toUpperCase()}</span><span><b>{c.name}</b><small>{c.api}</small></span><Glyph name="arrow"/></button>)}
      <small className={styles.sourceLabel}>本机运行时</small>
      {sorted.filter(c => c.api === 'local-wemm-runtime').map(c => <button type="button" className={styles.modelItem} data-current={connection?.connectionRef === c.connectionRef} onClick={() => chooseConnection(c.connectionRef)} key={c.connectionRef}><span className={styles.avatar} data-local>WM</span><span><b>{c.name}</b><small>内置 WeMM 向量模型</small></span><Glyph name="arrow"/></button>)}
      <p className={styles.sidebarNote}>本机 WeMM 不使用通用 API 密钥，也不参与语言模型能力声明。</p>
    </aside><section className={styles.modelMain}>{connection ? <><div className={styles.modelHeader}><div className={styles.modelTitle}><span className={styles.avatar} data-local={local || undefined}>{local ? 'WM' : connection.name.slice(0, 2).toUpperCase()}</span><div><h2>{connection.name}</h2><small>{model?.modelId ?? '尚未登记模型'}</small></div></div><div className={styles.modelActions}>{!local ? <><ActionButton onClick={() => setTab('technical')}>编辑配置</ActionButton><ActionButton onClick={() => setConfirm(true)}>{connection.enabled ? '停用连接' : '启用连接'}</ActionButton></> : null}</div></div>
      <div className={styles.modelBadges}><span>{local ? '本机向量运行时' : '模型 API'}</span><span>{connection.enabled ? '连接已启用' : '连接已停用'}</span><span>{local ? '运行资格未由本页确认' : testOk ? '当前模型已响应' : '当前资格待核对'}</span></div>
      <nav className={styles.modelTabs} aria-label="连接详情导航">{[['overview','连接概览'],['technical','技术配置'],['permission','数据权限']].map(([key, label]) => <button type="button" key={key} aria-current={tab === key ? 'page' : undefined} onClick={() => setTab(key as typeof tab)}>{label}</button>)}</nav>
      <div className={styles.modelBody}>{tab === 'overview' ? <><div className={styles.kvGrid}><div><small>模型来源</small><b>{local ? '内置 / 本机 Runtime' : '外部或本机 HTTP 端点'}</b></div><div><small>当前协议</small><b>{connection.api}</b></div><div><small>服务地址</small><b className={styles.breakable}>{connection.baseUrl}</b></div><div><small>运行状态</small><b>{local ? '尚无专用运行时状态回执' : testOk ? '当前密钥能力测试通过' : '需要验证'}</b></div></div>
        {models.length > 1 ? <div className={styles.picker}><small>选择模型</small>{models.map(entry => <button type="button" aria-pressed={model?.identityRef === entry.identityRef} onClick={() => setSelectedModel(entry.identityRef)} key={entry.identityRef}>{entry.modelId}</button>)}</div> : null}
        <h3 className={styles.blockTitle}>可用性检查</h3><div className={styles.checkList}>{[
          ['连接开关', connection.enabled, connection.enabled ? '已启用，不代表自动授权数据外发' : '连接已停用'],
          ['已登记模型', Boolean(model), model?.modelId ?? '请先登记模型标识'],
          ['模型能力声明', local ? null : Boolean(capability), local ? '本机运行时使用独立能力合同' : capability ? '能力参数已保存，不等于完成验证' : '尚未登记上下文和输出能力'],
          ['当前实际测试', local ? null : testOk, local ? '应由独立 WeMM 运行时检测确认' : testOk ? '当前模型已响应' : '当前连接版本缺少成功测试'],
        ].map(([label, ok, desc]) => <div className={styles.checkRow} key={String(label)}><span className={styles.checkMark} data-ok={String(ok)}>{ok === true ? '✓' : ok === false ? '!' : '?'}</span><div><b>{label}</b><small>{desc}</small></div></div>)}</div>
        {!local ? <><h3 className={styles.blockTitle}>独立验证</h3><ProbeForm key={connection.versionRef + ':' + (model?.identityRef ?? '')} data={data} connection={connection} model={model} action={action}/></> : <p className={styles.callout}>WeMM 是独立 Python 模型进程。当前通用 AI 设置 API 没有直接暴露其实时握手状态；这里不会把“已启用”伪称为“模型已运行”。</p>}
      </> : tab === 'technical' ? local ? <p className={styles.callout}>内置 WeMM 连接由本机运行时和数据库迁移登记，使用 local:// 地址。这里只读展示，不允许通过普通 HTTP 模型表单修改其协议、地址或密钥。</p> : <div className={styles.legacyForm}><ConnectionForm key={connection.versionRef} connection={connection} data={data} action={action}/><form className={styles.form} onSubmit={event => void addModel(event)}><label>登记模型标识<input name="modelId" required maxLength={200} placeholder="服务商实际模型 ID"/></label><ActionButton type="submit" busy={action.busy !== null}>登记模型</ActionButton></form>{model ? <AiCapabilityForm key={model.identityRef + ':' + model.endpointRef} data={data} model={model} connection={connection} action={action}/> : null}</div> : local ? <p className={styles.callout}>本机 WeMM 不需要授权向外部地址发送材料。评论向量工作是否接入，仍由独立工作状态决定。</p> : <div className={styles.legacyForm}><EgressForm data={data} connection={connection} action={action}/></div>}</div></> : <div className={styles.empty}>当前还没有连接，使用“接入模型”添加第一个。</div>}</section></div>
    <p className={styles.footnote}>连接已保存 ≠ 模型测试通过 ≠ 材料许可批准 ≠ 可发起业务执行。所有按钮使用真实保存和执行回执。</p>
    {confirm && connection ? <Modal title={connection.enabled ? '停用模型连接' : '启用模型连接'} onClose={() => setConfirm(false)}><p>此操作只修改连接开关，不会恢复或批准原有的外发许可，也不会发起模型调用。</p><div className={styles.formFooter}><ActionButton onClick={() => setConfirm(false)}>取消</ActionButton><ActionButton variant="primary" busy={action.busy !== null} onClick={() => void enableConnection()}>确认{connection.enabled ? '停用' : '启用'}</ActionButton></div></Modal> : null}
    {wizard ? <ConnectionWizard data={data} action={action} onClose={() => setWizard(false)} onSaved={ref => { setWizard(false); chooseConnection(ref); setTab('technical'); }}/> : null}
  </div>;
}
type HistoryPage = Awaited<ReturnType<typeof fetchHistory>>;
type HistoryItem = HistoryPage['items'][number];
function moneyKnown(v: string | null) {
  if (v === null) return '未知';
  const pico = BigInt(v);
  const major = pico / 1_000_000_000_000n;
  const sub = (pico % 1_000_000_000_000n).toString().padStart(12, '0').slice(0, 6);
  return '$' + major.toString() + '.' + sub;
}
function useHistory(taskKey: string) {
  const [cursors, setCursors] = useState(['']);
  useEffect(() => setCursors(['']), [taskKey]);
  const before = cursors[cursors.length - 1] ?? '';
  const history = useQuery({ queryKey: ['ai-invocation-history', taskKey, before], queryFn: ({ signal }) => fetchHistory(taskKey, before, signal), retry: false });
  return { history, cursors, setCursors };
}
export function AiRuns({ data }: { data: AiSettings }) {
  const [query, setQuery] = useSearchParams();
  const taskKey = query.get('task') ?? '';
  const [filter, setFilter] = useState('all');
  const [selected, setSelected] = useState<HistoryItem | null>(null);
  const { history, cursors, setCursors } = useHistory(taskKey);
  const rows = data.execution.dailyUsage.filter(row => !taskKey || row.taskKey === taskKey);
  const add = (key: 'calls' | 'failedCalls' | 'unknownCalls' | 'measuredTokens') => rows.reduce<bigint | null>((sum, row) => sum === null || row[key] === null ? null : sum + BigInt(row[key] as string), 0n);
  const calls = add('calls'), failures = add('failedCalls'), unknown = add('unknownCalls'), measured = add('measuredTokens');
  const byDay = new Map<string, { known: bigint; unknown: boolean }>();
  for (const row of rows) {
    const day = byDay.get(row.budgetDay) ?? { known: 0n, unknown: false };
    if (row.measuredTokens === null) day.unknown = true;
    else day.known += BigInt(row.measuredTokens);
    byDay.set(row.budgetDay, day);
  }
  const bars = Array.from(byDay.entries()).sort(([a], [b]) => a.localeCompare(b)).slice(-7);
  const max = bars.reduce((value, [, entry]) => entry.known > value ? entry.known : value, 1n);
  const items = (history.data?.items ?? []).filter(item => filter === 'all' || (filter === 'succeeded' ? item.state === 'succeeded' : item.state === 'failed'));
  return <div className={styles.experience}><div className={styles.sectionHeading}><div><h2>运行与用量</h2><small>真实数据库回执 · 最近七日用量与分页调用记录分开统计</small></div><label className={styles.runFilter}>按工作筛选<select value={taskKey} onChange={event => setQuery({ view: 'runs', ...(event.target.value ? { task: event.target.value } : {}) })}><option value="">全部工作</option>{data.tasks.map(t => <option key={t.taskKey} value={t.taskKey}>{t.label}</option>)}</select></label></div>
    <div className={styles.runStats}>{[
      ['调用数', calls === null ? '—' : count(calls.toString()), '最近七日'],
      ['失败数', failures === null ? '—' : count(failures.toString()), '按运行账记录'],
      ['实测 token', measured === null ? '含未知值' : count(measured.toString()), '不包含预留额度'],
      ['未知用量调用', unknown === null ? '—' : count(unknown.toString()), '不能当作零消耗'],
    ].map(([label, value, note]) => <div className={styles.metric} key={label}><small>{label}</small><b>{value}</b><small>{note}</small></div>)}</div>
    <div className={styles.runPanels}><section className={styles.panel}><div className={styles.panelHeading}><h3>有记录日期的实测用量</h3><small>最近七日范围 · 未知不补零</small></div>{bars.length ? <div className={styles.chart} role="img" aria-label="最近记录日期的已知实测 token 用量柱状图">{bars.map(([date, val]) => <div key={date} className={styles.chartColumn}><span title={date + ' · ' + (val.unknown ? '部分未知' : count(val.known.toString()))}><i style={{ height: (Math.max(2, Number(val.known * 100n / max))) + '%' }} /></span><small>{date.slice(5)}</small></div>)}</div> : <div className={styles.empty}>当前筛选范围没有用量记录，不能据此推断历史无消耗。</div>}<div className={styles.chartLegend}><i/> 已记录的实测 token {bars.some(([, val]) => val.unknown) ? '· 部分日期包含未知量' : ''}</div></section>
      <section className={styles.panel}><div className={styles.panelHeading}><h3>运行风险</h3></div><div className={styles.riskNumber}>{failures === null ? '—' : count(failures.toString())}</div><b>最近七日已记录失败调用</b><small className={styles.riskSubtitle}>这不是内容产出的质量得分。需要结合实际输出检查。</small><div className={styles.riskDetail}>{data.execution.failures.length ? data.execution.failures.slice(0, 2).map(f => <p key={f.invocationRef}>{f.description ?? f.failureCode ?? '失败原因尚未归类'}</p>) : <p>未提供失败详情，不代表执行一定成功。</p>}</div><button type="button" className={styles.linkButton} onClick={() => setFilter('failed')}>查看本页失败调用 <Glyph name="arrow"/></button></section>
    </div>
    <section className={styles.panel}><div className={styles.tableHeader}><div><h3>调用记录</h3><small>每页最多 50 条 · 先筛选工作再翻页</small></div><div className={styles.segments}>{([['all','全部'],['succeeded','成功'],['failed','失败']] as const).map(([key, label]) => <button key={key} type="button" data-active={filter === key} onClick={() => setFilter(key)}>{label}</button>)}</div></div>
      <div className={styles.tableScroll}><table className={styles.dataTable}><thead><tr><th>时间</th><th>业务工作</th><th>模型</th><th>运行状态</th><th>实测 token</th><th>额度占用 token</th><th>详情</th></tr></thead><tbody>{items.map(item => <tr key={item.invocationRef}><td>{humanMoment(item.createdAt)}</td><td>{data.tasks.find(t => t.taskKey === item.taskKey)?.label ?? item.taskKey}</td><td className={styles.mono}>{item.modelId ?? '未指定'}</td><td><span className={styles.tableStatus} data-fail={item.state === 'failed'}>{BATCH_STATE[item.state] ?? item.state}</span></td><td>{fmt(item.measuredTokens)}</td><td>{fmt(item.quotaHeldTokens)}</td><td><button type="button" className={styles.linkButton} onClick={() => setSelected(item)}>查看 <Glyph name="arrow"/></button></td></tr>)}
      {!items.length ? <tr><td colSpan={7}>{history.isPending ? '正在读取调用记录…' : history.isError ? '调用记录读取失败，不能视为空记录。' : '本页没有符合条件的调用。'}</td></tr> : null}</tbody></table></div><div className={styles.tableFooter}><small>未知用量与已知零值分开显示；成功率不代表内容质量。</small><div><ActionButton disabled={cursors.length <= 1 || history.isFetching} onClick={() => setCursors(v => v.slice(0, -1))}>较新一页</ActionButton><ActionButton disabled={history.isFetching} onClick={() => void history.refetch()}><Glyph name="refresh"/>刷新</ActionButton><ActionButton disabled={!history.data?.hasMore || history.isFetching} onClick={() => { const next = history.data?.nextCursor; if (next) setCursors(v => [...v, next]); }}>更早一页</ActionButton></div></div>
    </section>
    <p className={styles.footnote}>聚合数据来自用量回执，调用表按实际游标分页。显示的过滤结果只覆盖当前已读取的一页；不会用样例记录填充生产图表。</p>
    {selected ? <Drawer title="调用与版本详情" onClose={() => setSelected(null)}><dl className={styles.definition}><dt>工作</dt><dd>{data.tasks.find(t => t.taskKey === selected.taskKey)?.label ?? selected.taskKey}</dd><dt>时间</dt><dd>{humanMoment(selected.createdAt)}</dd><dt>模型</dt><dd>{selected.connectionName} / {selected.modelId ?? '未知'}</dd><dt>状态</dt><dd>{selected.state}</dd><dt>输入 token</dt><dd>{fmt(selected.inputTokens)}</dd><dt>输出 token</dt><dd>{fmt(selected.outputTokens)}</dd><dt>实测 token</dt><dd>{fmt(selected.measuredTokens)}</dd><dt>额度占用</dt><dd>{fmt(selected.quotaHeldTokens)}</dd><dt>用量状态</dt><dd>{selected.usageState}</dd><dt>已知报价</dt><dd>{moneyKnown(selected.quotePicoUsd)}{selected.quotePicoUsd === null ? '（未定价）' : '（仅本次有记录的报价）'}</dd><dt>失败码</dt><dd>{selected.failureCode ?? '无记录'}</dd><dt>模型配置</dt><dd className={styles.breakable}>{selected.modelConfigRef ?? '—'}</dd><dt>配置版本</dt><dd className={styles.breakable}>{selected.taskConfigRef ?? '—'}</dd><dt>调用标识</dt><dd className={styles.breakable}>{selected.invocationRef}</dd></dl></Drawer> : null}
  </div>;
}
export function AiAdvanced({ data, action }: { data: AiSettings; action: AiAction }) {
  const [query, setQuery] = useSearchParams();
  const area = query.get('area') ?? 'controls';
  const controls = data.controls.filter(c => c.taskKey === null);
  return <div className={styles.experience}><div className={styles.sectionHeading}><div><h2>高级配置</h2><small>运行安全、材料许可、工作额度与固定做法版本</small></div></div>
    <nav className={styles.advancedTabs} aria-label="高级配置类型">{([['controls','开关与安全'],['permissions','材料许可'],['work','工作配置与额度'],['skills','做法与试跑']] as const).map(([key, label]) => <button type="button" key={key} aria-current={area === key ? 'page' : undefined} onClick={() => setQuery({ view: 'advanced', area: key, ...(key === 'work' && query.get('task') ? { task: query.get('task')! } : {}) })}>{label}</button>)}</nav>
    {area === 'controls' ? <><div className={styles.advancedGrid}>{controls.map(c => <div className={styles.controlBox} key={c.scope}><small>{c.scope === 'global' ? 'GLOBAL AI' : 'AI EXECUTOR'}</small><ControlAction control={c} action={action} label={c.scope === 'global' ? '全局 AI' : '执行器'}/></div>)}</div><p className={styles.callout}>全局、执行器、工作开关分别生效。操作需填写原因；启用不代表材料许可或模型已经通过，也不会自动创建新任务。</p></> : null}
    {area === 'permissions' ? <div className={styles.panel}><h3 className={styles.blockTitle}>按模型地址与业务材料授权</h3><p className={styles.subText}>许可的真实粒度为当前端点 × 工作 × 材料；地址变更后不会恢复旧授权。</p><PermissionPicker data={data} action={action}/></div> : null}
    {area === 'work' ? <div className={styles.advancedContent}><AiTaskSection data={data} action={action} linkBase="?view=advanced&area=work&task="/></div> : null}
    {area === 'skills' ? <div className={styles.advancedContent}><AiSkillSection data={data} action={action} linkBase="?view=advanced&area=skills&task="/></div> : null}
    <p className={styles.footnote}>操作将写入真实配置与回执。固定做法仅在完成所需试跑和显式启用后用于新批次，不重写历史。</p>
  </div>;
}
function PermissionPicker({ data, action }: { data: AiSettings; action: AiAction }) {
  const selectable = data.connections.filter(c => c.api !== 'local-wemm-runtime');
  const [selected, setSelected] = useState(selectable[0]?.connectionRef ?? '');
  const connection = selectable.find(c => c.connectionRef === selected);
  if (!selectable.length) return <p className={styles.callout}>当前没有可管理材料许可的外部或 HTTP 模型服务。</p>;
  if (!connection) return null;
  return <div className={styles.form}><label>选择模型服务<select value={selected} onChange={event => setSelected(event.target.value)}>{selectable.map(c => <option key={c.connectionRef} value={c.connectionRef}>{c.name} · {c.baseUrl}</option>)}</select></label><div className={styles.legacyForm}><EgressForm key={connection.endpointRef} data={data} connection={connection} action={action}/></div></div>;
}
