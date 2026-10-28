import type { StudyOverview } from './overviewDtos';
import { num } from './format';
import { c } from './styles';

function Delta({ value, label, detail }: { value: number | null | undefined; label: string; detail: string }) {
  const valid = value != null && Number.isFinite(value), amount = valid ? value : null;
  const kind = amount == null ? 'unknown' : amount > 0 ? 'up' : amount < 0 ? 'down' : 'flat';
  const shown = amount == null ? '—' : `${amount > 0 ? '+' : ''}${num(amount)}`;
  return <span className={c('study-metric-delta')} data-change={kind} title={detail} aria-label={`${label} ${shown}`}>
    {kind === 'up' || kind === 'down' ? <svg viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true"><path d={kind === 'up' ? 'M3 13L13 3M5.4 3H13v7.6' : 'M3 3l10 10M13 5.4V13H5.4'} /></svg> : <span aria-hidden="true">—</span>}
    <b>{shown}</b><small>{label}</small>
  </span>;
}

export function OverviewMetrics({ overview, candidates }: { overview: StudyOverview; candidates: () => void }) {
  const corpus = overview.corpusSummary;
  const research = overview.researchSummary, knowledge = overview.knowledgeSummary;
  const coverageNote = overview.indexCoverage ? `已索引 ${num(overview.indexCoverage.indexedCount)} 条；待索引 ${num(overview.indexCoverage.pendingCount)} 条` : '索引覆盖暂不可读';
  const points = overview.observationSeries?.points ?? [], y = points.length >= 2 ? points.at(-2) : null, yDate = y?.date || '上一完整自然日';
  const pendingIndex = Number(overview.indexCoverage?.pendingCount ?? 0);
  const displayable = corpus?.displayableCommentCount, eligible = corpus?.eligibleCommentCount, studied = research?.studiedCommentCount;
  const eligibleShown = pendingIndex ? null : eligible;
  const eligiblePct = Number(displayable) > 0 && eligible != null ? `${(100 * eligible / Number(displayable)).toFixed(1)}%` : '—';
  const studiedRate = !pendingIndex && eligible != null && studied != null && eligible > 0 ? 100 * studied / eligible : null;
  const rateText = studiedRate == null ? '—' : `${studiedRate.toFixed(1)}%`, fill = studiedRate == null ? 0 : Math.min(100, Math.max(0, studiedRate));
  const timestamp = new Date(overview.asOf || '');
  const timeLabel = Number.isFinite(timestamp.getTime()) ? new Intl.DateTimeFormat('zh-CN', { timeZone: overview.observationSeries?.timezone || 'Asia/Shanghai', year: 'numeric', month: '2-digit', day: '2-digit', hour: '2-digit', minute: '2-digit', hour12: false }).format(timestamp) : '统计时间未知';
  return <div className={c('study-overview-metrics')} aria-label="研究累计读数与昨日活动">
    <div className={c('study-metrics-asof')}><span>数据截至</span><time>{timeLabel}</time><span className={c('study-metrics-help')} tabIndex={0} title={`${coverageNote}。昨日角标表示上一完整自然日的事件量，不是累计指标净增。`}>ⓘ</span></div>
    <div className={c('study-metric study-metric-hero')}><p className={c('study-metric-label')}>可研究评论</p><div className={c('study-metric-value')}><strong>{num(eligibleShown)}</strong><small>条</small></div><div className={c('study-metric-foot')}><span>{pendingIndex ? `已有 ${num(corpus?.rawReadableCommentCount)} 条原始评论 · ${num(pendingIndex)} 条待索引` : `可显示 ${num(displayable)} · 可研究占 ${eligiblePct}`}</span><Delta value={y?.newObservedCommentCount} label="昨日首次观察" detail={`${yDate} 首次被观察的稳定评论身份，可能包含不可研究评论；不是昨日新增可研究评论。`} /></div></div>
    <div className={c('study-metric-separator')} aria-hidden="true" />
    <div className={c('study-metric study-metric-progress')}><p className={c('study-metric-label')}>已进入研究</p><div className={c('study-metric-value')}><strong>{num(studied)}</strong><small>条</small><span className={c('study-metric-rate')}>{rateText}<small>覆盖</small></span></div><div className={c('study-metric-rail')} role="progressbar" aria-label="可研究评论当前已进入研究比例" aria-valuemin={0} aria-valuemax={100} {...(studiedRate == null ? {} : { 'aria-valuenow': Number(fill.toFixed(1)) })}><i style={{ width: `${fill.toFixed(2)}%` }} /></div><div className={c('study-metric-foot')}><span>其中 {num(research?.succeededCommentCount)} 条已出结果</span><Delta value={y?.studiedCommentCount} label="昨日进入" detail={`${yDate} 研究目标来源去重的当日事件数，重研也可能计入；非当前累计净增。`} /></div></div>
    <div className={c('study-metric-separator')} aria-hidden="true" />
    <div className={c('study-metric-pair')}>
      <div className={c('study-metric study-metric-small')}><p className={c('study-metric-label')}>当前研究信号</p><div className={c('study-metric-value')}><strong>{num(research?.currentSignalCount)}</strong><small>个</small></div><Delta value={y?.acceptedSignalCount} label="昨日写入" detail={`${yDate} 研究信号写入量，不等于当前有效信号的净增。`} /></div>
      <div className={c('study-metric study-metric-small')}><p className={c('study-metric-label')}>长期用户问题</p><div className={c('study-metric-value')}><strong>{num(knowledge?.problemCount)}</strong><small>个</small></div><Delta value={overview.yesterdayNewProblemCount} label="昨日建档" detail={`${yDate} 建档且当前仍可读取的长期问题档案；受来源资格影响可能少于历史建档数。`} /></div>
    </div>
    <button type="button" className={c('study-metric-pending')} data-overview-pending-state="awaiting_independent_support" onClick={candidates}><i aria-hidden="true" /><span>{num(knowledge?.pendingCandidateCount)} 条表达等待独立支持</span><span className={c('study-metric-pending-arrow')} aria-hidden="true">→</span></button>
  </div>;
}
