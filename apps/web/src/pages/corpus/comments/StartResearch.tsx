import { useEffect, useLayoutEffect, useRef, useState } from 'react';
import { decode, obj, optNum, optStr, str, type Json } from '../../../shared/decode';
import { errorText, num } from './format';
import { selectionExclusionLabels, studyModeLabel } from './labels';
import { countMap, parseSetup, type StudyPolicy, type StudySetup } from './methodDtos';
import { PolicyControls } from './PolicyControls';
import { usePolicies } from './usePolicies';
import { StudyDialog } from './StudyDialog';
import { studyPost, useStudyRead, type CommentKey } from './studyApi';
import { SourceSummary, WorkPicker, type WorkRole } from './WorkPicker';
import { c } from './styles';

export interface SelectedComment { commentKey: CommentKey; observationRole: string }
export interface SelectionCommand {
  domainRef: string; policyRef: string;
  scope: { kind: 'works'; workRefs: string[] } | { kind: 'comments'; commentKeys: CommentKey[] };
  workRoles: WorkRole[]; mode: string; limits: { commentBudget: number; contextCharacterBudget: number; tokenLimit: number };
}
interface SelectionReceipt { targetCount: number | null; requestedWorkCount: number | null; coveredWorkCount: number | null; scopeCommentCount: number | null; exclusionCounts: Record<string, number | null> }
const parseSelection = (row: Json): SelectionReceipt => decode(row, r => ({ targetCount: optNum(r, 'targetCount'), requestedWorkCount: optNum(r, 'requestedWorkCount'), coveredWorkCount: optNum(r, 'coveredWorkCount'), scopeCommentCount: optNum(r, 'scopeCommentCount'), exclusionCounts: countMap(obj(r, 'exclusionCounts')) }));
const parseStart = (row: Json) => decode(row, r => ({ requestRef: str(r, 'requestRef'), outcome: str(r, 'outcome'), runRef: optStr(r, 'runRef'), requestedWorkCount: optNum(r, 'requestedWorkCount'), coveredWorkCount: optNum(r, 'coveredWorkCount'), targetCount: optNum(r, 'targetCount') }));

function exclusionSummary(counts: Record<string, number | null>, mode: string) {
  const labels = { ...selectionExclusionLabels };
  if (mode === 'new_only') labels.notSelectedByMode = '已有研究记录';
  const rows = Object.entries(labels).filter(([key]) => Number(counts[key]) > 0).map(([key, title]) => `${title} ${num(counts[key])} 条`);
  return rows.length ? rows.join('；') : '没有被排除的评论';
}

interface Props { onStatus?: (value: { text: string; kind: string }) => void; domain: string; domainName: string; open: boolean; kind: 'works' | 'comments'; opening: number; selectedComments: SelectedComment[]; selectedWorks: WorkRole[]; onSelectedWorks: (rows: WorkRole[]) => void; onClose: () => void; onCreated: (runRef: string, feedback: string) => void }
export function StartResearch(props: Props) {
  const setup = useStudyRead(props.domain, 'setup', parseSetup, props.open);
  const policies = usePolicies(props.domain, props.open);
  const { open, onStatus, domainName } = props;
  useEffect(() => { if (!open) return;
    if (setup.isPending) onStatus?.({ text: '正在读取研究准备信息…', kind: 'info' });
    else if (setup.isError) onStatus?.({ text: `无法读取准备信息：${errorText(setup.error)}`, kind: 'error' });
    else if (setup.data?.domainStatus === 'paused') onStatus?.({ text: `${domainName}已暂停；历史结果可读，不能创建新策略或运行。`, kind: 'error' });
    else if (setup.data && !setup.data.modelConfigs.length) onStatus?.({ text: '没有启用的模型配置，无法保存策略。', kind: 'error' });
  }, [open, onStatus, domainName, setup.isPending, setup.isError, setup.error, setup.data]);

  if (!setup.data || !policies.data) return props.open ? <StudyDialog id="study-dialog" titleId="study-dialog-title" onClose={props.onClose}><header className={c('study-dialog-header')}><div><p className={c('study-label')}>受控启动</p><h2 id="study-dialog-title">发起一次新研究</h2></div><button type="button" id="study-dialog-close" onClick={props.onClose}>关闭</button></header><p role="status">{setup.isError ? `无法读取准备信息：${errorText(setup.error)}` : policies.isError ? `研究方法列表加载失败：${errorText(policies.error)}` : '正在读取研究准备信息…'}</p>{setup.isError || policies.isError ? <button type="button" onClick={() => { void setup.refetch(); void policies.refetch(); }}>重试</button> : null}</StudyDialog> : null;
  return <ResearchSession {...props} setup={setup.data} policies={policies.data} reload={async () => { const result = await policies.refetch(); if (!result.data || result.error) throw result.error ?? new Error('方法目录未取得'); return result.data; }} />;
}

function ResearchSession({ domain, domainName, onStatus, open, kind, opening, selectedComments, selectedWorks, onSelectedWorks, onClose, onCreated, setup, policies, reload }: Props & { setup: StudySetup; policies: StudyPolicy[]; reload: () => Promise<StudyPolicy[]> }) {
  const recorded = policies.filter(policy => policy.recordingState === 'recorded');
  const [policyRef, setPolicyRef] = useState(() => recorded.find(policy => policy.isActive)?.policyRef ?? recorded[0]?.policyRef ?? '');
  const initial = recorded.find(policy => policy.policyRef === policyRef);
  const [limits, setLimits] = useState(() => ({ commentBudget: String(initial?.defaults?.commentBudget || 100), contextCharacterBudget: String(initial?.defaults?.contextCharacterBudget || 6000), tokenLimit: '100000' }));
  const [mode, setMode] = useState('new_only'), [reason, setReason] = useState('');
  const [preview, setPreview] = useState<{ signature: string; receipt: SelectionReceipt; opening: number } | null>(null), [previewText, setPreviewTextValue] = useState<{ opening: number; text: string } | null>(null), [result, setResultValue] = useState<{ opening: number; text: string } | null>(null);
  const [previewBusy, setPreviewBusy] = useState(false), [startBusy, setStartBusy] = useState(false);
  const setPreviewText = (text: string) => setPreviewTextValue({ opening, text });
  const setResult = (text: string) => setResultValue({ opening, text });
  const [uncertain, setUncertain] = useState<{ signature: string; requestRef: string } | null>(null);
  const previewSerial = useRef(0), startGuard = useRef(false), pendingStart = useRef<{ signature: string; requestRef: string } | null>(null);
  const alive = useRef(true);
  useEffect(() => { alive.current = true; return () => { alive.current = false; previewSerial.current += 1; }; }, []);
  const roles = kind === 'comments' ? [...new Map(selectedComments.map(comment => [comment.commentKey.workRef, comment.observationRole])).entries()].map(([contentPublicRef, observationRole]) => ({ contentPublicRef, observationRole })) : selectedWorks;
  const command: SelectionCommand = { domainRef: domain, policyRef, scope: kind === 'comments' ? { kind: 'comments', commentKeys: selectedComments.map(comment => comment.commentKey) } : { kind: 'works', workRefs: roles.map(role => role.contentPublicRef) }, workRoles: roles, mode, limits: { commentBudget: Number(limits.commentBudget), contextCharacterBudget: Number(limits.contextCharacterBudget), tokenLimit: Number(limits.tokenLimit) } };
  const signature = JSON.stringify(command), currentSignature = useRef(signature);
  // Event handlers compare against the latest committed user intent, not a captured input snapshot.
  const commandRef = useRef(command);
  useLayoutEffect(() => { currentSignature.current = signature; commandRef.current = command; });
  const available = setup.domainStatus === 'active' && setup.modelConfigs.length > 0;
  const count = kind === 'comments' ? selectedComments.length : selectedWorks.length;
  const currentPreview = preview?.signature === signature && preview.opening === opening;
  const retryPending = uncertain?.signature === JSON.stringify({ ...command, reason: mode === 'new_only' ? null : reason });
  async function previewSelection() {
    if (previewBusy || !count || !policyRef) return;
    const serial = ++previewSerial.current, intent = commandRef.current, intentSignature = JSON.stringify(intent);
    setPreviewBusy(true);
    try {
      const receipt = await studyPost(domain, 'selection-preview', intent, parseSelection);
      if (serial !== previewSerial.current) return;
      if (intentSignature !== currentSignature.current) { setPreviewText('预览期间选择已变化，请重新预览。'); return; }
      setPreview({ signature: intentSignature, receipt, opening });
      setPreviewText(`按“${studyModeLabel[intent.mode] || '本次模式'}”预览：选中 ${num(receipt.requestedWorkCount)} 篇作品，范围内 ${num(receipt.scopeCommentCount)} 条评论；排除：${exclusionSummary(receipt.exclusionCounts, intent.mode)}。预计冻结 ${num(receipt.targetCount)} 条，实际覆盖 ${num(receipt.coveredWorkCount)} 篇；本次上限 ${num(intent.limits.commentBudget)} 条。预览只读，提交时会重新冻结，以创建回执为准。`);
    } catch (error) { if (serial === previewSerial.current) { setPreview(null); setPreviewText(`预览失败：${errorText(error)}`); } }
    finally { if (serial === previewSerial.current) setPreviewBusy(false); }
  }
  async function start() {
    if (startGuard.current || (!retryPending && (!currentPreview || !(Number(preview?.receipt.targetCount) > 0)))) return;
    startGuard.current = true; setStartBusy(true);
    const intent = { ...commandRef.current, reason: mode === 'new_only' ? null : reason }, intentSignature = JSON.stringify(intent);
    if (pendingStart.current?.signature !== intentSignature) pendingStart.current = { signature: intentSignature, requestRef: crypto.randomUUID() };
    const requestRef = pendingStart.current.requestRef;
    try {
      const receipt = await studyPost(domain, 'runs', { ...intent, requestRef }, parseStart);
      if (receipt.requestRef !== requestRef || !['created', 'index_pending', 'no_work'].includes(receipt.outcome) || (receipt.outcome === 'created' ? !receipt.runRef || !(Number(receipt.targetCount) > 0) : receipt.runRef != null || receipt.targetCount !== 0)) throw new Error('启动回执与本次请求不一致，结果未确认；重试沿用原请求编号。');
      if (!alive.current) return;
      pendingStart.current = null; setUncertain(null); setPreview(null); setPreviewText('');
      const feedback = receipt.outcome === 'created' ? `已创建 ${receipt.runRef}：选中 ${Number(receipt.requestedWorkCount || 0)} 篇，覆盖 ${Number(receipt.coveredWorkCount || 0)} 篇，冻结 ${Number(receipt.targetCount || 0)} 条目标评论。已进入模型执行队列。` : `本次未创建空运行：${receipt.outcome === 'no_work' ? '没有符合所选模式的新评论' : '仍有评论等待索引'}。 requestRef=${receipt.requestRef}`;
      setResult(feedback); if (receipt.runRef) onCreated(receipt.runRef, feedback);
    } catch (error) { if (alive.current) { setUncertain(pendingStart.current); setResult(`创建结果未获确认：${errorText(error)}。重试将沿用本次请求编号。`); } }
    finally { startGuard.current = false; if (alive.current) setStartBusy(false); }
  }
  return <StudyDialog id="study-dialog" titleId="study-dialog-title" onClose={onClose} open={open}><header className={c('study-dialog-header')}><div><p className={c('study-label')}>受控启动</p><h2 id="study-dialog-title">{kind === 'comments' ? '研究所选评论' : '发起一次新研究'}</h2></div><button type="button" id="study-dialog-close" onClick={onClose}>关闭</button></header>
    <WorkPicker domain={domain} setup={setup} kind={kind} selectedComments={selectedComments.length} selected={selectedWorks} onSelected={onSelectedWorks} commentBudget={Number(limits.commentBudget)} onCatalogLoaded={(count, failed) => { if (available) onStatus?.({ kind: 'info', text: failed ? '准备信息已加载；作品目录暂时不可用，请在下方重试。' : `已加载 ${count ?? 0} 篇当前领域作品；可切换查看参考作品。` }); }} />
    <section className={c('study-command')} aria-labelledby="setup-title"><h3 id="setup-title" className="v7-sr-only">研究策略</h3><PolicyControls domain={domain} setup={setup} policies={policies} selected={policyRef} reload={reload} onSelect={(ref, snapshot) => { setPolicyRef(ref); const chosen = snapshot ?? policies.find(policy => policy.policyRef === ref); setLimits(old => ({ ...old, commentBudget: String(chosen?.defaults?.commentBudget || 100), contextCharacterBudget: String(chosen?.defaults?.contextCharacterBudget || 6000) })); }} />
      <details className={c('study-catalog-context')}><summary>查看领域评论与来源资格</summary><SourceSummary id="source-preview" role="primary" preview={setup.sourcePreview} /></details>
      {!available ? <p className={c('study-dialog-note')} role="status">{setup.domainStatus === 'paused' ? `${domainName}已暂停；历史结果可读，不能创建新策略或运行。` : '没有启用的模型配置，无法保存策略。'}</p> : null}
    </section>
    <section className={c('study-run-confirm')} aria-label="预览并开始研究"><div className={c('study-run-settings')}><label>启动模式<select id="study-mode" value={mode} disabled={startBusy} onChange={event => setMode(event.target.value)}>{Object.entries(studyModeLabel).map(([value, title]) => <option value={value} key={value}>{title}</option>)}</select></label>
      {([['commentBudget', 'run-comment-budget', '本次最多评论数', 1, 3000], ['contextCharacterBudget', 'run-context-character-budget', '单作品语境字符上限', 1, 20000], ['tokenLimit', 'token-budget', '本次模型词元上限', 1024, 10000000]] as const).map(([key, id, title, min, max]) => <label key={key}>{title}<input id={id} type="number" min={min} max={max} value={limits[key]} disabled={startBusy} onChange={event => setLimits(old => ({ ...old, [key]: event.target.value }))} /></label>)}
      <label id="study-reason-label" hidden={mode === 'new_only'}>本次重研原因<input id="study-reason" type="text" maxLength={500} placeholder="说明这次重研的原因" value={reason} disabled={startBusy} onChange={event => setReason(event.target.value)} /></label><button id="preview-run" type="button" disabled={!available || !count || !policyRef || previewBusy || startBusy} onClick={() => { void previewSelection(); }}>预览本次选择</button>
    </div><p id="selection-preview" className={c('study-table-status')} role="status">{preview?.opening === opening && !currentPreview ? '选择、模式、方法或预算已变化，请重新预览。' : previewText?.opening === opening ? previewText.text : ''}</p><footer className={c('study-workspace-footer')}><p id="selection-scope-note">{kind === 'comments' ? '服务端会核对所选评论的资格与预算。确认开始后，目标进入模型执行队列，可能产生调用与费用。' : '服务端会按所选作品、当前模式和预算冻结目标。确认开始后，目标进入模型执行队列，可能产生调用与费用。'}</p><div className={c('run-actions')}><button id="start-run" type="button" disabled={!available || (!retryPending && (!currentPreview || !(Number(preview?.receipt.targetCount) > 0))) || startBusy} onClick={() => { void start(); }}>{retryPending ? '核对并重试原启动请求' : '确认并开始研究'}</button><p id="run-result" role="status">{result?.opening === opening ? result.text : retryPending ? '上次启动回执尚未确认；核对重试会沿用原请求编号，不是另发一次研究意图。' : ''}</p></div></footer></section>
  </StudyDialog>;
}
