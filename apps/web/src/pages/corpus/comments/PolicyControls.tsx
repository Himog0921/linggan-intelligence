import { useRef, useState } from 'react';
import { bool, decode, str } from '../../../shared/decode';
import { errorText } from './format';
import { parsePolicyDetail, type StudyPolicy, type StudySetup } from './methodDtos';
import { PolicySnapshot } from './PolicySnapshot';
import { studyPost, useStudyRead } from './studyApi';
import { c } from './styles';


const instructionExtra = (instruction: string | null | undefined) => instruction?.match(/<stage-instructions>\n([\s\S]*?)\n<\/stage-instructions>$/)?.[1] ?? null;
interface EditorDraft { modelConfigRef: string; methodName: string; commentBudget: string; contextCharacterBudget: string; semantic: string; resolution: string; pair: string }
function PolicyEditor({ domain, setup, source, onClose, onSaved }: { domain: string; setup: StudySetup; source: StudyPolicy | null; onClose: () => void; onSaved: (ref: string) => Promise<void> }) {
  const query = useStudyRead(domain, `policies/${encodeURIComponent(source?.policyRef ?? '')}`, parsePolicyDetail, Boolean(source));
  if (source && query.isPending) return <p id="policy-status" className={c('study-dialog-note')} role="status">正在读取所选方法版本…</p>;
  if (source && query.isError) return <><p id="policy-status" className={c('study-dialog-note')} role="status">无法打开方法编辑：{errorText(query.error)}</p><button type="button" onClick={onClose}>取消</button></>;
  const snapshot = source ? query.data : null;
  if (source && (!snapshot?.methodManifest || (['semantic', 'resolution', 'pair'] as const).some(key => instructionExtra(snapshot.methodManifest?.stages[key]?.systemInstruction) == null))) return <><p id="policy-status" className={c('study-dialog-note')} role="status">无法打开方法编辑：无法安全还原此版本的补充说明；原版本未更改</p><button type="button" onClick={onClose}>取消</button></>;
  return <PolicyEditorForm key={source?.policyRef || 'new'} domain={domain} setup={setup} snapshot={snapshot ?? null} onClose={onClose} onSaved={onSaved} />;
}
function PolicyEditorForm({ domain, setup, snapshot, onClose, onSaved }: { domain: string; setup: StudySetup; snapshot: StudyPolicy | null; onClose: () => void; onSaved: (ref: string) => Promise<void> }) {
  const [draft, setDraft] = useState<EditorDraft>(() => ({ modelConfigRef: setup.modelConfigs.some(model => model.configRef === snapshot?.methodManifest?.modelConfigRef) ? snapshot!.methodManifest!.modelConfigRef! : setup.modelConfigs[0]?.configRef ?? '', methodName: snapshot?.methodName || '评论研究方法', commentBudget: String(snapshot?.defaults?.commentBudget || 100), contextCharacterBudget: String(snapshot?.defaults?.contextCharacterBudget || 6000), semantic: instructionExtra(snapshot?.methodManifest?.stages.semantic?.systemInstruction) ?? '', resolution: instructionExtra(snapshot?.methodManifest?.stages.resolution?.systemInstruction) ?? '', pair: instructionExtra(snapshot?.methodManifest?.stages.pair?.systemInstruction) ?? '' }));
  const [status, setStatus] = useState(snapshot ? '编辑副本已载入；保存会生成新的不可变方法版本，原版本与已运行研究保持不变。' : '创建第一版研究方法。');
  const [busy, setBusy] = useState(false), [unconfirmed, setUnconfirmed] = useState(false);
  const guard = useRef(false);
  const field = (name: keyof EditorDraft, value: string) => setDraft(current => ({ ...current, [name]: value }));
  async function save() {
    if (guard.current || unconfirmed) return;
    guard.current = true; setBusy(true);
    let saved = false;
    try {
      const policy = await studyPost(domain, 'policies', { domainRef: domain, methodName: draft.methodName, parentPolicyRef: snapshot?.policyRef ?? null, modelConfigRef: draft.modelConfigRef, defaults: { commentBudget: Number(draft.commentBudget), contextCharacterBudget: Number(draft.contextCharacterBudget) }, stageInstructions: { semantic: draft.semantic, resolution: draft.resolution, pair: draft.pair } }, parsePolicyDetail);
      saved = true; await onSaved(policy.policyRef);
    } catch (error) {
      const message = errorText(error);
      setStatus(saved ? `方法已保存，但方法目录刷新失败：${message}` : `未保存方法版本：${message}`);
      // This API has no requestRef. An absent receipt must be resolved by rereading the directory.
      if (!saved && error instanceof Error && 'answered' in error && !error.answered) setUnconfirmed(true);
    } finally { guard.current = false; setBusy(false); }
  }
  return <><form id="policy-form" className={c('policy-form')} onSubmit={event => { event.preventDefault(); void save(); }}>
    <label>模型配置<select id="model-config" required disabled={busy || unconfirmed} value={draft.modelConfigRef} onChange={event => field('modelConfigRef', event.target.value)}>{setup.modelConfigs.map(model => <option key={model.configRef} value={model.configRef}>{model.modelId} · 输入上限 {model.inputTokenLimit} 词元／输出上限 {model.outputTokenLimit} 词元</option>)}</select></label>
    <label>方法名称<input id="method-name" type="text" maxLength={100} required value={draft.methodName} disabled={busy || unconfirmed} onChange={event => field('methodName', event.target.value)} /></label>
    <label>方法默认最多评论数<input id="comment-budget" type="number" min={1} max={3000} required value={draft.commentBudget} disabled={busy || unconfirmed} onChange={event => field('commentBudget', event.target.value)} /></label>
    <label>方法默认单作品语境字符上限<input id="context-character-budget" type="number" min={1} max={20000} required value={draft.contextCharacterBudget} disabled={busy || unconfirmed} onChange={event => field('contextCharacterBudget', event.target.value)} /></label>
    {([['semantic', '语义提取补充说明'], ['resolution', '归并判断补充说明'], ['pair', '问题配对补充说明']] as const).map(([key, title]) => <label className={c('study-instruction')} key={key}>{title}<textarea id={`stage-${key}`} maxLength={32000} rows={3} value={draft[key]} disabled={busy || unconfirmed} onChange={event => field(key, event.target.value)} /></label>)}
    <div className={c('study-policy-actions')}><button id="save-policy" type="submit" disabled={busy || unconfirmed || !draft.modelConfigRef}>保存新版本</button><button id="cancel-policy-edit" type="button" disabled={busy} onClick={onClose}>取消</button></div>
  </form><p id="policy-status" className={c('study-dialog-note')} role="status">{status}{unconfirmed ? ' 保存结果未确认，请取消编辑并刷新方法目录后核对。' : ''}</p></>;
}

function PolicyViewer({ domain, policy, policies, close }: { domain: string; policy: StudyPolicy; policies: StudyPolicy[]; close: () => void }) {
  const [compare, setCompare] = useState('');
  const current = useStudyRead(domain, `policies/${encodeURIComponent(policy.policyRef)}`, parsePolicyDetail);
  const other = useStudyRead(domain, `policies/${encodeURIComponent(compare)}`, parsePolicyDetail, Boolean(compare));
  return <section id="policy-viewer" className={c('study-policy-viewer')} aria-label="方法版本详情"><div className={c('study-policy-viewer-toolbar')}><label>并排对比其他版本<select id="compare-policy" value={compare} disabled={!current.data} onChange={event => setCompare(event.target.value)}><option value="">不对比</option>{policies.filter(item => item.policyRef !== policy.policyRef).map(item => <option value={item.policyRef} key={item.policyRef}>{item.methodName || '未命名方法'} · {item.policyRef.slice(0, 8)}</option>)}</select></label><button id="close-policy-viewer" type="button" onClick={close}>收起方法详情</button></div><p id="policy-viewer-status" role="status">{current.isPending ? '正在读取完整方法版本…' : current.isError ? `方法详情读取失败：${errorText(current.error)}` : compare ? other.isPending ? '正在读取对比版本…' : other.isError ? `对比版本读取失败：${errorText(other.error)}` : '两版完整方法并排显示；旧版本不会被修改。' : '当前方法为不可变版本；复制并编辑会保存为新版本。'}</p><div id="policy-viewer-content" className={c('study-policy-compare')}>{current.data ? <PolicySnapshot policy={current.data} /> : null}{compare && other.data ? <div data-policy-comparison><PolicySnapshot policy={other.data} /></div> : null}</div></section>;
}

export function PolicyControls({ domain, setup, policies, selected, onSelect, reload }: { domain: string; setup: StudySetup; policies: StudyPolicy[]; selected: string; onSelect: (ref: string, policy?: StudyPolicy) => void; reload: () => Promise<StudyPolicy[]> }) {
  const recorded = policies.filter(item => item.recordingState === 'recorded'), current = recorded.find(item => item.policyRef === selected) ?? null;
  const activeRef = policies.find(item => item.isActive)?.policyRef ?? null;
  const available = setup.domainStatus === 'active' && setup.modelConfigs.length > 0;
  const [viewer, setViewer] = useState(false), [editing, setEditing] = useState(() => available && !recorded.length), [busy, setBusy] = useState(false), [status, setStatus] = useState('');
  const guard = useRef(false);
  async function activate() {
    if (guard.current || !current) return;
    guard.current = true; setBusy(true);
    let confirmed = false;
    try {
      const receipt = await studyPost(domain, `policies/${encodeURIComponent(current.policyRef)}/activate?domain=${encodeURIComponent(domain)}`, { expectedActivePolicyRef: activeRef }, value => decode(value, row => ({ domainRef: str(row, 'domainRef'), policyRef: str(row, 'policyRef'), isActive: bool(row, 'isActive') })));
      if (receipt.domainRef !== domain || receipt.policyRef !== current.policyRef || !receipt.isActive) throw new Error('默认方法回执与本次选择不一致。');
      confirmed = true; setStatus('已将所选方法设为当前领域默认版本。'); await reload();
    } catch (error) { setStatus(confirmed ? `默认方法已按回执切换，但目录刷新失败：${errorText(error)}` : `切换默认方法未获确认：${errorText(error)}。请刷新目录后核对。`); }
    finally { guard.current = false; setBusy(false); }
  }
  return <>
    <div className={c('study-policy-current')}><label>本次使用的方法版本<select id="study-policy" required value={selected} disabled={editing || !recorded.length} onChange={event => { onSelect(event.target.value, recorded.find(item => item.policyRef === event.target.value)); setViewer(false); }}>{recorded.length ? recorded.map(item => <option key={item.policyRef} value={item.policyRef}>{item.methodName || '未命名方法'}{item.isActive ? ' · 默认' : ''} · {item.policyRef.slice(0, 8)}</option>) : <option value="">没有已记录的方法版本</option>}</select></label><div className={c('study-policy-current-actions')}><button id="view-policy" type="button" disabled={!current} onClick={() => setViewer(true)}>查看方法</button><button id="edit-policy" type="button" disabled={!available || busy || editing} onClick={() => { setEditing(true); setStatus(''); }}>{current ? '复制并编辑' : '新建方法'}</button><button id="activate-policy" type="button" disabled={!current || current.policyRef === activeRef || busy} onClick={() => { void activate(); }}>设为默认方法</button></div></div>
    <p id="selected-policy-summary" className={c('study-dialog-note')} role="status">{current ? `${current.methodName || '未命名方法'} · 版本 ${current.policyRef.slice(0, 8)} · 评论上限 ${Number(current.defaults?.commentBudget || 0)} 条 · 语境上限 ${Number(current.defaults?.contextCharacterBudget || 0)} 字符${current.isActive ? ' · 当前默认' : ''}` : '还没有已保存的方法版本。首次使用前需要创建一版研究方法。'}</p>
    {viewer && current ? <PolicyViewer key={current.policyRef} domain={domain} policy={current} policies={recorded} close={() => setViewer(false)} /> : null}
    {editing ? <PolicyEditor domain={domain} setup={setup} source={current} onClose={() => setEditing(false)} onSaved={async ref => { const rows = await reload(); onSelect(ref, rows.find(item => item.policyRef === ref)); setEditing(false); setStatus(`已保存并选中新方法版本：${ref}。可直接用于本次预览和启动。`); }} /> : <p id="policy-status" className={c('study-dialog-note')} role="status">{status}</p>}
  </>;
}
