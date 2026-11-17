import { useState } from 'react';
import { useQuery } from '@tanstack/react-query';
import { Link } from 'react-router';
import { readCards, type Card, type Comparison, type DemandGroup } from './opportunityApi';
import { FocusAuthors, Metric, QualityFlags } from './presentation';
import { comparisonReason, protocolLabel, readableError, supplyLabel, tierLabel } from './labels';
import styles from './Opportunity.module.css';

interface Row { card: Card; group: DemandGroup; comparison: Comparison }

function taskLinks(domain: string) {
  return [
    { label: '查看用户问题', description: '从评论研究的问题出发', to: `/corpus/comments?${new URLSearchParams({ domain, view: 'problems' })}` },
    { label: '查看作品依据', description: '在证据库阅读来源材料', to: `/corpus/evidence?${new URLSearchParams({ domain })}` },
    { label: '查看创作者', description: '在创作者目录了解作者', to: `/corpus/creators?${new URLSearchParams({ domain })}` },
  ];
}

function ObservationTable({ rows, domain, label }: { rows: Row[]; domain: string; label: string }) {
  return <div className={styles.tableScroll}><table className={styles.table} aria-label={label}>
    <thead><tr><th scope="col">用户问题</th><th scope="col">提及人数</th><th scope="col">提及率（每千条评论，按作品发布周）及上期对比</th><th scope="col">供给情况</th><th scope="col">受众集中的作者</th><th scope="col">证据等级</th></tr></thead>
    <tbody>{rows.map(({ card, group, comparison }) => {
      const flags = [...new Set([...card.flags, ...group.flags])];
      const detail = `/opportunity?${new URLSearchParams({ domain, problem: card.problem.id })}`;
      return <tr key={`${card.problem.id}:${group.protocolVersion ?? 'unknown'}`}>
        <th scope="row"><Link className={styles.problemLink} to={detail}>{card.problem.statement}</Link><span className={styles.metaLine}>{protocolLabel(group.protocolVersion)}</span><QualityFlags flags={flags} /></th>
        <td><Metric value={group.mentionersTotal} flags={flags} /></td>
        <td><span className={styles.metaLine}>本周 <Metric value={comparison.currentRatePer1k} flags={flags} /></span><span className={styles.metaLine}>上周 <Metric value={comparison.previousRatePer1k} flags={flags} /></span>{comparison.status === 'incomparable' ? <span className={styles.muted}>不可比较：{comparisonReason(comparison.reason)}</span> : <span className={styles.muted}>{comparison.status === 'rising' ? '观察到上升' : '本周未观察到上升'}</span>}</td>
        <td>{supplyLabel(card.supply.state)}</td><td><FocusAuthors groups={card.channel.groups} protocol={group.protocolVersion} /></td><td>{tierLabel(group.evidenceTier)}</td>
      </tr>;
    })}</tbody>
  </table></div>;
}

export function OpportunityHome({ domain }: { domain: string }) {
  const [cursors, setCursors] = useState<(string | null)[]>([null]);
  const cursor = cursors.at(-1) ?? null;
  const query = useQuery({ queryKey: ['opportunity', 'list', domain, cursor], queryFn: ({ signal }) => readCards(domain, cursor, signal), gcTime: 0, refetchOnWindowFocus: true });
  const restart = () => { if (cursor === null) void query.refetch(); else setCursors([null]); };
  const rows: Row[] = query.data?.items.flatMap((card, index) => card.demand.groups.flatMap((group, groupIndex) => {
    const comparison = query.data?.comparisons[index]?.groups[groupIndex];
    return comparison ? [{ card, group, comparison }] : [];
  })) ?? [];
  const rising = rows.filter(row => row.comparison.status === 'rising');
  const other = rows.filter(row => row.comparison.status !== 'rising');
  return <>
    <header className={styles.header}><div><h1>机会首页</h1><p>围绕用户问题，查看讨论、作品和作者依据。</p></div></header>
    <nav className={styles.tasks} aria-label="从任务开始">{taskLinks(domain).map(item => <Link key={item.to} to={item.to}><strong>{item.label}</strong><span>{item.description}</span></Link>)}</nav>
    <section className={styles.section} aria-labelledby="rising-title"><div className={styles.sectionHeading}><div><h2 id="rising-title">本周观察到上升</h2><p>同一已知采集协议的本周与上周提及率对比；这不是显著性或市场趋势判断。</p></div>{query.data && !query.isError ? <small>读取于 {query.data.page.asOf}（UTC）</small> : null}</div>
      {query.isPending ? <p role="status">正在读取机会列表…</p> : query.isError ? <div role="alert"><p>{readableError(query.error)}</p><button type="button" onClick={restart}>从第一页重新读取</button></div> : !query.data.items.length ? <p>当前领域尚无可读取的机会卡。</p> : <>
        {rising.length ? <ObservationTable rows={rising} domain={domain} label="本周观察到上升的问题" /> : <p>本页没有观察到上升的问题。</p>}
        {other.length ? <details className={styles.other}><summary>查看本页其他观察与不可比较项</summary><ObservationTable rows={other} domain={domain} label="本页其他观察与不可比较项" /></details> : null}
        <div className={styles.pagination}><button type="button" disabled={cursors.length === 1} onClick={() => setCursors(old => old.slice(0, -1))}>上一页</button><button type="button" disabled={!query.data.page.hasMore} onClick={() => { const next = query.data.page.nextCursor; if (next) setCursors(old => [...old, next]); }}>下一页</button></div>
      </>}
      <p className={styles.limitation}>提及人数反映讨论热度，不等于问题对用户的重要性；评论只代表愿意留言的人，且每篇作品只采集平台排序靠前的部分评论。</p>
    </section>
  </>;
}
