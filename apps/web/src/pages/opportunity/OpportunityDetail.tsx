import { useState } from 'react';
import { useQuery, useQueryClient } from '@tanstack/react-query';
import { Link } from 'react-router';
import { readCard, readEvidence, type Card, type Creator, type CreatorGroup, type EvidenceKind, type Voice } from './opportunityApi';
import { Metric, QualityFlags } from './presentation';
import { protocolLabel, readableError, supplyLabel, tierLabel } from './labels';
import styles from './Opportunity.module.css';

function VoiceItem({ voice, domain, flags }: { voice: Voice; domain: string; flags: readonly string[] }) {
  const comment = `/corpus/comments?${new URLSearchParams({ domain, view: 'comments', detail: 'comment', commentWorkRef: voice.commentRef.contentRef, commentExternalId: voice.commentRef.commentExternalId })}`;
  return <li className={styles.voice}><blockquote>{voice.tombstone ? '来源已撤回：原文不再展示，保留引用记录' : voice.text ?? '原声不可用'}</blockquote>{!voice.tombstone ? <div className={styles.voiceMeta}>点赞 <Metric value={voice.likes} flags={flags} /> · <Link to={comment}>查看原声上下文</Link></div> : null}</li>;
}

function CreatorLine({ creator, flags }: { creator: Creator; flags: readonly string[] }) {
  return <li className={styles.creator}>
    <div><strong>{creator.name ?? '姓名未知'}</strong><span className={styles.metaLine}>{creator.observed === null ? '观察状态未知' : creator.observed ? '已在观察名单' : '未在观察名单'}</span></div>
    <div className={styles.creatorChart}>
      <svg viewBox="0 0 1000 24" preserveAspectRatio="none" role="img" aria-label={`提及率每千 ${creator.ratePer1k}，区间 ${creator.ciPer1k.low} 至 ${creator.ciPer1k.high}`}>
        <rect x="0" y="8" width={creator.ratePer1k} height="8" className={styles.bar} />
        <line x1={creator.ciPer1k.low} y1="12" x2={creator.ciPer1k.high} y2="12" className={styles.interval} />
        <circle cx={creator.ratePer1k} cy="12" r="5" className={styles.point} />
      </svg>
      <span>每千 <Metric value={creator.ratePer1k} flags={flags} /> · 区间 <Metric value={creator.ciPer1k.low} flags={flags} />–<Metric value={creator.ciPer1k.high} flags={flags} /></span>
    </div>
    <span className={styles.creatorCount}>提及 <Metric value={creator.k} flags={flags} /> 人 · 有效评论 <Metric value={creator.n} flags={flags} /> 条</span>
  </li>;
}

function ChannelGroup({ group, flags }: { group: CreatorGroup; flags: readonly string[] }) {
  return <section className={styles.protocol}><h3>{protocolLabel(group.protocolVersion)}</h3>
    {group.ranked.length ? <ol className={styles.creators}>{group.ranked.map(creator => <CreatorLine key={creator.creatorId} creator={creator} flags={flags} />)}</ol> : <p className={styles.muted}>暂无可排序作者。</p>}
    {group.insufficient.length ? <><h4>样本不足，暂不排序</h4><ul className={styles.creators}>{group.insufficient.map(creator => <CreatorLine key={creator.creatorId} creator={creator} flags={[...flags, 'insufficient_sample']} />)}</ul></> : null}
  </section>;
}

function useEvidence(card: Card, domain: string, refreshCard: () => void, onEvidenceFailure: () => void) {
  const queryClient = useQueryClient();
  const [kind, setKind] = useState<EvidenceKind | null>(null);
  const [cursors, setCursors] = useState<(string | null)[]>([null]);
  const cursor = cursors.at(-1) ?? null;
  const query = useQuery({ queryKey: ['opportunity', 'evidence', domain, card.problem.id, card.asOf, kind, cursor],
    queryFn: async ({ signal }) => {
      try { return await readEvidence(domain, card.problem.id, kind ?? 'comments', cursor, signal); }
      catch (error) { if (!signal.aborted) onEvidenceFailure(); throw error; }
    }, enabled: kind !== null,
    gcTime: 0, refetchOnWindowFocus: true });
  const changeKind = (next: EvidenceKind) => { setKind(next); setCursors([null]); };
  const restart = () => { if (cursor === null) void query.refetch(); else setCursors([null]); };
  const refreshMismatchedCard = () => {
    onEvidenceFailure();
    setKind(null); setCursors([null]);
    queryClient.removeQueries({ queryKey: ['opportunity', 'evidence', domain, card.problem.id] });
    refreshCard();
  };
  return { kind, cursors, setCursors, query, changeKind, restart, refreshMismatchedCard };
}

function EvidencePanel({ card, domain, evidence }: { card: Card; domain: string; evidence: ReturnType<typeof useEvidence> }) {
  const { kind, cursors, setCursors, query, changeKind, restart, refreshMismatchedCard } = evidence;
  return <section className={styles.section} aria-labelledby="evidence-title"><h2 id="evidence-title">判断与依据</h2>
    <p>供给尚未评估。可查看这张卡片关联的评论、作品和作者依据。</p>
    <div className={styles.evidenceTabs} role="group" aria-label="依据类型">{([['comments', '评论依据'], ['works', '作品依据'], ['creators', '作者依据']] as const).map(([value, label]) => <button type="button" key={value} aria-pressed={kind === value} onClick={() => changeKind(value)}>{label}</button>)}</div>
    {kind === null ? <p className={styles.muted}>选择一种依据后读取。</p> : query.isPending || query.isFetching ? <p role="status">正在读取依据…</p> : query.isError ? <div role="alert"><p>{readableError(query.error)}</p><button type="button" onClick={restart}>从第一页重新读取</button></div> : query.data.cardAsOf !== card.asOf ? <div role="alert"><p>卡片已更新，请重新读取后查看依据。</p><button type="button" onClick={refreshMismatchedCard}>重新读取卡片</button></div> : <>
      <p className={styles.muted}>卡片计算于 {query.data.cardAsOf}（UTC）；依据读取于 {query.data.page.asOf}（UTC）。</p>
      {!query.data.items.length ? <p>当前没有可读取的{kind === 'comments' ? '评论' : kind === 'works' ? '作品' : '作者'}依据。</p> : <ul className={styles.evidenceList}>{query.data.items.map((item, index) => {
        if ('commentRef' in item) return <VoiceItem key={`${item.commentRef.contentRef}:${item.commentRef.commentExternalId}`} voice={item} domain={domain} flags={card.flags} />;
        if ('creator' in item) return <li key={`${item.protocolVersion ?? 'unknown'}:${item.creator.creatorId}`}><strong>{item.creator.name ?? '姓名未知'}</strong> · {protocolLabel(item.protocolVersion)} · {item.section === 'insufficient' ? '样本不足，暂不排序' : '组内排序'} · 提及率每千 <Metric value={item.creator.ratePer1k} flags={card.flags} /></li>;
        const href = `/corpus/evidence?${new URLSearchParams({ domain, work: item.workId })}`;
        return <li key={`${item.workId}:${index}`}><Link to={href}>查看关联作品</Link> <span className={styles.muted}>依据：评论提及；不代表已判断供给。</span></li>;
      })}</ul>}
      <div className={styles.pagination}><button type="button" disabled={cursors.length === 1} onClick={() => setCursors(old => old.slice(0, -1))}>上一页</button><button type="button" disabled={!query.data.page.hasMore} onClick={() => { const next = query.data.page.nextCursor; if (next) setCursors(old => [...old, next]); }}>下一页</button></div>
    </>}
  </section>;
}

function CardContent({ card, domain, home, refreshCard, onEvidenceFailure }: { card: Card; domain: string; home: string; refreshCard: () => void; onEvidenceFailure: () => void }) {
  const evidence = useEvidence(card, domain, refreshCard, onEvidenceFailure);
  const voicesVisible = evidence.kind === null || (!evidence.query.isPending && !evidence.query.isFetching && !evidence.query.isError && evidence.query.data?.cardAsOf === card.asOf);
  return <>
    <header className={styles.header}><div><Link to={home}>← 返回机会首页</Link><h1>{card.problem.statement}</h1><p>卡片计算于 {card.asOf}（UTC） · 分类版本 {card.versions.taxonomy} · 指标版本 {card.versions.metricDef}</p></div></header>
    {card.problem.redirectedFrom ? <p className={styles.notice}>原问题已合并，当前显示合并后的问题。</p> : null}
    <QualityFlags flags={card.flags} />
    {card.demand.groups.map(group => {
      const flags = [...new Set([...card.flags, ...group.flags])];
      return <section className={styles.protocolMetrics} key={group.protocolVersion ?? 'unknown'}><h2>{protocolLabel(group.protocolVersion)}</h2><QualityFlags flags={group.flags} />
        <dl className={styles.metrics}><div><dt>提及人数</dt><dd><Metric value={group.mentionersTotal} flags={flags} /></dd></div><div><dt>周期 {group.current.periodStart} 提及人数</dt><dd><Metric value={group.current.k} flags={flags} /></dd></div><div><dt>周期 {group.current.periodStart} 有效评论数</dt><dd><Metric value={group.current.n} flags={flags} /></dd></div><div><dt>提及率（每千条评论，按作品发布周）</dt><dd><Metric value={group.current.ratePer1k} flags={flags} /></dd></div></dl>
      </section>;
    })}
    <div className={styles.columns}>
      <section className={styles.column}><h2>需求</h2><p className={styles.notice}>{card.demand.importance.status === 'not_calibrated' ? '重要性未校准' : '重要性已校准'}</p>
        {card.demand.groups.map(group => <section className={styles.protocol} key={group.protocolVersion ?? 'unknown'}><h3>{protocolLabel(group.protocolVersion)} · {tierLabel(group.evidenceTier)}</h3><QualityFlags flags={group.flags} /><div className={styles.tableScroll}><table className={styles.series}><thead><tr><th scope="col">周期开始</th><th scope="col">提及人数</th><th scope="col">有效评论数</th><th scope="col">提及率 / 千</th></tr></thead><tbody>{group.series.map(point => <tr key={point.periodStart}><th scope="row">{point.periodStart}</th><td><Metric value={point.k} flags={group.flags} /></td><td><Metric value={point.n} flags={group.flags} /></td><td><Metric value={point.ratePer1k} flags={group.flags} /></td></tr>)}</tbody></table></div><p className={styles.muted}>基线提及率 / 千：<Metric value={group.baseline.ratePer1k} flags={group.flags} /></p></section>)}
        <h3>代表原声</h3>{!voicesVisible ? <p className={styles.muted}>正在核对来源，原声暂不展示。</p> : card.demand.sampleVoices.length ? <ul className={styles.evidenceList}>{card.demand.sampleVoices.map(voice => <VoiceItem key={`${voice.commentRef.contentRef}:${voice.commentRef.commentExternalId}`} voice={voice} domain={domain} flags={card.flags} />)}</ul> : <p className={styles.muted}>当前没有可展示的代表原声。</p>}
        <h3>用户办法</h3>{card.demand.userMethods.status === 'unavailable' ? <p className={styles.muted}>暂不可用：目前无法证明办法与该问题的归属。</p> : <ul>{card.demand.userMethods.items.map(item => <li key={item.label}>{item.label} · 提及人数 <Metric value={item.mentioners} flags={card.flags} /></li>)}</ul>}
      </section>
      <section className={styles.column}><h2>供给</h2><p className={styles.supply}>{supplyLabel(card.supply.state)}</p>{card.supply.state === 'not_evaluated' ? <p className={styles.muted}>尚未开展作品供给判断。</p> : <p className={styles.muted}>{card.supply.judgmentStatus === 'ai_suggested' ? 'AI 建议' : card.supply.judgmentStatus === 'verified' ? '已复核' : '判断状态未知'}</p>}</section>
      <section className={styles.column}><h2>渠道</h2><p className={styles.muted}>作者窗口 <Metric value={card.channel.windowDays} flags={card.flags} /> 天 · 排序最少样本 <Metric value={card.channel.minN} flags={card.flags} /></p><p className={styles.muted}>条形为点估计，线段为区间；协议组内排序。</p>{card.channel.groups.map(group => <ChannelGroup key={group.protocolVersion ?? 'unknown'} group={group} flags={card.flags} />)}</section>
    </div>
    <EvidencePanel card={card} domain={domain} evidence={evidence} />
    <section className={styles.section}><h2>依据与局限</h2><ul>{card.limitations.map(text => <li key={text}>{text}</li>)}</ul>{card.exclusions.length ? <details><summary>查看资格排除</summary><ul>{card.exclusions.map(item => <li key={item.reason}>{item.reason}：<Metric value={item.comments} flags={card.flags} /> 条评论</li>)}</ul></details> : null}</section>
  </>;
}

export function OpportunityDetail({ domain, problem, home }: { domain: string; problem: string; home: string }) {
  const [evidenceUnavailable, setEvidenceUnavailable] = useState(false);
  const query = useQuery({ queryKey: ['opportunity', 'card', domain, problem], queryFn: ({ signal }) => readCard(domain, problem, signal), gcTime: 0, refetchOnWindowFocus: true });
  const refreshCard = () => { void query.refetch().then(result => { if (result.isSuccess) setEvidenceUnavailable(false); }); };
  if (query.isPending || query.isFetching) return <p role="status">正在读取机会卡…</p>;
  if (query.isError) return <div role="alert"><p>{readableError(query.error)}</p><button type="button" onClick={refreshCard}>重试</button> <Link to={home}>返回机会首页</Link></div>;
  if (evidenceUnavailable) return <div role="alert"><p>依据未能确认，已隐藏原声。请重新读取卡片。</p><button type="button" onClick={refreshCard}>重新读取卡片</button> <Link to={home}>返回机会首页</Link></div>;
  return <CardContent card={query.data} domain={domain} home={home} refreshCard={refreshCard} onEvidenceFailure={() => setEvidenceUnavailable(true)} />;
}
