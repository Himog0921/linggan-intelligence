import { useState, type ReactNode } from 'react';
import { Tag, Tabs } from '../ui';
import { dims, labels, percent, type Dimension, type Drill, type EffectDimension, type Rate, type Snapshot } from './api';
import css from './Lens.module.css';

type Open = (drill: Drill) => void;
const dimTabs = dims.map(dim => [dim, labels[dim]] as const);
function Link({ children, label, onClick }: { children: ReactNode; label: string; onClick: () => void }) {
  return <button type="button" className={css.link} aria-label={label} onClick={onClick}>{children}</button>;
}
function SectionTitle({ title, grade }: { title: string; grade: string }) {
  return <header className={css.heading}><h2>{title}</h2><Tag>{grade}</Tag></header>;
}
export function Quality({ snapshot: { quality: q }, open, spot }: { snapshot: Snapshot; open: Open; spot: () => void }) {
  const summary = q.agreement.find(a => !a.meetsThreshold) ?? q.agreement[0];
  return <section className={css.quality} aria-label="数据质量">
    <div><span>数据截至</span><strong>{q.latest ?? '未知'}</strong>{q.stale && <small>数据陈旧 · 距今 {q.days} 天</small>}</div>
    <div><span>有阅读量</span><strong>{percent(q.share)}</strong><small>
      <Link label="查看有阅读量的笔记" onClick={() => open({ metric: 'titledViews', label: '有阅读量' })}>{q.withViews}</Link> / <Link label="查看有标题的发布记录" onClick={() => open({ metric: 'titledPublications', label: '有标题的发布记录' })}>{q.titled}</Link> 篇
    </small><small>范围内 {q.publications} 篇，有标题 {q.titled} 篇，待标 {q.pending} 个标题</small></div>
    <div><span>机器标签一致率</span><details><summary>{summary ? (labels[summary.dimension as keyof typeof labels] ?? summary.dimension) + ' ' + percent(summary.value) + (summary.meetsThreshold ? '' : ' · 未达标') : '暂无复标结果'}</summary>{q.agreement.length ? q.agreement.map(a =>
      <p key={a.run + a.dimension}>{labels[a.dimension as keyof typeof labels] ?? a.dimension} · {percent(a.value)} · κ {a.kappa?.toFixed(2) ?? '未知'} · {a.count} 条 <small>批次 {a.run.slice(0, 8)}</small></p>) : <p>暂无复标结果</p>}</details></div>
    <div><span>人工抽看</span><Link label="查看人工抽看样本" onClick={spot}>查看抽看样本</Link><small>已裁定 {q.adjudicated} 个标题</small>{q.corrected !== null && <small>已改正 {q.corrected} 个标题</small>}</div>
    <div><span>分类体系</span><strong>v{q.taxonomy}</strong><small>指标 v{q.metric}</small></div>
  </section>;
}
export function Structure({ snapshot, open }: { snapshot: Snapshot; open: Open }) {
  const [dim, setDim] = useState<Dimension>('offer');
  return <section className={css.section}><SectionTitle title="内容结构" grade="结构事实" />
    <Tabs label="结构维度" values={dimTabs} current={dim} onSelect={v => setDim(v as Dimension)} compact />
    <div className={css.distribution}>{snapshot.structure[dim].map(row => <button type="button" className={css.group} key={row.key}
      aria-label={row.name + '，' + row.publications + ' 篇'} onClick={() => open({ label: row.name, metric: 'publications', [dim]: row.key })}>
      <span>{row.name}</span><strong>{row.publications} 篇</strong><small>{percent(row.share)}</small>
      <i aria-hidden="true" style={{ width: row.share === null ? 0 : percentCss(row.share) }} />
    </button>)}</div>
  </section>;
}
const percentCss = (v: number) => new Intl.NumberFormat('en-US', { style: 'percent', maximumFractionDigits: 4 }).format(v);
export function Coverage({ snapshot, open }: { snapshot: Snapshot; open: Open }) {
  const { coverage: { cells, sparse, empty, top10Share }, structure } = snapshot;
  const columns = [...new Set(cells.map(cell => cell.column))];
  const rows = new Map<string, Map<string, typeof cells[number]>>();
  for (const cell of cells) {
    const row = rows.get(cell.row) ?? new Map();
    row.set(cell.column, cell);
    rows.set(cell.row, row);
  }
  const names = (dim: Dimension, k: string) => structure[dim].find(v => v.key === k)?.name ?? k;
  return <section className={css.section}><SectionTitle title="覆盖分布" grade="结构事实" />
    <p>时刻 × 给什么 · {empty} 个空格 · {sparse} 个稀疏格 · 前十格占比 {percent(top10Share)}</p>
    <div className={css.tableScroll}><table className={css.grid}><caption className="v7-sr-only">时刻与给什么的发布篇数</caption>
      <thead><tr><th scope="col">时刻 / 给什么</th>{columns.map(col => <th scope="col" key={col}>{names('offer',col)}</th>)}</tr></thead>
      <tbody>{[...rows].map(([key,row]) => <tr key={key}><th scope="row">{names('moment',key)}</th>{columns.map(col => {
        const cell = row.get(col);
        if (!cell) return <td key={col}>不适用</td>;
        const label = names('moment',key) + ' × ' + names('offer',col);
        return <td key={col} data-state={cell.state}><button type="button" aria-label={label + '，' + cell.count + ' 篇'}
          onClick={() => open({ label, moment:key, offer:col, metric:'publications' })}>{cell.count}</button></td>;
      })}</tr>)}</tbody>
    </table></div><p className={css.legend}><span data-state="empty">空格</span><span data-state="sparse">稀疏</span><span data-state="populated">已覆盖</span></p>
  </section>;
}
function Interval({ rate:r, name, base, open }: { rate: Rate; name: string; base: Drill; open: Open }) {
  return <div className={css.rate}>
    <strong>{r.value === null ? (r.reason === 'no_data' ? '暂无样本' : '样本不足') : percent(r.value)}</strong>
    <span><Link label={name+'分子 '+r.numerator} onClick={() => open({ ...base, label:name+' · 分子', subset:'numerator' })}>{r.numerator}</Link>
      {' / '}<Link label={name+'分母 '+r.denominator} onClick={() => open({ ...base, label:name+' · 分母', subset:'denominator' })}>{r.denominator}</Link></span>
    {r.low !== null && r.high !== null && r.value !== null && <>
      <svg className={css.interval} viewBox="0 0 100 8" preserveAspectRatio="none" aria-hidden="true">
        <line x1="0" x2="100%" y1="4" y2="4" /><line className={css.range} x1={percentCss(r.low)} x2={percentCss(r.high)} y1="4" y2="4" />
        <line className={css.point} x1={percentCss(r.value)} x2={percentCss(r.value)} y1="0" y2="8" />
      </svg><small>区间 {percent(r.low)}–{percent(r.high)}</small></>}
  </div>;
}
export function Effects({ snapshot, open }: { snapshot: Snapshot; open: Open }) {
  const [dim, setDim] = useState<EffectDimension>('offer');
  const effect = snapshot.effect[dim];
  const labelFor = (k: string) => snapshot.structure[dim].find(v => v.key === k)?.name ?? k;
  return <section className={css.section}><SectionTitle title="发布效果" grade="待检验假设" />
    <Tabs label="效果维度" values={dimTabs.filter(([v]) => v !== 'situation')} current={dim} onSelect={v => setDim(v as EffectDimension)} compact />
    {!effect.available ? <p role="status">{labels[dim]}的复标一致率不足，暂不展示该维度的效果。</p> :
      <div className={css.effectTable}><table className={css.effects}><thead><tr><th>{labels[dim]}</th><th>过千率</th><th>评论活跃率</th><th>千次阅读评论数</th><th>阅读量中位数</th><th>爆款篇数</th><th>账号内阅读偏差</th></tr></thead>
        <tbody>{effect.rows.map(row => {
          const label = labelFor(row.key);
          const drill = (metric: Drill['metric']): Drill => ({ metric, label, [dim]:row.key });
          return <tr key={row.key}><th scope="row">{label}</th>
            <td><Interval rate={row.midRate} name={label+'过千率'} base={drill('overThousand')} open={open} /></td>
            <td><Interval rate={row.activeCommentRate} name={label+'评论活跃率'} base={drill('activeComments')} open={open} /></td>
            <td><Link label={label+'千次阅读评论数样本'} onClick={() => open(drill('commentsPerThousandViews'))}>{row.commentsPerThousandViews.value?.toFixed(1) ?? '未知'}</Link></td>
            <td><Link label={label+'阅读量中位数样本'} onClick={() => open(drill('views'))}>{row.medianViews ?? '未填'}</Link></td>
            <td><Link label={label+'爆款篇数'} onClick={() => open({...drill('hit'), subset:'numerator'})}>{row.highCount}</Link></td>
            <td><Link label={label+'账号内阅读偏差样本'} onClick={() => open(drill('withinAccountViews'))}>{row.withinAccountResidual.value?.toFixed(2) ?? '未知'}</Link></td>
          </tr>;
        })}</tbody></table></div>}
    <h3>维度影响份额</h3>
    <table className={css.effects}><thead><tr><th>维度</th><th>范围</th><th>扣除随机水平后</th><th>随机水平</th><th>置换 p 值</th><th>样本</th></tr></thead>
      <tbody>{snapshot.effect.variance.map(v => {
        const name = v.factor === 'account' ? '账号' : labels[v.factor as Dimension];
        const within = v.scope === 'within_account';
        return <tr key={v.factor+v.scope}><th scope="row">{name}</th><td>{within ? '账号内' : '全体'}</td><td>{percent(v.excess)}</td><td>{percent(v.nullMean)}</td><td>{v.p?.toPrecision(3) ?? '未知'}</td><td><Link label={name+'影响份额样本'} onClick={() => open({ label:name+'影响份额 · '+(within?'账号内':'全体'),metric:within?'withinAccountViews':'views' })}>{v.observations}</Link></td></tr>;
      })}</tbody></table>
    <p className={css.note}>这些差异来自已发布笔记的回溯，账号、时机和偶然都混在里面，只能当作待检验的假设。</p>
  </section>;
}
