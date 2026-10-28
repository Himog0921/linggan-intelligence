import { useState } from 'react';
import { ApiError, requestJson } from '../../../shared/api';
import { decode, str } from '../../../shared/decode';
import { preview, type Preview } from './commentResearchApi';
import type { SelectedComment, StudyNavigation } from './studyApi';
import { c } from './styles';

type Prepared = { input: Preview; commandRef: string; receipt: string | null };
export function CommentResearchSelection({ domain, selected, nav, clear }: { domain: string; selected: SelectedComment[]; nav: StudyNavigation; clear: () => void }) {
  const [prepared, setPrepared] = useState<Prepared[]>([]), [notice, setNotice] = useState(''), [busy, setBusy] = useState(false), [uncertain, setUncertain] = useState(false);
  const current = prepared.length === selected.length && prepared.every((p, i) => p.input.sourceRef === selected[i]?.sourceRef);
  async function prepare() {
    setBusy(true); setNotice(''); setPrepared([]);
    try {
      const next: Prepared[] = [];
      for (const entry of selected) {
        const input = preview(await requestJson(`/api/local/comment-research/preview?${new URLSearchParams({ domainRef: domain, workRef: entry.commentKey.workRef, sourceRef: entry.sourceRef, observationRole: entry.observationRole })}`));
        next.push({ input, commandRef: crypto.randomUUID(), receipt: null });
      }
      setPrepared(next); setNotice(`已检查 ${next.length} 条评论及其完整可用底稿。缺页和未识读限制会进入每项研究；父评论只提供语境。`);
    } catch { setNotice('部分所选评论或底稿不可用，尚未提交研究。请刷新评论后重新选择。'); } finally { setBusy(false); }
  }
  async function start() {
    if (!current) return; setBusy(true); setNotice('');
    const next = [...prepared];
    try {
      for (let i = 0; i < next.length; i++) {
        const entry = next[i]; if (!entry) throw new Error('选择范围已变化'); if (entry.receipt) continue;
        const p = entry.input;
        const receipt = decode(await requestJson('/api/local/comment-research', { method: 'POST', body: { commandRef: entry.commandRef, domainRef: p.domainRef, workRef: p.workRef, sourceRef: p.sourceRef, observationRole: p.observationRole, knowledgeCutoff: p.knowledgeCutoff, expectedInputHash: p.inputHash, taskConfigRef: p.taskConfigRef, expectedTaskRevision: p.taskRevision } }), r => str(r, 'requestRef'));
        next[i] = { ...entry, receipt }; setPrepared([...next]);
      }
      setUncertain(false); setNotice(`已提交 ${next.length} 条研究，结果尚未完成。`);
    } catch (error) { setUncertain(!(error instanceof ApiError && error.answered)); setNotice(`已确认提交 ${next.filter(p => p.receipt).length} 条；余下提交未完成。重试沿用原请求编号，不会重复提交已确认项。若设置或材料已变化，请先核对研究记录。`); } finally { setBusy(false); }
  }
  return <section className={c('study-detail-section')} aria-label="研究所选评论"><h3>研究所选评论（{selected.length} / 50）</h3><p>逐条检查目标原声和作品研究底稿，三阶段各用独立的 AI 做法。开始后会调用已配置模型并消耗额度。</p>
    <button type="button" disabled={busy || uncertain} onClick={() => void prepare()}>检查所选评论</button>
    {current ? <button type="button" disabled={busy || prepared.every(p => p.receipt)} onClick={() => void start()}>{uncertain ? '重试未确认项' : '开始所选评论研究'}</button> : null}
    <button type="button" disabled={busy || uncertain} onClick={() => { clear(); setPrepared([]); }}>清空选择</button><button type="button" onClick={() => nav.view('runs')}>查看研究记录</button>{notice ? <p role="status">{notice}</p> : null}
  </section>;
}
