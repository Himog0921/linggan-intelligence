import { useRef, useState } from 'react';
import { ActionButton } from '../../../shared/ActionButton';
import { commandError, planL1, previewSelection, startSelection, submitL1, uncertain, why, type PreviewPlan } from './understandingApi';
import type { SelectedComment, StudyNavigation } from './studyApi';
import { c } from './styles';

type L1Intent = { plan: Awaited<ReturnType<typeof planL1>>; commandRef: string };

export function CommentResearchSelection({ domain, selected, nav, clear }: { domain: string; selected: SelectedComment[]; nav: StudyNavigation; clear?: () => void }) {
  const [plan, setPlan] = useState<PreviewPlan | null>(null);
  const [rerun, setRerun] = useState(false);
  const [busy, setBusy] = useState(false);
  const [notice, setNotice] = useState('');
  const [unknown, setUnknown] = useState(false);
  const [l1Unknown, setL1Unknown] = useState(false);
  const [batch, setBatch] = useState<string | null>(null);
  const command = useRef<string | null>(null);
  const l1Intents = useRef(new Map<string, L1Intent>());
  const accepted = useRef(new Set<string>());

  async function prepare() {
    setBusy(true); setNotice(''); setPlan(null); setBatch(null); command.current = null;
    try { setPlan(await previewSelection(domain, selected, rerun)); }
    catch (error) { setNotice(commandError(error)); }
    finally { setBusy(false); }
  }

  async function start() {
    if (!plan) return;
    setBusy(true); setNotice(''); command.current ??= crypto.randomUUID();
    try {
      const receipt = await startSelection(domain, selected, rerun, plan, command.current);
      setBatch(receipt.batchRef);
      setNotice(`已提交 ${receipt.targetCount} 条评论，共 ${receipt.unitCount} 个研究单元。请在研究记录查看执行进度。`);
      setUnknown(false);
    } catch (error) {
      const unconfirmed = uncertain(error);
      setUnknown(unconfirmed);
      if (!unconfirmed) command.current = null;
      setNotice(commandError(error));
    } finally { setBusy(false); }
  }

  async function supplement() {
    if (!plan) return;
    setBusy(true); setNotice('');
    let blocked = 0, unresolved = 0;
    const reasons = new Set<string>();
    for (const work of plan.missingWorks) {
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
      } catch (error) {
        if (intent && uncertain(error)) { unresolved += 1; }
        else { blocked += 1; l1Intents.current.delete(work); reasons.add(commandError(error)); }
      }
    }
    setL1Unknown(unresolved > 0);
    const confirmed = plan.missingWorks.filter(work => accepted.current.has(work)).length;
    setNotice([
      `${confirmed} 个作品已收到 L1 提交回执`, blocked ? `${blocked} 个暂不可提交` : '',
      unresolved ? `${unresolved} 个回执未确认，重试将沿用原请求` : '', ...reasons,
    ].filter(Boolean).join('。') + '。L1 就绪后可重新预览评论。');
    setBusy(false);
  }

  return <section className={c('study-detail-section')} aria-label="研究所选评论">
    <h3>研究所选评论（{selected.length} 条）</h3>
    <div className={c('study-link-actions')}>
      <label><input type="checkbox" checked={rerun} disabled={busy || unknown} onChange={event => {
        setRerun(event.target.checked); setPlan(null); setBatch(null);
      }} /> 重新研究已有结果的评论</label>
      <ActionButton type="button" disabled={busy || unknown} onClick={() => void prepare()}>预览所选评论</ActionButton>
    </div>
    {plan ? <>
      <p>已选 {plan.selected} 条 · 可研究 {plan.researchable} 条 · 涉及 {plan.works} 个可研究作品 · {plan.units} 个研究单元 · 预计 {plan.calls} 次单元调用</p>
      {plan.waiting ? <p>L1 未就绪 {plan.waiting} 条，涉及 {plan.missingWorks.length} 个作品。</p> : null}
      {plan.researched ? <p>已有结果，跳过 {plan.researched} 条。</p> : null}
      {plan.active ? <p>已排队或运行 {plan.active} 条。</p> : null}
      {plan.unresolved ? <p>上一次调用结果未确认 {plan.unresolved} 条。</p> : null}
      {plan.exclusions.length ? <ul>{plan.exclusions.map(item => <li key={item.code}>{why(item.code)} · {item.count} 条</li>)}</ul> : null}
    </> : null}
    <div className={c('study-link-actions')}>
      {plan?.missingWorks.length ? <ActionButton type="button" disabled={busy || unknown} onClick={() => void supplement()}>为缺 L1 的作品提交内容理解</ActionButton> : null}
      {plan && plan.researchable > 0 ? <ActionButton variant="primary" type="button" disabled={busy || Boolean(batch)} onClick={() => void start()}>{unknown ? '重试同一请求' : '开始所选评论研究'}</ActionButton> : null}
      {clear ? <ActionButton type="button" disabled={busy || unknown || l1Unknown} onClick={clear}>清空选择</ActionButton> : null}
      <button className={c('study-link')} type="button" onClick={() => batch && nav.run ? nav.run(batch) : nav.view('runs')}>查看研究记录</button>
    </div>
    {notice ? <p role="status">{notice}</p> : null}
  </section>;
}
