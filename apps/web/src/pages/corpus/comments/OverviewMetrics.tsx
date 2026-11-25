import type { StudyOverview } from './overviewDtos';
import { num } from './format';
import { CrIcon } from './CrIcon';
import { c } from './styles';

function Delta({ value, label, detail }: { value: number | null | undefined; label: string; detail: string }) {
  const amount = value != null && Number.isFinite(value) ? value : null;
  const shown = amount == null ? '—' : amount > 0 ? `↗ +${num(amount)}` : amount < 0 ? `↘ ${num(amount)}` : `+${num(amount)}`;
  return <span className={c('delta')} data-change={amount == null ? 'unknown' : amount > 0 ? 'up' : amount < 0 ? 'down' : 'flat'} title={detail} aria-label={`${label} ${amount == null ? '未知' : num(amount)}`}>{shown}<small>{label}</small></span>;
}

const percent = (part: number, whole: number) => whole > 0 ? `${(Math.round(part / whole * 1000) / 10).toFixed(1).replace(/\.0$/, '')}%` : '—';

/** 「已进入研究」分段条：有 researchBreakdown 时四段；旧服务没有时退回单段（已有结果 / 可研究）。 */
function ResearchBar({ overview }: { overview: StudyOverview }) {
  const breakdown = overview.researchBreakdown, eligible = overview.corpusSummary.eligibleCommentCount;
  const segments: [string, number, string][] = breakdown
    ? [['有结果', breakdown.withResult, 'done'], ['研究中', breakdown.running, 'running'], ['最近失败', breakdown.failedWithoutResult, 'failed'], ['未研究', breakdown.never, 'never']]
    : [['已有结果', overview.researchSummary.succeededCommentCount, 'done'], ['可研究', Math.max(0, eligible - overview.researchSummary.succeededCommentCount), 'never']];
  const total = Math.max(1, breakdown ? segments.reduce((sum, [, n]) => sum + n, 0) : eligible);
  return <>
    <div className={c('research-bar')} role="img" aria-label={`可研究评论 ${num(eligible)} 条：` + segments.map(([label, n]) => `${label} ${num(n)} 条`).join('，')}>
      {segments.map(([label, n, kind]) => n ? <i key={kind} className={c(kind)} style={{ width: `${n / total * 100}%` }} title={`${label} ${num(n)} 条`} /> : null)}
    </div>
    <div className={c('research-legend')}>{segments.map(([label, n, kind]) => <span key={kind}><i className={c(kind)} />{label} <b>{num(n)}</b></span>)}</div>
  </>;
}

export function OverviewMetrics({ overview }: { overview: StudyOverview }) {
  const corpus = overview.corpusSummary, research = overview.researchSummary;
  const points = overview.observationSeries.points, y = points.length >= 2 ? points.at(-2) : null, yDate = y?.date || '上一完整自然日';
  const pendingIndex = overview.indexCoverage.pendingCount;
  const withResult = overview.researchBreakdown?.withResult ?? research.succeededCommentCount;
  const timestamp = new Date(overview.asOf || '');
  const timeLabel = Number.isFinite(timestamp.getTime()) ? new Intl.DateTimeFormat('zh-CN', { timeZone: overview.observationSeries.timezone || 'Asia/Shanghai', year: 'numeric', month: '2-digit', day: '2-digit', hour: '2-digit', minute: '2-digit', hour12: false }).format(timestamp) : '统计时间未知';
  const coverageNote = `已索引 ${num(overview.indexCoverage.indexedCount)} 条；待索引 ${num(pendingIndex)} 条。昨日角标表示上一完整自然日的事件量，不是累计指标净增。`;
  return <div className={c('ov-cell metric-col')} role="group" aria-label="研究累计读数与昨日活动">
    <div className={c('data-stamp')}><span>数据截至</span><span className={c('stamp-date')}><time>{timeLabel}</time> <span tabIndex={0} title={coverageNote} aria-label={coverageNote}><CrIcon name="Info" /></span></span></div>
    <div className={c('metric-block')}>
      <div className={c('metric-title')}>可研究评论</div>
      <div className={c('metric-number')}><span>{num(pendingIndex ? null : corpus.eligibleCommentCount)}</span><small>条</small></div>
      <div className={c('metric-sub')}><span>已有 {num(corpus.rawReadableCommentCount)} 条原始评论 · {num(pendingIndex)} 条待索引</span>
        <Delta value={y?.newObservedCommentCount} label="昨日首次观察" detail={`${yDate} 首次被观察的稳定评论身份，可能包含不可研究评论；不是昨日新增可研究评论。`} /></div>
    </div>
    <div className={c('metric-block')}>
      <div className={c('metric-title')}>已进入研究</div>
      <div className={c('metric-number')}><span>{num(research.studiedCommentCount)}</span><small>条</small></div>
      <ResearchBar overview={overview} />
      <div className={c('metric-sub')}><span>{num(withResult)} 条已有结果</span><span>结果覆盖 {pendingIndex ? '—' : percent(withResult, corpus.eligibleCommentCount)}</span></div>
    </div>
  </div>;
}

export function OverviewKnowledge({ overview, problems, candidates }: { overview: StudyOverview; problems: { formal: number; candidate: number }; candidates: () => void }) {
  const y = overview.observationSeries.points.at(-2), yDate = y?.date || '上一完整自然日';
  return <div className={c('ov-cell ov-row2 metric-col')}>
    <div className={c('metric-pair')}>
      <div><div className={c('metric-title')}>当前研究信号</div><div className={c('metric-number')}><span>{num(overview.researchSummary.currentSignalCount)}</span><small>条</small></div>
        <div className={c('metric-sub')}><span>来自当前有效结果</span></div>
        <div className={c('metric-sub')}><Delta value={y?.acceptedSignalCount} label="昨日写入" detail={`${yDate} 研究信号写入量，不等于当前有效信号的净增。`} /></div></div>
      <div><div className={c('metric-title')}>长期用户问题</div><div className={c('metric-number')}><span>{num(problems.formal + problems.candidate)}</span><small>个</small></div>
        <div className={c('metric-sub')}><span>正式 {num(problems.formal)} · 候选 {num(problems.candidate)}</span></div>
        <div className={c('metric-sub')}><Delta value={overview.yesterdayNewProblemCount} label="昨日建档" detail={`${yDate} 建档且当前仍可读取的正式问题档案；受来源资格影响可能少于历史建档数。`} /></div></div>
    </div>
    <button type="button" className={c('candidate-link')} data-overview-pending-state="awaiting_independent_support" onClick={candidates}>
      <span>{num(problems.candidate)} 个候选问题等待独立支持</span><CrIcon name="ArrowRight" />
    </button>
  </div>;
}
