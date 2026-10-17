import { FactGrid, SourceNote, StateLine, Tech, Section } from './Primitives';
import {
  LANE_LABELS,
  LANE_ORDER,
  currentMetricRow,
  detailFieldRow,
  knownText,
  laneSummary,
  limitationCopy,
  momentText,
  monitoringOriginCopy,
  publishedCopy,
  stoppedReasonMeaning,
  timelineRuns,
  type FactRowData,
} from './evidencePresent';
import type { AuthorContext, Inspector, WorkItem } from './evidenceTypes';
import styles from './Inspector.module.css';

function IdentityFacts({ item }: { item: WorkItem }) {
  const display = item.display;
  const limitation = item.primaryLimitation;
  const rows: FactRowData[] = [
    ['稳定引用', item.ref],
    ['平台', item.platform || '当前未知'],
    ['标题', knownText(display.title, display.titleState), display.titleState],
    ['发布时间', publishedCopy(display, { withFieldName: false }), display.publishedAtState],
    ['时间来源字段', display.publishedAtSourceField ?? '当前未知', display.publishedAtSourceKind],
    ['时间精度', display.publishedAtPrecision, display.publishedAtParserVersion],
    ['最近观察', momentText(item.lastObservedAt)],
    // 读模型把「当前主要限制」写死成同一个默认值，不等价于「只跑过发现通道」：只在这里列出，不作标题结论（DR-8）。
    ['当前主要限制', limitation && limitation !== 'NONE' ? limitationCopy(limitation) : '本次读取没有记录额外限制', limitation],
  ];
  return <FactGrid rows={rows} />;
}

function PlatformFacts({ item, inspector }: { item: WorkItem; inspector: Inspector }) {
  const engagement = item.display.engagement;
  const comments = inspector.commentsCoverage;
  const stopped = comments?.stoppedReason ?? null;
  const rows: FactRowData[] = [
    ['平台点赞', knownText(engagement.like.value, engagement.like.state), engagement.like.state],
    ['平台评论数', knownText(engagement.comment.value, engagement.comment.state), engagement.comment.state],
    ['平台收藏', knownText(engagement.collect.value, engagement.collect.state), engagement.collect.state],
    ['平台分享', knownText(engagement.share.value, engagement.share.state), engagement.share.state],
    ['本地留存评论', comments?.retained ?? '当前未知'],
    ['已去重评论身份', comments?.uniqueCollectedCount ?? '当前未知'],
    // 停止原因：中文含义在前，原始码作旁注——页面不再直接显示 `risk_control` 这种生产方枚举（H12）。
    ['本地留存边界', stopped ? `停止原因：${stoppedReasonMeaning(stopped)}` : '当前未表达', stopped ?? comments?.collectionState ?? 'UNKNOWN'],
  ];
  return <FactGrid rows={rows} />;
}

function AuthorFacts({ context, shared }: { context: AuthorContext | null; shared: boolean }) {
  if (context === null) return <SourceNote>当前详情没有合格作者上下文；这不表示作者不存在。</SourceNote>;
  return (
    <FactGrid
      rows={[
        ['显示名', knownText(context.displayName, context.displayNameState), context.displayNameState],
        ['简介', context.biographyState === 'KNOWN' ? '已形成状态化上下文' : '当前未知', context.biographyState],
        ['关注者数', context.followerCountState === 'KNOWN' && context.followerCount !== null ? context.followerCount : '当前未知', context.followerCountState],
        ['观察时间', momentText(context.observedAt)],
        ...(shared ? ([['范围', '共享的当前作者上下文']] as FactRowData[]) : []),
      ]}
    />
  );
}

function Timeline({ inspector }: { inspector: Inspector }) {
  const runs = timelineRuns(inspector.timeline);
  return (
    <Section title="互动数据观察时间线">
      {runs.length === 0 ? (
        <SourceNote>当前详情没有互动数据观察时点；不把未知写成 0。</SourceNote>
      ) : (
        <div className={styles.runs}>
          {runs.map((run, index) => (
            <article key={index} className={styles.run}>
              <strong>{run.count === 1 ? run.first : `${run.first} — ${run.last}`}</strong>
              {run.count > 1 ? <span className={styles.inlineNote}>{`连续 ${run.count} 次观察结果相同`}</span> : null}
              <p>{run.copy}</p>
              <Tech>{run.lane}</Tech>
            </article>
          ))}
        </div>
      )}
    </Section>
  );
}

/**
 * 「技术事实」折叠区（默认收起，一项不删）：稳定引用、时间来源与精度、当前主要限制、互动当前值与变化、详情字段来源、
 * 平台计数与本地留存、材料通道状态、互动观察时间线、作者上下文。字段码与 UUID 只出现在这里；
 * 折叠是排布，不是改写——每一行的原文案与状态码与旧页一致。
 */
export function TechnicalFacts({ item, inspector }: { item: WorkItem; inspector: Inspector }) {
  const detail = inspector.detail;
  const metrics = inspector.engagement;
  const author = inspector.authorContext ?? inspector.sharedAuthorContext;
  return (
    <details className={styles.technical}>
      <summary>技术事实</summary>
      <Section title="作品字段与来源">
        <IdentityFacts item={item} />
      </Section>
      <Section title="来源与溯源">
        <FactGrid rows={[['来源监控', monitoringOriginCopy(item)]]} />
      </Section>
      <Section title="当前互动状态与变化">
        <FactGrid rows={[currentMetricRow('点赞', metrics.likeCount), currentMetricRow('评论', metrics.commentCount), currentMetricRow('收藏', metrics.collectCount), currentMetricRow('分享', metrics.shareCount)]} />
      </Section>
      <Section title="详情字段当前事实">
        <FactGrid rows={[detailFieldRow('标题来源', detail.title), detailFieldRow('作者来源', detail.creator), detailFieldRow('发布时间来源', detail.publishedAt), detailFieldRow('正文字段', detail.body, 'body')]} />
      </Section>
      <Section title="平台计数与本地留存">
        <PlatformFacts item={item} inspector={inspector} />
      </Section>
      <Section title="材料通道状态">
        <div className={styles.lanes}>
          {LANE_ORDER.map(lane => {
            const summary = laneSummary(item.lanes, lane);
            return (
              <div key={lane} className={styles.lane}>
                <span className={styles.laneName}>{LANE_LABELS[lane]}</span>
                <span className={styles.laneCount}>{summary?.valueState === 'KNOWN' && summary.retained !== null ? `保留 ${summary.retained}` : '数量未知'}</span>
                <StateLine state={summary?.state} />
                <Tech>{summary?.state ?? 'UNKNOWN'}</Tech>
              </div>
            );
          })}
        </div>
      </Section>
      <Timeline inspector={inspector} />
      <Section title={inspector.sharedAuthorContext && !inspector.authorContext ? '共享作者上下文（当前）' : '作者上下文'}>
        <AuthorFacts context={author} shared={inspector.authorContext === null && inspector.sharedAuthorContext !== null} />
      </Section>
    </details>
  );
}
