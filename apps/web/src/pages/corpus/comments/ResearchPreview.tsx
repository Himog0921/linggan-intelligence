import { useEffect, useRef, useState } from 'react';
import { ActionButton } from '../../../shared/ActionButton';
import { CrIcon } from './CrIcon';
import { num, shortRef } from './format';
import { StudyDialog } from './StudyDialog';
import type { SelectedComment, StudyNavigation } from './studyApi';
import { commandError, planL1, previewSelection, startSelection, submitL1, uncertain, why, type PreviewPlan } from './understandingApi';
import { c } from './styles';

type L1Intent = { plan: Awaited<ReturnType<typeof planL1>>; commandRef: string };
const workName = (item: SelectedComment) => item.workTitle || `作品 ${shortRef(item.commentKey.workRef)}`;
/** 逐条排除原因（CONTRACT 第二轮 §6）；其余沿用现有排除原因码的中文。 */
const EXCLUDED_REASON: Record<string, string> = { waiting_l1: '作品语境未就绪', already_queued: '正在研究，不会重复提交', already_researched: '已有研究结果，本次跳过' };
const excludedReason = (code: string) => EXCLUDED_REASON[code] ?? why(code);
type GroupRow = NonNullable<PreviewPlan['groups']>[number];
function groupState(group: GroupRow): { label: string; tone: string } {
  if (group.waitingL1) return { label: '待作品语境', tone: 'wait' };
  if (group.researchable > 0) return { label: group.active ? `可进入研究 · ${group.active} 条进行中` : '可进入研究', tone: 'good' };
  if (group.active) return { label: `${group.active} 条进行中`, tone: 'running' };
  return { label: '本次不提交', tone: '' };
}

/**
 * B-04 研究预览。服务返回 groups / excludedTargets（CONTRACT 第二轮 §6）时按逐作品计划与逐条排除显示；
 * 旧服务缺失这些字段时降级：按所选评论自身分组，待作品语境逐条列出（waitingL1.workRefs），其余原因按计数列出。
 */
export function ResearchPreview({ domain, selected, initialRerun, nav, close, submitted }: {
  domain: string; selected: SelectedComment[]; initialRerun: boolean; nav: StudyNavigation;
  close: () => void; submitted: (batchRef: string, message: string) => void;
}) {
  const [rerun, setRerun] = useState(initialRerun);
  const [plan, setPlan] = useState<PreviewPlan | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState('');
  const [busy, setBusy] = useState(false);
  const [notice, setNotice] = useState('');
  const [unknown, setUnknown] = useState(false);
  const [round, setRound] = useState(0);
  const command = useRef<string | null>(null);
  const l1Intents = useRef(new Map<string, L1Intent>());
  const accepted = useRef(new Set<string>());
  const [acceptedWorks, setAcceptedWorks] = useState<string[]>([]);

  useEffect(() => {
    let live = true;
    // eslint-disable-next-line react-hooks/set-state-in-effect -- a new preview request resets the visible plan
    setLoading(true); setError(''); setPlan(null); command.current = null;
    previewSelection(domain, selected, rerun).then(next => { if (live) setPlan(next); }, failure => { if (live) setError(commandError(failure)); })
      .finally(() => { if (live) setLoading(false); });
    return () => { live = false; };
  }, [domain, selected, rerun, round]);

  async function start() {
    if (!plan || busy) return;
    setBusy(true); setNotice(''); command.current ??= crypto.randomUUID();
    try {
      const receipt = await startSelection(domain, selected, rerun, plan, command.current);
      submitted(receipt.batchRef, `已提交 ${receipt.targetCount} 条评论，共 ${receipt.unitCount} 个研究单元。`);
    } catch (failure) {
      const unconfirmed = uncertain(failure);
      setUnknown(unconfirmed);
      if (!unconfirmed) command.current = null;
      setNotice(commandError(failure));
    } finally { setBusy(false); }
  }

  async function supplement(works: string[]) {
    setBusy(true); setNotice('');
    let blocked = 0, unresolved = 0;
    const reasons = new Set<string>();
    for (const work of works) {
      if (accepted.current.has(work)) continue;
      let intent = l1Intents.current.get(work);
      try {
        if (!intent) {
          const next = await planL1(domain, work);
          if (next.domainRef !== domain || next.workRef !== work) throw new Error('L1 回执范围不一致');
          if (!next.canStart) { blocked += 1; next.reasons.forEach(reason => reasons.add(why(reason))); continue; }
          intent = { plan: next, commandRef: crypto.randomUUID() };
          l1Intents.current.set(work, intent);
        }
        await submitL1(intent.plan, intent.commandRef);
        l1Intents.current.delete(work);
        accepted.current.add(work);
      } catch (failure) {
        if (intent && uncertain(failure)) unresolved += 1;
        else { blocked += 1; l1Intents.current.delete(work); reasons.add(commandError(failure)); }
      }
    }
    const confirmed = works.filter(work => accepted.current.has(work)).length;
    setAcceptedWorks([...accepted.current]);
    setNotice([`${confirmed} 个作品已收到作品语境补充回执`, blocked ? `${blocked} 个暂不可提交` : '',
      unresolved ? `${unresolved} 个回执未确认，重试将沿用原请求` : '', ...reasons].filter(Boolean).join('。') + '。语境就绪后可重新预览。');
    setBusy(false);
  }

  const missing = new Set(plan?.missingWorks ?? []);
  const byKey = new Map(selected.map(item => [`${item.commentKey.workRef}\u0000${item.commentKey.commentExternalId}`, item]));
  const titles = new Map<string, string>();
  for (const item of selected) if (item.workTitle && !titles.has(item.commentKey.workRef)) titles.set(item.commentKey.workRef, item.workTitle);
  for (const group of plan?.groups ?? []) if (group.workTitle) titles.set(group.workRef, group.workTitle);
  const titleOf = (workRef: string) => titles.get(workRef) || `作品 ${shortRef(workRef)}`;
  const openActive = <button type="button" className={c('text-action')} onClick={() => { close(); nav.records(); }}>查看进行中</button>;
  const supplementButton = (workRef: string) => <button type="button" className={c('text-action')} disabled={busy || unknown || acceptedWorks.includes(workRef)} onClick={() => void supplement([workRef])}>{acceptedWorks.includes(workRef) ? '已提交补充' : '补充作品语境'}</button>;
  const groups = new Map<string, { title: string; count: number }>();
  for (const item of selected) {
    if (missing.has(item.commentKey.workRef)) continue;
    const group = groups.get(item.commentKey.workRef) ?? { title: workName(item), count: 0 };
    group.count += 1; groups.set(item.commentKey.workRef, group);
  }
  const waitingRows = selected.filter(item => missing.has(item.commentKey.workRef));
  const excludedCount = plan ? Math.max(0, plan.selected - plan.researchable) : 0;
  const targetRows = plan?.excludedTargets ?? null;
  const reasonRows = plan && targetRows ? [
    ...(plan.researched && !rerun ? [{ key: 'researched', label: '已有研究结果，本次跳过', count: plan.researched, action: <button type="button" className={c('text-action')} disabled={busy} onClick={() => setRerun(true)}>改为重新研究</button> }] : []),
    ...(plan.unresolved ? [{ key: 'unresolved', label: '上一次调用结果未确认', count: plan.unresolved, action: null }] : []),
  ] : plan ? [
    ...(plan.active ? [{ key: 'active', label: '正在研究，不会重复提交', count: plan.active, action: <button type="button" className={c('text-action')} onClick={() => { close(); nav.records(); }}>查看进行中</button> }] : []),
    ...(plan.researched && !rerun ? [{ key: 'researched', label: '已有研究结果，本次跳过', count: plan.researched, action: <button type="button" className={c('text-action')} disabled={busy} onClick={() => setRerun(true)}>改为重新研究</button> }] : []),
    ...(plan.unresolved ? [{ key: 'unresolved', label: '上一次调用结果未确认', count: plan.unresolved, action: null }] : []),
    ...plan.exclusions.map(item => ({ key: item.code, label: why(item.code), count: item.count, action: null })),
  ] : [];
  const canStart = Boolean(plan && plan.units > 0 && plan.researchable > 0);

  return <StudyDialog className={c('dialog preview-dialog')} titleId="research-preview-title" onClose={() => { if (!busy) close(); }}>
    <header className={c('dialog-head')}><div><h2 id="research-preview-title">本次研究预览</h2><small>按所属作品分组，共享作品语境</small></div>
      <button type="button" className={c('icon-btn')} aria-label="关闭研究预览" disabled={busy} onClick={close}><CrIcon name="X" /></button></header>
    <div className={c('dialog-body')} aria-live="polite">
      {loading ? <p className={c('view-status')}>正在预览所选 {selected.length} 条评论…</p>
        : error ? <p className={c('view-status')} role="alert">预览未完成：{error} <button type="button" className={c('text-action')} onClick={() => setRound(n => n + 1)}>重新预览</button></p>
        : plan ? <>
          <div className={c('preview-lead')}><strong>将提交 <b>{num(plan.researchable)}</b> 条评论 · 预计调用 <b>{num(plan.calls)}</b> 次</strong>
            <span>已选 {num(plan.selected)} 条{excludedCount ? `，其中 ${num(excludedCount)} 条不会提交` : ''}</span></div>
          <div className={c('preview-summary')}>
            <div><b>{num(plan.researchable)}</b><span>将提交评论</span></div>
            <div title="每个研究单元调用 1 次模型；同一作品的评论合成研究单元"><b>{num(plan.calls)}</b><span>预计调用</span></div>
            <div><b>{num(plan.works)}</b><span>涉及作品</span></div>
            <div><b>{num(excludedCount)}</b><span>不会提交</span></div>
          </div>
          {rerun && plan.researched ? <div className={c('notice info')}>其中 {num(plan.researched)} 条已有研究结果：本次会生成新结果，现有结果保留到新结果完成为止。</div> : null}
          <h3 className={c('preview-h')}>按所属作品分组</h3>
          {plan.groups ? plan.groups.length ? plan.groups.map(group => { const state = groupState(group); return <div key={group.workRef} className={c('plan-group')}>
            <div><strong>{titleOf(group.workRef)}</strong><small>{[`将提交 ${num(group.researchable)} 条`, `${num(group.units)} 个研究单元`,
              group.researched ? `已有结果 ${num(group.researched)} 条` : '', group.selected !== group.researchable ? `已选 ${num(group.selected)} 条` : ''].filter(Boolean).join(' · ')}</small></div>
            <span className={c('plan-group-side')}><span className={c('state', state.tone)}>{state.label}</span>{group.waitingL1 ? supplementButton(group.workRef) : group.active && !group.researchable ? openActive : null}</span>
          </div>; }) : <p className={c('muted preview-none')}>没有可提交的评论。</p>
          : plan.researchable && groups.size ? [...groups].map(([work, group]) => <div key={work} className={c('plan-group')}>
            <div><strong>{group.title}</strong><small>已选 {group.count} 条评论</small></div><span className={c('state good')}>可进入研究</span>
          </div>) : <p className={c('muted preview-none')}>没有可提交的评论。</p>}
          {excludedCount || waitingRows.length || targetRows?.length ? <details className={c('preview-excluded')} open={!plan.units}>
            <summary>不会提交 {num(excludedCount)} 条 <span>查看原因与处理办法</span></summary>
            {reasonRows.length && targetRows ? reasonRows.map(row => <div key={row.key} className={c('excluded-row')}><div><p>{row.label}</p><small>{num(row.count)} 条</small></div>{row.action}</div>) : null}
            {targetRows ? targetRows.map(target => {
              const item = byKey.get(`${target.workRef}\u0000${target.commentExternalId}`), reason = excludedReason(target.reason);
              return <div key={`${target.workRef}\u0000${target.commentExternalId}`} className={c('excluded-row')}>
                <div><p>{item?.text || titleOf(target.workRef)}</p><small>{item?.text ? `${titleOf(target.workRef)} · ${reason}` : reason}</small></div>
                {target.reason === 'waiting_l1' ? supplementButton(target.workRef) : target.reason === 'already_queued' ? openActive : null}
              </div>;
            }) : null}
            {targetRows && plan.excludedTruncated ? <p className={c('excluded-more')}>仅列出前 200 条</p> : null}
            {targetRows ? null : waitingRows.map(item => <div key={item.sourceRef} className={c('excluded-row')}>
              <div><p>{item.text || `评论 ${item.commentKey.commentExternalId}`}</p><small>{workName(item)} · 作品语境未就绪</small></div>
              {supplementButton(item.commentKey.workRef)}
            </div>)}
            {targetRows ? null : reasonRows.map(row => <div key={row.key} className={c('excluded-row')}><div><p>{row.label}</p><small>{num(row.count)} 条</small></div>{row.action}</div>)}
          </details> : null}
          {acceptedWorks.length ? <div className={c('notice info')}>作品语境补充已提交。就绪后 <button type="button" className={c('text-action')} disabled={busy} onClick={() => setRound(n => n + 1)}>重新预览</button></div> : null}
        </> : null}
      {notice ? <p role="status" className={c('notice')}>{notice}</p> : null}
    </div>
    <footer className={c('dialog-footer')}>
      <span className={c('footer-note')}>{canStart ? '提交后可在“研究记录”查看进度，关闭窗口不影响任务' : loading ? '' : '当前没有可提交的研究单元'}</span>
      <div className={c('actions')}>
        <ActionButton disabled={busy} onClick={close}>返回修改</ActionButton>
        <ActionButton variant="ink" disabled={!canStart || busy} onClick={() => void start()}>{busy ? '正在提交…' : unknown ? '重试同一请求' : canStart ? `提交 ${plan!.researchable} 条研究` : '开始研究'}</ActionButton>
      </div>
    </footer>
  </StudyDialog>;
}
