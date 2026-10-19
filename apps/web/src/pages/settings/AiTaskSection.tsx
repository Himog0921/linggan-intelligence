import { useState, type FormEvent } from 'react';
import { Link, useSearchParams } from 'react-router';
import { command } from './aiSettingsApi';
import { budgetText, controlReason, controlStatus, count, TASK_STATE, THINKING, type AiSettings, type Control, type ModelLimits, type Task } from './aiSettingsModel';
import { Field, Impact, Reference, SaveBar } from './AiSettingsFields';
import type { AiAction } from './useAiAction';
import styles from './AiSettings.module.css';
import { readLimits } from './aiSettingsForms';

export function ControlAction({ control, action, label }: { control: Control; action: AiAction; label: string }) {
  const [reason, setReason] = useState('');
  const enabled = !control.enabled;
  function change() {
    const payload = { scope: control.taskKey ? { kind: 'task', task: control.taskKey } : { kind: control.scope }, expectedRevision: control.revision, enabled, reason };
    void action.run(`control:${control.scope}`, payload, commandRef => command('control', { ...payload, commandRef }), enabled ? `${label}已启用，实际执行仍需通过所有准入条件。` : `${label}已暂停，已经发出的请求会保留最小用量回执。`);
  }
  return <Impact title={`${label} · ${controlStatus(control)}`} label={enabled ? `启用${label}` : `暂停${label}`} action={change}
    disabled={action.busy !== null || !reason.trim() || (enabled && control.lifecycle !== null && control.lifecycle !== 'active')}>
    <p>{controlReason(control) || '尚无操作原因'}。{control.taskKey ? TASK_STATE[control.lifecycle ?? ''] ?? '接入状态未知' : '仅改变本层开关，不改变其他层开关。'}</p>
    <Field label={`${label}操作原因`}><input value={reason} maxLength={300} onChange={e => setReason(e.target.value)} /></Field>
    <small>启用后允许领取已有的合格队列；暂停不取消已完成结果，也不重新启动终态批次。</small>
  </Impact>;
}
export function LimitsFields({ limits, max, levels }: { limits: ModelLimits | null; max: { contextTokens: number; maxOutputTokens: number; maxTimeoutSeconds: number; maxAttempts: number }; levels: string[] }) {
  return <div className={styles.grid}>
    <Field label="输入 token 上限"><input type="number" name="inputTokens" min={1024} max={max.contextTokens} required defaultValue={limits?.inputTokens} /></Field>
    <Field label="输出 token 上限"><input type="number" name="outputTokens" min={16} max={max.maxOutputTokens} required defaultValue={limits?.outputTokens} /></Field>
    <Field label="单次等待秒数"><input type="number" name="timeoutSeconds" min={1} max={max.maxTimeoutSeconds} required defaultValue={limits?.timeoutSeconds} /></Field>
    <Field label="最多尝试次数"><input type="number" name="attempts" min={1} max={max.maxAttempts} required defaultValue={limits?.attempts ?? 1} /></Field>
    <Field label="思考等级"><select name="thinkingLevel" defaultValue={limits?.thinkingLevel ?? ''} required><option value="">请选择</option>{levels.map(level => <option key={level} value={level}>{THINKING[level] ?? level}</option>)}</select></Field>
    <p><small>输入与输出之和必须在已声明的上下文容量内；保存时由服务端核对。</small></p>
  </div>;
}
const COLUMNS: Record<string, string> = { title: '标题', cover_headline: '封面文字', body: '正文', tags: '标签', ocr: '图片文字', transcript: '转录文字' };
function TaskForm({ data, task, action }: { data: AiSettings; task: Task; action: AiAction }) {
  const current = task.configuration;
  const [identity, setIdentity] = useState(current.model?.identityRef ?? '');
  const [budgetMode, setBudgetMode] = useState(current.budget.mode);
  const selected = data.models.find(m => m.identityRef === identity);
  const modelRequired = task.taskKey !== 'model_probe';
  async function save(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const form = new FormData(event.currentTarget);
    try {
      const model = modelRequired ? selected?.capability && selected.selectionModelRef ? { modelRef: selected.selectionModelRef, capabilityRef: selected.capability.capabilityRef, limits: readLimits(form) } : undefined : null;
      if (model === undefined) throw new Error('请先在模型区为当前地址登记模型能力。');
      const budget = budgetMode === 'capped' ? { mode: budgetMode, tokens: String(form.get('budgetTokens')) } : { mode: budgetMode };
      const payload = { taskKey: task.taskKey, expectedRevision: task.revision, model, skillVersionRef: String(form.get('skillVersionRef') ?? '') || null, materialColumns: form.getAll('materialColumns'), budget };
      await action.run(`config:${task.taskKey}`, payload, commandRef => command('task-configs', { ...payload, commandRef }), receipt => receipt.applied ? '配置已保存，用于后续批次；开关保持当前状态。' : '候选配置已保存，当前做法保持原版本。请到做法区试跑比较后启用。');
    } catch (error) { action.setNotice({ tone: 'error', text: error instanceof Error ? error.message : '请核对配置。' }); }
  }
  return <form className={styles.form} onSubmit={event => void save(event)}>
    <h3>{task.label}配置</h3><p>{TASK_STATE[task.lifecycle] ?? '接入状态未知'} · {budgetText(current.budget)}</p>
    {modelRequired ? <><Field label="选择模型"><select value={identity} onChange={event => setIdentity(event.target.value)} required><option value="">请选择</option>{data.models.map(model => <option key={model.identityRef} value={model.identityRef}>{data.connections.find(c => c.connectionRef === model.connectionRef)?.name} · {model.modelId}{model.capability ? '' : '（能力待配置）'}</option>)}</select></Field>
      {selected?.capability ? <LimitsFields key={identity} limits={current.model?.identityRef === identity ? current.model.limits : null} max={{ ...selected.capability, maxAttempts: data.resources.maxAttempts }} levels={selected.capability.thinkingLevels} /> : <p>当前地址没有已声明能力，请先到模型区补齐。</p>}</> : <p>模型测试在模型区逐次选择连接与模型；此处仅配置测试任务的额度。</p>}
    {task.skillAllowed ? <Field label="固定做法版本" hint="选择新版本会保存候选，试跑并启用后才替换当前版本。"><select name="skillVersionRef" defaultValue={current.skillVersionRef ?? ''} required={task.requiresSkill}><option value="">暂不绑定</option>{data.skills.flatMap(skill => skill.versions.map(version => <option key={version.versionRef} value={version.versionRef}>{skill.name} · v{version.versionNo}{version.disableModelInvocation ? '（不供模型自动发现，可人工绑定）' : ''}</option>))}</select></Field> : null}
    {['note_interpretation', 'direction_list_draft', 'direction_proposal'].includes(task.taskKey) ? <fieldset><legend>允许输入的材料列</legend><div className={styles.checklist}>{Object.entries(COLUMNS).map(([key, label]) => <label key={key}><input type="checkbox" name="materialColumns" value={key} defaultChecked={current.materialColumns.includes(key)} />{label}</label>)}</div></fieldset> : null}
    <div className={styles.grid}><Field label="每日额度"><select value={budgetMode} onChange={event => { const value = event.target.value; if (value === 'unconfigured' || value === 'unlimited' || value === 'capped') setBudgetMode(value); }}><option value="unconfigured">待配置</option><option value="unlimited">当前不设每日上限</option><option value="capped">指定每日 token 上限</option></select></Field>
      {budgetMode === 'capped' ? <Field label="每日 token 上限" hint="预留额度包含每次输入与输出；未知用量仍占预留。"><input name="budgetTokens" inputMode="numeric" pattern="[1-9][0-9]*" required defaultValue={current.budget.mode === 'capped' ? current.budget.tokens : ''} /></Field> : null}</div>
    <details><summary>当前版本与配置依据</summary><p>配置 <Reference value={current.taskConfigRef} />；修订 {count(task.revision)}</p><p>已启用做法 <Reference value={task.activeSkillVersionRef} />；{task.approvalReason ?? '尚未启用做法'}</p></details>
    <SaveBar busy={action.busy !== null} />
  </form>;
}
export function AiTaskSection({ data, action }: { data: AiSettings; action: AiAction }) {
  const [query] = useSearchParams();
  const task = data.tasks.find(t => t.taskKey === query.get('task')) ?? data.tasks[0];
  return <section className={styles.section} aria-labelledby="tasks-title"><h2 id="tasks-title">自动化与额度</h2><p>全局、执行器、工作三层开关分别记录。所有开关开启后，仍需通过当前配置、材料许可和额度检查。</p>
    <div className={styles.grid}>{data.controls.filter(c => c.taskKey === null).map(control => <ControlAction key={`${control.scope}:${control.revision}`} control={control} action={action} label={control.scope === 'global' ? '全局 AI' : '执行器'} />)}</div>
    <nav className={styles.selector} aria-label="工作清单">{data.tasks.map(t => <Link key={t.taskKey} to={`?section=automation&task=${encodeURIComponent(t.taskKey)}`} aria-current={task?.taskKey === t.taskKey ? 'page' : undefined}><span>{t.label}</span><small>{TASK_STATE[t.lifecycle] ?? '状态未知'}</small></Link>)}</nav>
    {task ? <>{['active', 'pending'].includes(task.lifecycle) ? <TaskForm key={`${task.taskKey}:${task.revision}`} task={task} data={data} action={action} /> : <section className={styles.group}><h3>{task.label} · 历史配置</h3><p>{TASK_STATE[task.lifecycle] ?? '接入状态未识别'}；保留历史版本，当前不提供配置或启用入口。</p><Reference value={task.configuration.taskConfigRef} /></section>}{data.controls.filter(c => c.taskKey === task.taskKey).map(control => <ControlAction key={`${control.scope}:${control.revision}`} control={control} action={action} label={task.label} />)}</> : <p>尚未取得工作注册表。</p>}
  </section>;
}
