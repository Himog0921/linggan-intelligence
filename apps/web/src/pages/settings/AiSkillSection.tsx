import { useQuery, useQueryClient } from '@tanstack/react-query';
import { useState, type FormEvent } from 'react';
import { Link, useSearchParams } from 'react-router';
import { ActionButton } from '../../shared/ActionButton';
import { parseWritingSuggestion } from '../workbench/workbenchModel';
import { AI_SETTINGS_KEY, command, fetchComparison, fetchPrompts, fetchSamples, fetchSkill, type parseSkill } from './aiSettingsApi';
import { BATCH_STATE, TASK_STATE, type AiSettings, type Json, type Task } from './aiSettingsModel';
import { Field, Moment, Reference } from './AiSettingsFields';
import { readSkillFolder } from './aiSkillUpload';
import type { AiAction } from './useAiAction';
import styles from './AiSettings.module.css';
const SOURCE_QUALIFICATION: Record<string, string> = { readable: '当前可读', unavailable: '当前来源不可读取；历史产出已隐藏', not_connected: '这类输入尚未接入读取' };

function PromptEditor({ task, revision, version, active, action, onSaved }: {
  task: string; revision: number; version: Awaited<ReturnType<typeof fetchPrompts>>['versions'][number];
  active: boolean; action: AiAction; onSaved: (reference?: string) => Promise<void>;
}) {
  const [systemText, setSystemText] = useState(version.systemText);
  const [instructionText, setInstructionText] = useState(version.instructionText);
  const dirty = systemText !== version.systemText || instructionText !== version.instructionText;
  async function save(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const payload = { taskKey: task, expectedTaskRevision: revision, systemText, instructionText };
    const receipt = await action.run(`prompt-save:${task}`, payload, commandRef => command('prompts/save', { ...payload, commandRef }),
      result => `提示词 v${result.versionNo} 已保存，尚未启用。`);
    if (receipt) await onSaved(receipt.versionRef);
  }
  async function activate() {
    const payload = { taskKey: task, expectedTaskRevision: revision, versionRef: version.versionRef };
    const receipt = await action.run(`prompt-activate:${task}`, payload, commandRef => command('prompts/activate', { ...payload, commandRef }),
      '提示词已切换，后续任务使用所选版本。');
    if (receipt) await onSaved();
  }
  return <form className={styles.form} onSubmit={event => void save(event)}><h4>提示词 v{version.versionNo}{active ? ' · 当前启用' : ''}</h4>
    <Field label="系统提示词"><textarea value={systemText} onChange={event => setSystemText(event.target.value)} rows={9} required /></Field>
    <Field label="任务指令"><textarea value={instructionText} onChange={event => setInstructionText(event.target.value)} rows={9} required /></Field>
    <div className={styles.actions}><ActionButton type="submit" variant="primary" disabled={action.busy !== null || !systemText.trim() || !instructionText.trim()}>保存提示词新版本</ActionButton>
      <ActionButton type="button" onClick={() => void activate()} disabled={action.busy !== null || active || dirty}>启用此提示词版本</ActionButton></div>
    <p>启用旧版本即可回退；未保存的修改需先保存新版本。已排队的任务保持原版本，保存不会调用模型。</p>
  </form>;
}
function PromptPanel({ task, action }: { task: Task; action: AiAction }) {
  const client = useQueryClient();
  const [selected, setSelected] = useState('');
  const key = ['ai-prompts', task.taskKey] as const;
  const query = useQuery({ queryKey: key, queryFn: ({ signal }) => fetchPrompts(task.taskKey, signal), retry: false });
  if (query.isPending) return <p>正在读取提示词版本…</p>;
  if (query.isError) return <p role="alert">提示词读取失败，请重试。</p>;
  const version = query.data.versions.find(item => item.versionRef === selected)
    ?? query.data.versions.find(item => item.versionRef === query.data.activeVersionRef);
  async function refresh(reference?: string) {
    await client.invalidateQueries({ queryKey: key });
    if (reference) setSelected(reference);
  }
  return <section className={styles.group}><h3>{task.label} · 提示词</h3>
    <Field label="提示词版本"><select value={version?.versionRef ?? ''} onChange={event => setSelected(event.target.value)}>
      {query.data.versions.map(item => <option key={item.versionRef} value={item.versionRef}>v{item.versionNo}{item.versionRef === query.data.activeVersionRef ? ' · 当前启用' : ''}</option>)}
    </select></Field>
    {version ? <PromptEditor key={version.versionRef} task={task.taskKey} revision={query.data.revision} version={version}
      active={version.versionRef === query.data.activeVersionRef} action={action} onSaved={refresh} /> : <p>尚无提示词版本。</p>}
  </section>;
}
function SkillSwitch({ data, task, action }: { data: AiSettings; task: Task; action: AiAction }) {
  const [versionRef, setVersionRef] = useState(task.activeSkillVersionRef ?? '');
  async function switchVersion() {
    const payload = { taskKey: task.taskKey, expectedTaskRevision: task.revision, expectedBindingRevision: task.bindingRevision, versionRef };
    await action.run(`skill-switch:${task.taskKey}`, payload, commandRef => command('skills/switch', { ...payload, commandRef }),
      '做法已切换，当前模型、提示词和额度保持不变。');
  }
  return <section className={styles.group}><h3>{task.label} · 切换做法</h3>
    <Field label="切换做法版本"><select value={versionRef} onChange={event => setVersionRef(event.target.value)}><option value="">请选择做法版本</option>
      {data.skills.flatMap(skill => skill.versions.map(version => <option key={version.versionRef} value={version.versionRef}>{skill.name} · v{version.versionNo}{version.versionRef === task.activeSkillVersionRef ? ' · 当前启用' : ''}</option>))}
    </select></Field>
    <ActionButton type="button" onClick={() => void switchVersion()} disabled={action.busy !== null || !['active', 'pending'].includes(task.lifecycle) || !versionRef || versionRef === task.activeSkillVersionRef}>切换做法版本</ActionButton>
    <p>选择旧版本即可回退；已排队任务保留原做法。切换不会运行模型。{task.lifecycle === 'pending' ? '此工作尚未接入执行，可以先准备做法。' : ''}</p>
  </section>;
}
function SkillCreate({ action }: { action: AiAction }) {
  const [name, setName] = useState('');
  const [content, setContent] = useState('');
  async function create(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const payload = { expectedVersionRef: null, package: { directory: name, files: [{ path: 'SKILL.md', encoding: 'utf8', content }] } };
    await action.run(`create:${name}`, payload, commandRef => command('skills/import', { ...payload, commandRef }),
      receipt => `已创建做法 v${receipt.versionNo}；尚未启用或运行。`);
  }
  return <form className={styles.form} onSubmit={event => void create(event)}><h3>新建做法</h3>
    <Field label="做法名称" hint="小写字母、数字和连字符；与 SKILL.md 中的 name 一致。"><input value={name} onChange={event => setName(event.target.value)} pattern="[a-z0-9]+(-[a-z0-9]+)*" maxLength={64} required /></Field>
    <Field label="SKILL.md 内容" hint="包含 name、description 和做法正文。"><textarea value={content} onChange={event => setContent(event.target.value)} rows={14} required /></Field>
    <ActionButton type="submit" variant="primary" disabled={action.busy !== null || !name || !content.trim()}>创建做法</ActionButton>
  </form>;
}
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
  return <form className={styles.form} onSubmit={event => void importFolder(event)}><h3>从本地目录导入</h3><p>导入包含 SKILL.md 与引用文件的目录，保存为新版本。最多 {data.resources.maxSkillFiles} 个文件，单文件 {data.resources.maxSkillFileBytes.toLocaleString()} 字节，总计 {data.resources.maxSkillTotalBytes.toLocaleString()} 字节。</p>
    <Field label="选择做法目录"><input type="file" multiple ref={node => { node?.setAttribute('webkitdirectory', ''); }} onChange={event => setFiles(Array.from(event.target.files ?? []))} /></Field>
    {files.length ? <details><summary>已选择 {files.length} 个文件</summary><ul>{files.map(file => <li key={file.webkitRelativePath}>{file.webkitRelativePath} · {file.size.toLocaleString()} 字节</li>)}</ul></details> : null}
    <ActionButton type="submit" variant="primary" disabled={action.busy !== null || !files.length} busy={reading}>导入做法包</ActionButton>
  </form>;
}
function VersionEditor({ version, data, action }: { version: ReturnType<typeof parseSkill>; data: AiSettings; action: AiAction }) {
  const [files, setFiles] = useState(() => version.files.map(file => ({
    path: file.path, encoding: file.kind === 'binary' ? 'base64' as const : 'utf8' as const,
    content: file.kind === 'binary' ? file.contentBase64 : file.content,
  })));
  // Keep the revision observed when this draft opened, even if settings refresh in the background.
  const [baseVersionRef] = useState(() => data.skills.find(skill => skill.skillRef === version.skillRef)?.versions[0]?.versionRef);
  const incomplete = files.some(file => file.content === null) || !baseVersionRef;
  async function save(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (incomplete || !baseVersionRef) return;
    const packet = { directory: version.name, files: files.map(file => ({ ...file, content: file.content ?? '' })) };
    const payload = { expectedVersionRef: baseVersionRef, package: packet };
    await action.run(`edit:${version.skillRef}`, payload, commandRef => command('skills/import', { ...payload, commandRef }),
      receipt => receipt.reused ? '内容与现有版本相同，未切换任何工作。' : `已保存做法 v${receipt.versionNo}；尚未启用或运行。`);
  }
  return <form className={styles.form} onSubmit={event => void save(event)}>
    <h4>编辑做法文件</h4><p>基于 v{version.versionNo} 保存新版本；当前运行与已启用版本保持原样。</p>
    {files.map((file, index) => file.encoding === 'base64' ? <p key={file.path}>{file.path} · 二进制文件，保存时原样保留</p> :
      <Field key={file.path} label={file.path}><textarea value={file.content ?? ''} rows={file.path === 'SKILL.md' ? 18 : 8} onChange={event => {
        const content = event.target.value;
        setFiles(current => current.map((item, i) => i === index ? { ...item, content } : item));
      }} /></Field>)}
    {incomplete ? <p role="alert">文件内容或最新版本未完整读取，暂时不能保存。</p> : null}
    <ActionButton type="submit" variant="primary" disabled={incomplete || action.busy !== null}>保存新版本</ActionButton>
  </form>;
}
function VersionRead({ reference, data, action }: { reference: string; data: AiSettings; action: AiAction }) {
  const query = useQuery({ queryKey: ['ai-skill', reference], queryFn: ({ signal }) => fetchSkill(reference, signal), retry: false });
  if (query.isPending) return <p>正在读取固定版本…</p>;
  if (query.isError) return <p role="alert">版本读取失败，未展示残缺内容。</p>;
  return <div className={styles.group}><h3>{query.data.name} · v{query.data.versionNo}</h3><p>{query.data.description}</p><p>{query.data.disableModelInvocation ? '此版本声明不供模型自动发现；人工固定绑定仍须通过当前运行准入。' : '此版本可供发现和工作配置引用，实际调用仍须通过当前运行准入。'}</p>
    <details><summary>只读做法内容</summary><pre className={styles.code}>{query.data.body}</pre></details>
    <details><summary>文件与版本校验</summary><p>包 <Reference value={query.data.packageSha256} /></p><p>原始包 <Reference value={query.data.sourceSha256} /></p><ul>{query.data.files.map(file => <li key={file.path}>{file.path} · {file.bytes.toLocaleString()} 字节 · {file.kind}</li>)}</ul></details>
    <VersionEditor key={query.data.versionRef} version={query.data} data={data} action={action} />
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
  const [reason, setReason] = useState('');
  const writingTrial = task.taskKey === 'writing_legacy';
  const samples = useQuery({ queryKey: ['ai-writing-samples'], queryFn: ({ signal }) => fetchSamples(signal), enabled: writingTrial, retry: false });
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
    const payload = { taskKey: task.taskKey, expectedTaskRevision: task.revision, expectedBindingRevision: task.bindingRevision, candidateConfigRef: candidateRef, trialComparisonRef: comparisonRef || null, reason };
    void action.run(`activate:${task.taskKey}`, payload, commandRef => command('skills/activate', { ...payload, commandRef }), '固定做法版本已启用，用于后续批次；未开启工作开关或创建任务。');
  }
  return <div className={styles.group}><h3>{task.label} · {writingTrial ? '试跑与启用' : '启用做法'}</h3><p>{TASK_STATE[task.lifecycle] ?? '接入状态未知'}。先在自动化与额度区保存模型、资源和候选做法版本。</p>
    <div className={styles.grid}><Field label="候选配置"><select value={candidateRef} onChange={event => { setCandidateRef(event.target.value); setComparisonRef(''); setPosition(0); }}><option value="">请选择</option>{configs.map(c => <option key={c.taskConfigRef} value={c.taskConfigRef}>{configLabel(c.taskConfigRef)}</option>)}</select></Field>
      {writingTrial ? <Field label="固定旧配置"><select value={baselineRef} onChange={event => setBaselineRef(event.target.value)}><option value="">首版，无旧配置</option>{configs.map(c => <option key={c.taskConfigRef} value={c.taskConfigRef}>{configLabel(c.taskConfigRef)}</option>)}</select></Field> : null}</div>
    {writingTrial ? <><form className={styles.form} onSubmit={event => void createTrial(event)}><Field label="试跑样本"><select value={previewRef} required onChange={event => setPreviewRef(event.target.value)}><option value="">请选择合格写作预览</option>{samples.data?.samples.map(s => <option key={s.previewRef} value={s.previewRef}>{s.title || '未命名作品'} · {s.action} · {s.createdAt}</option>)}</select></Field>
      <p><small>{samples.isError ? '样本读取失败。' : samples.isPending ? '正在读取样本…' : samples.data.state === 'not_connected' ? '写作预览选择尚未接入，暂不能选择试跑样本。' : `仅检查最近 ${samples.data.limit} 份仍对应当前作品版本的预览；${samples.data.samples.length} 份通过当前来源检查。未列出不表示库中不存在。`}</small></p>
      <div className={styles.impact}><p>试跑会分别运行所选固定配置，可能产生两份模型消耗；仍需当前开关、额度、材料和地址许可。未接入工作暂不能执行试跑。</p><ActionButton variant="primary" type="submit" disabled={action.busy !== null || task.lifecycle !== 'active' || !candidateRef || !previewRef}>提交固定样本试跑</ActionButton></div>
    </form>
    <Field label="查看试跑比较"><select value={comparisonRef} onChange={event => { setComparisonRef(event.target.value); setPosition(0); }}><option value="">请选择</option>{comparisons.map(c => <option key={c.comparisonRef} value={c.comparisonRef}>{c.createdAt} · {BATCH_STATE[c.candidateState] ?? c.candidateState} · {c.itemCount} 篇</option>)}</select></Field>
    {comparisonRef ? <div className={styles.group}><ActionButton onClick={() => void refreshComparison()} disabled={comparison.isFetching}>刷新比较回执</ActionButton>{comparison.isError ? <p role="alert">比较读取失败。</p> : comparison.isFetching ? <p>正在重核本次材料资格，产出暂不展示…</p> : comparison.data ? <><p>{comparison.data.qualifiesForActivation ? '固定试跑已完成，具备版本比较回执。' : '固定试跑尚未完成合格比较。'} 本次材料读取：{SOURCE_QUALIFICATION[comparison.data.sourceQualification] ?? '当前资格未确认，产出已隐藏'}</p><div className={styles.grid}>{[comparison.data.baseline, comparison.data.candidate].map((side, i) => <section key={i}><h3>{i === 0 ? '固定旧版本' : '候选版本'}</h3>{side ? <><p>{BATCH_STATE[side.itemState] ?? side.itemState} · 尝试 {side.attemptCount} 次 · {side.failureCode ?? '未记录失败码'}</p><OutputRead output={comparison.data.sourceQualification === 'readable' ? side.output : null} /><details><summary>运行版本</summary><Reference value={side.taskConfigRef} /><p>{side.executionContract}</p><Reference value={side.skillVersionRef} /></details></> : <p>首版，无旧版本。</p>}</section>)}</div><div className={styles.actions}><ActionButton disabled={position === 0} onClick={() => setPosition(p => p - 1)}>上一篇</ActionButton><span>第 {position + 1} / {comparison.data.itemCount} 篇</span><ActionButton disabled={position + 1 >= comparison.data.itemCount} onClick={() => setPosition(p => p + 1)}>下一篇</ActionButton></div></> : <p>正在读取比较…</p>}</div> : null}</> : null}
    <section className={styles.impact}><h3>启用候选做法</h3><p>直接切换只改变做法，保留当前模型、提示词和额度；已运行和进行中的批次保留原配置。{writingTrial ? '选择试跑比较时会启用试跑的完整配置。' : ''}</p>
      <Field label="启用理由"><input value={reason} maxLength={300} required onChange={event => setReason(event.target.value)} /></Field><ActionButton onClick={activate} disabled={action.busy !== null || !candidateRef || !reason.trim()}>启用候选做法</ActionButton>
    </section>
  </div>;
}
export function AiSkillSection({ data, action, linkBase = '?section=skills&task=' }: { data: AiSettings; action: AiAction; linkBase?: string }) {
  const [query] = useSearchParams(); const versionRef = query.get('version');
  const task = data.tasks.find(t => t.taskKey === query.get('task') && t.skillAllowed) ?? data.tasks.find(t => t.taskKey === 'writing_legacy');
  return <section className={styles.section} aria-labelledby="skills-title"><h2 id="skills-title">做法</h2><SkillCreate action={action} /><SkillImport data={data} action={action} />
    <div className={styles.group}><h3>已导入版本</h3>{data.skills.length ? <nav className={styles.selector} aria-label="做法版本">{data.skills.flatMap(skill => skill.versions.map(version => <Link key={version.versionRef} to={linkBase.startsWith('?view=') ? '?view=advanced&area=skills&version=' + encodeURIComponent(version.versionRef) + (task ? '&task=' + encodeURIComponent(task.taskKey) : '') : `?section=skills&version=${version.versionRef}${task ? `&task=${task.taskKey}` : ''}`} aria-current={version.versionRef === versionRef ? 'page' : undefined}>{skill.name} · v{version.versionNo}<small><Moment value={version.createdAt} /></small></Link>))}</nav> : <p>尚未导入做法。导入完成后再保存候选配置。</p>}{versionRef ? <VersionRead reference={versionRef} data={data} action={action} /> : null}</div>
    <nav className={styles.selector} aria-label="做法绑定的工作">{data.tasks.filter(t => t.skillAllowed).map(t => <Link key={t.taskKey} to={linkBase + encodeURIComponent(t.taskKey)} aria-current={task?.taskKey === t.taskKey ? 'page' : undefined}>{t.label}</Link>)}</nav>
    {task && ['ocr_recognition', 'writing_legacy', 'work_comparison', 'comment_study', 'comment_problem_resolution', 'comment_problem_pair_review', 'topic_research', 'creator_research', 'creator_account_research', 'topic_research_extract_review', 'topic_research_resolve', 'topic_research_resolve_review'].includes(task.taskKey) ? <PromptPanel key={task.taskKey} task={task} action={action} /> : null}
    {task ? <SkillSwitch key={`${task.taskKey}:${task.bindingRevision}`} data={data} task={task} action={action} /> : null}
    {task?.taskKey === 'writing_legacy' ? <TrialPanel key={`${task.taskKey}:${task.revision}`} data={data} task={task} action={action} /> : null}
  </section>;
}
