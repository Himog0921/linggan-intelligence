import { useQuery, useQueryClient } from '@tanstack/react-query';
import { useState, type FormEvent } from 'react';
import { Link, useSearchParams } from 'react-router';
import { ActionButton } from '../../shared/ActionButton';
import { parseWritingSuggestion } from '../workbench/workbenchModel';
import { AI_SETTINGS_KEY, command, fetchComparison, fetchSamples, fetchSkill } from './aiSettingsApi';
import { BATCH_STATE, TASK_STATE, type AiSettings, type Json, type Task } from './aiSettingsModel';
import { Field, Moment, Reference } from './AiSettingsFields';
import { readSkillFolder } from './aiSkillUpload';
import type { AiAction } from './useAiAction';
import styles from './AiSettings.module.css';
const SOURCE_QUALIFICATION: Record<string, string> = { readable: '当前可读', unavailable: '当前来源不可读取或已换版；历史产出已隐藏', not_connected: '这类输入尚未接入读取' };

function SkillImport({ data, action }: { data: AiSettings; action: AiAction }) {
  const [files, setFiles] = useState<File[]>([]);
  const [reading, setReading] = useState(false);
  async function importFolder(event: FormEvent<HTMLFormElement>) {
    event.preventDefault(); if (reading) return; setReading(true);
    try {
      const packet = await readSkillFolder(files, data.resources);
      const current = data.skills.find(skill => skill.name === packet.directory)?.versions[0];
      const payload = { expectedVersionRef: current?.versionRef ?? null, package: packet };
      await action.run(`import:${packet.directory}`, payload, commandRef => command('skills/import', { ...payload, commandRef }), receipt => receipt.reused ? '该做法包已存在，沿用原版本；未切换任何工作。' : `已导入做法 v${receipt.versionNo}，尚未试跑或启用。`);
    } catch (error) { action.setNotice({ tone: 'error', text: error instanceof Error ? error.message : '目录读取失败。' }); }
    finally { setReading(false); }
  }
  return <form className={styles.form} onSubmit={event => void importFolder(event)}><h3>从本地目录导入</h3><p>在本地修改 SKILL.md 与引用文件，再重新导入生成不可变版本。最多 {data.resources.maxSkillFiles} 个文件，单文件 {data.resources.maxSkillFileBytes.toLocaleString()} 字节，总计 {data.resources.maxSkillTotalBytes.toLocaleString()} 字节。</p>
    <Field label="选择做法目录"><input type="file" multiple ref={node => { node?.setAttribute('webkitdirectory', ''); }} onChange={event => setFiles(Array.from(event.target.files ?? []))} /></Field>
    {files.length ? <details><summary>已选择 {files.length} 个文件</summary><ul>{files.map(file => <li key={file.webkitRelativePath}>{file.webkitRelativePath} · {file.size.toLocaleString()} 字节</li>)}</ul></details> : null}
    <ActionButton type="submit" variant="primary" disabled={action.busy !== null || !files.length} busy={reading}>导入做法包</ActionButton>
  </form>;
}
function VersionRead({ reference }: { reference: string }) {
  const query = useQuery({ queryKey: ['ai-skill', reference], queryFn: ({ signal }) => fetchSkill(reference, signal), retry: false });
  if (query.isPending) return <p>正在读取固定版本…</p>;
  if (query.isError) return <p role="alert">版本读取失败，未展示残缺内容。</p>;
  return <div className={styles.group}><h3>{query.data.name} · v{query.data.versionNo}</h3><p>{query.data.description}</p><p>{query.data.disableModelInvocation ? '此版本声明不供模型自动发现；人工固定绑定仍须通过当前运行准入。' : '此版本可供发现和工作配置引用，实际调用仍须通过当前运行准入。'}</p>
    <details><summary>只读做法内容</summary><pre className={styles.code}>{query.data.body}</pre></details>
    <details><summary>文件与版本校验</summary><p>包 <Reference value={query.data.packageSha256} /></p><p>原始包 <Reference value={query.data.sourceSha256} /></p><ul>{query.data.files.map(file => <li key={file.path}>{file.path} · {file.bytes.toLocaleString()} 字节 · {file.kind}</li>)}</ul></details>
  </div>;
}
function OutputRead({ output }: { output: Json }) {
  if (output === null) return <p>尚无可展示产出，或来源资格已经变化。</p>;
  let suggestion;
  try { suggestion = parseWritingSuggestion(output); } catch { suggestion = null; }
  if (!suggestion) return <p>产出格式无法识别，未展示残缺内容。</p>;
  return <div>{suggestion.title ? <h3>{suggestion.title}</h3> : null}{suggestion.paragraphs.map((p, index) => <p key={index}>{p.text}</p>)}{suggestion.pages.map((p, index) => <section key={index}><h3>{p.heading}</h3><p>{p.body}</p></section>)}</div>;
}
function TrialPanel({ data, task, action }: { data: AiSettings; task: Task; action: AiAction }) {
  const client = useQueryClient();
  const configs = data.configurations.filter(c => c.taskKey === task.taskKey && c.skillVersionRef !== null);
  const [candidateRef, setCandidateRef] = useState(configs.find(c => c.taskConfigRef !== task.configuration.taskConfigRef)?.taskConfigRef ?? configs[0]?.taskConfigRef ?? '');
  const [baselineRef, setBaselineRef] = useState(task.activeSkillVersionRef ? task.configuration.taskConfigRef : '');
  const [previewRef, setPreviewRef] = useState('');
  const [comparisonRef, setComparisonRef] = useState('');
  const [position, setPosition] = useState(0);
  const [firstVersionException, setFirstVersionException] = useState(false);
  const [reason, setReason] = useState('');
  const candidate = configs.find(config => config.taskConfigRef === candidateRef);
  const candidateVersion = data.skills.flatMap(skill => skill.versions).find(version => version.versionRef === candidate?.skillVersionRef);
  const samples = useQuery({ queryKey: ['ai-writing-samples'], queryFn: ({ signal }) => fetchSamples(signal), retry: false });
  const comparisons = data.trials.filter(c => c.taskKey === task.taskKey && c.candidateConfigRef === candidateRef);
  const comparison = useQuery({ queryKey: ['ai-trial', comparisonRef, position], queryFn: ({ signal }) => fetchComparison(comparisonRef, position, signal), enabled: comparisonRef !== '', retry: false });
  async function refreshComparison() {
    await Promise.all([comparison.refetch(), client.invalidateQueries({ queryKey: AI_SETTINGS_KEY })]);
  }
  function configLabel(reference: string) {
    const config = configs.find(c => c.taskConfigRef === reference);
    const skill = data.skills.find(s => s.versions.some(v => v.versionRef === config?.skillVersionRef));
    const version = skill?.versions.find(v => v.versionRef === config?.skillVersionRef);
    return `${skill?.name ?? '做法'} v${version?.versionNo ?? '?'} · ${config?.model?.modelId ?? '模型待配'} · ${config?.createdAt ?? ''}`;
  }
  async function createTrial(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const payload = { taskKey: task.taskKey, expectedTaskRevision: task.revision, baselineConfigRef: baselineRef || null, candidateConfigRef: candidateRef, items: [{ itemKey: previewRef, input: { kind: 'writing_preview', previewRef } }] };
    const receipt = await action.run(`trial:${task.taskKey}`, payload, commandRef => command('trials', { ...payload, commandRef }), '试跑批次已接纳，两个版本使用同一份固定输入；请等待实际执行回执。');
    if (receipt) { setComparisonRef(receipt.comparisonRef); setPosition(0); }
  }
  function activate() {
    const payload = { taskKey: task.taskKey, expectedTaskRevision: task.revision, expectedBindingRevision: task.bindingRevision, candidateConfigRef: candidateRef, trialComparisonRef: comparisonRef || null, firstVersionException, reason };
    void action.run(`activate:${task.taskKey}`, payload, commandRef => command('skills/activate', { ...payload, commandRef }), '固定做法版本已启用，用于后续批次；未开启工作开关或创建任务。');
  }
  return <div className={styles.group}><h3>{task.label} · 试跑与启用</h3><p>{TASK_STATE[task.lifecycle] ?? '接入状态未知'}。先在自动化与额度区保存模型、资源和候选做法版本。</p>
    <div className={styles.grid}><Field label="候选配置"><select value={candidateRef} onChange={event => { setCandidateRef(event.target.value); setComparisonRef(''); setPosition(0); setFirstVersionException(false); }}><option value="">请选择</option>{configs.map(c => <option key={c.taskConfigRef} value={c.taskConfigRef}>{configLabel(c.taskConfigRef)}</option>)}</select></Field>
      <Field label="固定旧配置"><select value={baselineRef} onChange={event => setBaselineRef(event.target.value)}><option value="">首版，无旧配置</option>{configs.map(c => <option key={c.taskConfigRef} value={c.taskConfigRef}>{configLabel(c.taskConfigRef)}</option>)}</select></Field></div>
    <form className={styles.form} onSubmit={event => void createTrial(event)}><Field label="试跑样本"><select value={previewRef} required onChange={event => setPreviewRef(event.target.value)}><option value="">请选择合格写作预览</option>{samples.data?.samples.map(s => <option key={s.previewRef} value={s.previewRef}>{s.title || '未命名作品'} · {s.action} · {s.createdAt}</option>)}</select></Field>
      <p><small>{samples.isError ? '样本读取失败。' : samples.isPending ? '正在读取样本…' : samples.data.state === 'not_connected' ? '写作预览选择尚未接入，暂不能选择试跑样本。' : `仅检查最近 ${samples.data.limit} 份仍对应当前作品版本的预览；${samples.data.samples.length} 份通过当前来源检查。未列出不表示库中不存在。`}</small></p>
      <div className={styles.impact}><p>试跑会分别运行所选固定配置，可能产生两份模型消耗；仍需当前开关、额度、材料和地址许可。未接入工作暂不能执行试跑。</p><ActionButton variant="primary" type="submit" disabled={action.busy !== null || task.lifecycle !== 'active' || !candidateRef || !previewRef}>提交固定样本试跑</ActionButton></div>
    </form>
    <Field label="查看试跑比较"><select value={comparisonRef} onChange={event => { setComparisonRef(event.target.value); setPosition(0); }}><option value="">请选择</option>{comparisons.map(c => <option key={c.comparisonRef} value={c.comparisonRef}>{c.createdAt} · {BATCH_STATE[c.candidateState] ?? c.candidateState} · {c.itemCount} 篇</option>)}</select></Field>
    {comparisonRef ? <div className={styles.group}><ActionButton onClick={() => void refreshComparison()} disabled={comparison.isFetching}>刷新比较回执</ActionButton>{comparison.isError ? <p role="alert">比较读取失败。</p> : comparison.isFetching ? <p>正在重核本次材料资格，产出暂不展示…</p> : comparison.data ? <><p>{comparison.data.qualifiesForActivation ? '固定试跑已完成，具备版本比较回执。' : '固定试跑尚未完成合格比较。'} 本次材料读取：{SOURCE_QUALIFICATION[comparison.data.sourceQualification] ?? '当前资格未确认，产出已隐藏'}</p><div className={styles.grid}>{[comparison.data.baseline, comparison.data.candidate].map((side, i) => <section key={i}><h3>{i === 0 ? '固定旧版本' : '候选版本'}</h3>{side ? <><p>{BATCH_STATE[side.itemState] ?? side.itemState} · 尝试 {side.attemptCount} 次 · {side.failureCode ?? '未记录失败码'}</p><OutputRead output={comparison.data.sourceQualification === 'readable' ? side.output : null} /><details><summary>运行版本</summary><Reference value={side.taskConfigRef} /><p>{side.executionContract}</p><Reference value={side.skillVersionRef} /></details></> : <p>首版，无旧版本。</p>}</section>)}</div><div className={styles.actions}><ActionButton disabled={position === 0} onClick={() => setPosition(p => p - 1)}>上一篇</ActionButton><span>第 {position + 1} / {comparison.data.itemCount} 篇</span><ActionButton disabled={position + 1 >= comparison.data.itemCount} onClick={() => setPosition(p => p + 1)}>下一篇</ActionButton></div></> : <p>正在读取比较…</p>}</div> : null}
    <section className={styles.impact}><h3>启用候选做法</h3><p>只改变后续批次的固定配置；新版本需合格试跑。曾批准的版本可由服务端核对后恢复。首版例外必须填写理由。</p>{candidateVersion?.versionNo === 1 ? <label><input type="checkbox" checked={firstVersionException} onChange={event => setFirstVersionException(event.target.checked)} /> 此做法在本工作首版，申请跳过旧新比较</label> : null}
      <Field label="启用理由"><input value={reason} maxLength={300} required onChange={event => setReason(event.target.value)} /></Field><ActionButton onClick={activate} disabled={action.busy !== null || !candidateRef || !reason.trim()}>启用候选做法</ActionButton>
    </section>
  </div>;
}
export function AiSkillSection({ data, action }: { data: AiSettings; action: AiAction }) {
  const [query] = useSearchParams(); const versionRef = query.get('version');
  const task = data.tasks.find(t => t.taskKey === query.get('task') && t.skillAllowed) ?? data.tasks.find(t => t.taskKey === 'writing_legacy');
  return <section className={styles.section} aria-labelledby="skills-title"><h2 id="skills-title">做法</h2><SkillImport data={data} action={action} />
    <div className={styles.group}><h3>已导入版本</h3>{data.skills.length ? <nav className={styles.selector} aria-label="做法版本">{data.skills.flatMap(skill => skill.versions.map(version => <Link key={version.versionRef} to={`?section=skills&version=${version.versionRef}${task ? `&task=${task.taskKey}` : ''}`} aria-current={version.versionRef === versionRef ? 'page' : undefined}>{skill.name} · v{version.versionNo}<small><Moment value={version.createdAt} /></small></Link>))}</nav> : <p>尚未导入做法。导入完成后再保存候选配置。</p>}{versionRef ? <VersionRead reference={versionRef} /> : null}</div>
    <nav className={styles.selector} aria-label="做法绑定的工作">{data.tasks.filter(t => t.skillAllowed).map(t => <Link key={t.taskKey} to={`?section=skills&task=${t.taskKey}`} aria-current={task?.taskKey === t.taskKey ? 'page' : undefined}>{t.label}</Link>)}</nav>
    {task ? <TrialPanel key={`${task.taskKey}:${task.revision}`} data={data} task={task} action={action} /> : null}
  </section>;
}
