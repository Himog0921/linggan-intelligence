import { nonBlank, num } from './format';
import { signalKindLabel } from './labels';
import { candidateStates, changeKinds, problemWarning, resultDate } from './problemModel';
import type { ProblemCard, ProblemClue } from './problemDtos';
import { c } from './styles';

export function ProblemMetric({ value, label }: { value: number | null; label: string }) {
  return <span className={c('sr3-metric')}><b title={num(value)}>{value != null && value >= 10000 ? `${(value / 10000).toFixed(1)}万` : num(value)}</b><span>{label}</span></span>;
}
export function ProblemResultCard({ problem: p, onOpen }: { problem: ProblemCard; onOpen: () => void }) {
  const canDefine = p.definitionReadable, preview = p.preview?.sourceState === 'known' && p.preview.commentText?.trim() ? p.preview : null;
  const title = canDefine ? nonBlank(p.title) || '问题标题未记录' : '问题定义当前不可读取';
  return <article className={c('sr3-card', p.changeKind === 'definition_revision' ? 'sr3-revised' : '')}>
    <div className={c('sr3-card-top')}><span className={c('sr3-id')}>{p.problemRef.slice(0, 8)}</span><span className={c('sr3-kind')}>{changeKinds[p.changeKind || ''] || '更新状态未记录'}</span><span className={c('sr3-register')} aria-hidden="true">+</span></div>
    <h3 className={c('sr3-card-title')}>{title}</h3><div className={c('sr3-note')}><span>已保存的研究定义</span><p>{canDefine ? nonBlank(p.definition) || '定义正文未记录' : '不从受限材料补写结论。'}</p></div>
    <div className={c('sr3-quote')}><span className={c('sr3-quote-label')}>关联原声 <i aria-hidden="true">↘</i></span><blockquote>{preview ? preview.commentText : '当前没有可展示的原声预览。'}</blockquote><span className={c('sr3-quote-caption')}>{preview ? '预览 1 条 · 不作为整段定义的验证' : '原声可读性以最新读取为准'}</span></div>
    <div className={c('sr3-card-bottom')}><div className={c('sr3-metrics')}><ProblemMetric value={p.supportCommentCount} label="原声" /><ProblemMetric value={p.supportAuthorCount} label="账号" /><ProblemMetric value={p.supportWorkCount} label="作品" /></div><span className={c('sr3-open-glyph')} aria-hidden="true">↗</span></div>
    <div className={c('sr3-card-foot')}><span>{problemWarning(p) || `近 28 天首次关联 ${num(p.recentAddedSupportCommentCount)} 条原声`}</span><time>{resultDate(p.updatedAt)}</time></div><button type="button" className={c('sr3-card-hit')} data-sr3="open" data-ref={p.problemRef} aria-label={`查看问题：${title}`} onClick={onOpen}><span className="v7-sr-only">查看定义、原声与形成过程</span></button>
  </article>;
}
export function ProblemClueCard({ clue: p, onOpen }: { clue: ProblemClue; onOpen: () => void }) {
  const canRead = p.sourceState === 'known' && Boolean(p.commentText?.trim()), other = ['solution', 'experience'].includes(p.kind);
  return <article className={c('sr3-card sr3-clue')}><div className={c('sr3-card-top')}><span className={c('sr3-id')}>线索</span><span className={c('sr3-kind')}>{candidateStates[p.state] || '处理状态未记录'}</span><span aria-hidden="true">+</span></div><h3 className={c('sr3-card-title')}>{canRead ? nonBlank(p.proposition) || '尚未记录归一表达' : '来源当前不可读取'}</h3><div className={c('sr3-note')}><span>{signalKindLabel[p.kind] || p.kind}</span><p>{other ? '建议或个人经验，不作为已归纳问题。' : '有效表达尚未建立长期问题，处理状态不代表用户观点。'}</p></div><div className={c('sr3-quote')}><span className={c('sr3-quote-label')}>原始表达</span><blockquote>{canRead ? p.commentText : '来源受限，原声及衍生内容不显示。'}</blockquote></div><div className={c('sr3-card-bottom')}><span className={c('sr3-caption')}>查看原声与处理依据</span><span className={c('sr3-open-glyph')} aria-hidden="true">↗</span></div><div className={c('sr3-card-foot')}><span>1 条表达 · 不计入问题数量</span><time>{resultDate(p.createdAt)}</time></div><button type="button" className={c('sr3-card-hit')} data-sr3="clue" data-ref={p.signalRef} aria-label="查看这条未归并表达" onClick={onOpen}><span className="v7-sr-only">查看线索和处理依据</span></button></article>;
}
