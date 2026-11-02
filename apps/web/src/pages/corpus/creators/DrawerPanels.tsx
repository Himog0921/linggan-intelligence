import { useState } from 'react';
import { humanMoment } from '../../../shared/formatting';
import { IntelligenceIcon } from '../../../shared/IntelligenceIcon';
import { Cover } from './Media';
import { CreatorWorkResearch } from './CreatorWorkResearch';
import { CreatorAccountResearch } from './CreatorAccountResearch';
import { ExpandableText, JudgmentBlock, Quote } from './JudgmentBlock';
import {
  acquisitionText,
  analysisStatus,
  authorFocus,
  CRITERIA,
  effectiveJudgement,
  evidenceFor,
  failureReason,
  FIELD_NAMES,
  FIRST_SEEN_LABELS,
  lifecycleLabel,
  observedText,
  provenanceText,
  sampleBasisText,
  sampleText,
  targetHref,
  transitionReasonLabel,
  valueLabel,
  workTitle,
} from './creatorsPresent';
import { evidenceLink, formatCount, workLink, type CreatorQuery, type LinkContext } from './creatorsQuery';
import { Metric, type DrawerTab } from './CreatorTable';
import { useObservationRecord } from './useObservationRecord';
import type { Analysis, CreatorItem, CreatorList, Judgement, ObservationRecord, Work } from './creatorsTypes';
import type { WorkResources } from './useWorkResources';
import pageStyles from './CreatorsPage.module.css';
import styles from './CreatorDrawer.module.css';

export interface PanelContext {
  item: CreatorItem;
  data: CreatorList;
  query: CreatorQuery;
  link: LinkContext;
  resources: WorkResources;
  /** 领域已暂停时历史材料仍可读，写入操作不可用。 */
  writable: boolean;
  onCorrect: (field: string, workRef: string | null) => void;
  onTab: (tab: DrawerTab) => void;
}

/** 状态文字 + 色点：历史执行记录与已有结果分开表达，颜色不是唯一的载体。 */
function AnalysisLine({ analysis, value }: { analysis: Analysis; value: string | null | undefined }) {
  const status = analysisStatus(analysis, value);
  return (
    <p className={`${styles.help} ${styles.analysis}`} data-kind={status.kind}>
      {status.text}
    </p>
  );
}

function moment(raw: string | null, display: string | null): string {
  return raw ? humanMoment(raw) : (display ?? '未记录');
}

function WorkPreview({ ctx, work }: { ctx: PanelContext; work: Work }) {
  const { item, link, resources } = ctx;
  const title = workTitle(work, resources);
  return (
    <div className={styles.previewCard}>
      <Cover
        url={resources.byRef.get(work.workRef)?.coverUrl ?? null}
        pending={resources.pending}
        className={`${pageStyles.cover} ${pageStyles.coverLarge}`}
      />
      <div>
        <a className={styles.clamp} href={workLink(link, item.creatorKey, {}, work.workRef)} data-return-key={item.creatorKey} title={title}>
          {title}
        </a>
        <span className={styles.secondary}>
          {work.relevance === 'related' ? '已判相关' : work.relevance === 'unrelated' ? '已判无关' : '待判断'} · 赞 <Metric value={work.likes} />
        </span>
      </div>
    </div>
  );
}

/** 公开简介是事实：默认两行，长内容在原位展开。 */
function Biography({ text }: { text: string | null }) {
  const [open, setOpen] = useState(false);
  if (!text) return <p className={styles.help}>公开简介尚未取得。</p>;
  const long = text.length > 60 || text.includes('\n');
  return (
    <>
      <p className={styles.biography} data-open={open || !long}>{text}</p>
      {long ? (
        <button type="button" className={styles.textButton} onClick={() => setOpen(value => !value)}>
          {open ? '收起' : '展开'}
        </button>
      ) : null}
    </>
  );
}

/** 作者内容倾向：结论在前，理由、取样与支持片段在后；判定门槛放到统一的依据区。 */
function LeadJudgment({ ctx }: { ctx: PanelContext }) {
  const { item } = ctx;
  const analysis = item.authorAnalysis;
  const focus = authorFocus(item);
  const manual = analysis.manual.focus;
  const evidence = evidenceFor(analysis, 'focus', focus);
  const status = analysisStatus(analysis, focus.value);
  const statusVisible = analysis.state !== null && analysis.state !== 'idle';
  const sample = analysis.sampleWorkCount;
  return (
    <>
      <div className={styles.leadHead}>
        <strong className={styles.leadValue} data-value={focus.value}>{valueLabel(focus.value)}</strong>
        <button type="button" className={styles.textButton} disabled={!ctx.writable} aria-label="调整作者倾向" onClick={() => ctx.onCorrect('focus', null)}>
          {focus.value === 'unknown' ? '补充判断' : '调整判断'}
        </button>
      </div>
      {focus.reason ? (
        <ExpandableText text={focus.reason} />
      ) : (
        <p className={styles.leadReason}>当前材料尚不足以形成作者整体判断。</p>
      )}
      {statusVisible ? <p className={`${styles.help} ${styles.analysis}`} data-kind={status.kind}>{status.text}</p> : null}
      <div className={styles.leadMeta}>
        <details className={styles.inlineEvidence}>
          <summary>依据 {formatCount(evidence.length)} 片段</summary>
          {evidence.length > 0 ? (
            evidence.map(fragment => <Quote key={fragment.fragmentId} fragment={fragment} link={ctx.link} creatorKey={item.creatorKey} />)
          ) : (
            <p className={styles.help}>暂无可回查的支持片段。</p>
          )}
        </details>
        <span className={styles.sampleCount}>
          本次取样 {sample === null || !Number.isInteger(sample) ? '未记录' : `${formatCount(sample)} 篇`}
        </span>
      </div>
      <p className={styles.provenance}>
        {provenanceText(analysis, 'focus')}
        {manual?.supportUnavailable ? ' · 当时支持这项判断的材料已变化或不可读，请复核；原人工记录仍保留' : ''}
      </p>
    </>
  );
}

/** 概览：结论、公开简介、发现来源、领域作品；依据与身份线索收在统一的展开区。 */
export function OverviewPanel({ ctx }: { ctx: PanelContext }) {
  const { item, data } = ctx;
  const analysis = item.authorAnalysis;
  const firstSeenLabel = item.firstSeenPrecision ? FIRST_SEEN_LABELS[item.firstSeenPrecision] : undefined;
  const earliestWork = item.firstSeenWorkRef ? ctx.resources.byRef.get(item.firstSeenWorkRef) : undefined;
  const matched = item.matchedWorkRefs.length;
  return (
    <>
      <section aria-label="作者内容倾向">
        <div className={styles.sectionHead}>
          <h3>作者内容倾向</h3>
          {item.authorExternalId ? (
            <CreatorAccountResearch domain={data.domain.domainRef} platform={ctx.query.platform} author={item.authorExternalId} disabled={!ctx.writable} />
          ) : null}
        </div>
        <LeadJudgment ctx={ctx} />
      </section>

      <section aria-label="公开简介">
        <div className={styles.sectionHead}>
          <h3>公开简介</h3>
        </div>
        <Biography text={item.biography} />
      </section>

      <section aria-label="发现来源">
        <div className={styles.sectionHead}>
          <h3>发现来源</h3>
        </div>
        <p className={styles.factLine}>{item.discoveryKeywords.length > 0 ? item.discoveryKeywords.join('、') : '未记录发现词'}</p>
        <div className={styles.factRow}>
          <span className={styles.factMuted}>
            {item.firstSeenDisplay ?? '首次发现时间未记录'}
            {firstSeenLabel ? ` · ${firstSeenLabel}` : ''}
          </span>
          {item.firstSeenWorkRef ? (
            <a href={workLink(ctx.link, item.creatorKey, {}, item.firstSeenWorkRef)} data-return-key={item.creatorKey}>
              {earliestWork?.title ?? (ctx.resources.pending ? '首篇作品读取中…' : '首篇作品 →')}
            </a>
          ) : (
            <span className={styles.factMuted}>未记录最早收录的作品</span>
          )}
        </div>
      </section>

      <section aria-label="领域作品">
        <div className={styles.sectionHead}>
          <h3>领域作品</h3>
          <a href={workLink(ctx.link, item.creatorKey)} data-return-key={item.creatorKey}>
            查看全部 {formatCount(matched)} 篇 →
          </a>
        </div>
        {item.representatives.length === 0 ? (
          <p className={styles.help}>暂无可用作品。</p>
        ) : (
          item.representatives.map(work => <WorkPreview key={work.workRef} ctx={ctx} work={work} />)
        )}
      </section>

      <details className={styles.basis}>
        <summary>判断依据与材料范围</summary>
        <p className={styles.help}>{sampleBasisText(analysis)}。{sampleText(analysis)}</p>
        <p className={styles.help}>
          已判相关 {formatCount(item.relatedWorkCount)} 篇，待判断 {formatCount(item.unknownRelevanceWorkCount)} 篇，已判无关 {formatCount(item.unrelatedWorkCount)} 篇。
        </p>
        <p className={styles.help}>
          {data.policy.likeThreshold === null
            ? '未设置爆款标准。'
            : `爆款最低点赞 ${formatCount(data.policy.likeThreshold)}；当前范围相关爆款 ${item.viralWorkCount === null ? '未知' : `${formatCount(item.viralWorkCount)} 篇`}。`}
        </p>
        <p className={styles.help}><strong>作者在本领域的内容倾向：</strong>{CRITERIA.focus}</p>
        <div className={styles.identityBlock}>
          <h4>公开身份线索</h4>
          <JudgmentBlock
            analysis={analysis}
            field="institution_or_brand"
            judgement={effectiveJudgement(analysis, 'institution_or_brand')}
            creatorKey={item.creatorKey}
            link={ctx.link}
            workRef={null}
            disabled={!ctx.writable}
            onCorrect={ctx.onCorrect}
            actionLabel="调整公开身份线索"
          />
        </div>
      </details>
    </>
  );
}

function shortValue(field: string, value: string | null | undefined): string {
  if (field === 'relevance') {
    if (value === 'related') return '相关';
    if (value === 'unrelated') return '无关';
    return '未判断';
  }
  if (value === 'yes') return '有依据';
  if (value === 'no') return '未见';
  return '未判断';
}

function summaryLine(work: Work): string {
  return [
    `关系 ${shortValue('relevance', work.relevance)}`,
    `亲历 ${shortValue('personal_experience', effectiveJudgement(work.analysis, 'personal_experience').value)}`,
    `讲解 ${shortValue('professional_output', effectiveJudgement(work.analysis, 'professional_output').value)}`,
    `推广 ${shortValue('explicit_promotion', effectiveJudgement(work.analysis, 'explicit_promotion').value)}`,
  ].join(' · ');
}

const WORK_FIELDS = ['relevance', 'personal_experience', 'professional_output', 'explicit_promotion'] as const;

/** 一项判断的来源：人工调整各自保留时间；相同的自动更新时间收在作品层，只出现一次。 */
function workAttention(analysis: Analysis): string {
  const at = analysis.resultAt ?? analysis.resultAtDisplay;
  return at ? `自动依据更新于 ${humanMoment(at)}` : '自动依据更新时间未记录';
}

function WorkJudgment({ ctx, work, field }: { ctx: PanelContext; work: Work; field: string }) {
  const judgement: Judgement = effectiveJudgement(work.analysis, field);
  const manual = work.analysis.manual[field];
  const evidence = evidenceFor(work.analysis, field, judgement);
  const source = manual ? `${provenanceText(work.analysis, field)}${manual.supportUnavailable ? ' · 支持材料已变化，请复核' : ''}` : null;
  return (
    <div className={styles.judgmentRow}>
      <div className={styles.judgmentLine}>
        <span className={styles.judgmentLabel}>{FIELD_NAMES[field] ?? field}</span>
        <strong>{valueLabel(judgement.value)}</strong>
        {source ? <span className={styles.judgmentSource}>{source}</span> : null}
        <button type="button" className={styles.textButton} disabled={!ctx.writable} aria-label={`${judgement.value === 'unknown' ? '补充' : '调整'}${FIELD_NAMES[field] ?? field}`} onClick={() => ctx.onCorrect(field, work.workRef)}>
          {judgement.value === 'unknown' ? '补充判断' : '调整判断'}
        </button>
      </div>
      {judgement.reason ? <ExpandableText text={judgement.reason} /> : null}
      {evidence.length > 0 ? (
        <details className={styles.inlineEvidence}>
          <summary>依据 {formatCount(evidence.length)} 片段</summary>
          {evidence.map(fragment => (
            <Quote key={fragment.fragmentId} fragment={fragment} link={ctx.link} creatorKey={ctx.item.creatorKey} />
          ))}
        </details>
      ) : null}
    </div>
  );
}

/** 研究线索：每篇作品一行摘要；展开后才给判断、来源、引文与动作。 */
export function EvidencePanel({
  ctx,
  expanded,
  onToggleWork,
}: {
  ctx: PanelContext;
  expanded: ReadonlySet<string>;
  onToggleWork: (workRef: string, open: boolean) => void;
}) {
  const { item, link } = ctx;
  return (
    <>
      <div className={styles.panelHead}>
        <h3>当前命中 {formatCount(item.matchedWorkRefs.length)} 篇</h3>
        <a href={workLink(link, item.creatorKey)} data-return-key={item.creatorKey}>
          查看全部 →
        </a>
      </div>
      {item.representatives.length === 0 ? <p className={styles.help}>当前条件没有可展开的作品。</p> : null}
      {item.representatives.map(work => {
        const open = expanded.has(work.workRef);
        const failed = work.analysis.state === 'failed';
        const status = analysisStatus(work.analysis, work.relevance);
        const statusVisible =
          (work.analysis.state !== null && work.analysis.state !== 'idle') ||
          (work.analysis.resultAt !== null && status.kind === 'insufficient');
        return (
          <details
            key={work.workRef}
            className={styles.workGroup}
            data-work={work.workRef}
            open={open}
            onToggle={event => onToggleWork(work.workRef, event.currentTarget.open)}
          >
            <summary>
              <IntelligenceIcon name="chevron" expanded={open} className={styles.workChevron} />
              <div className={styles.workSummaryCopy}>
                <span className={styles.workSummaryTitle} title={workTitle(work, ctx.resources)}>{workTitle(work, ctx.resources)}</span>
                <span className={styles.workSummaryMeta} data-failed={failed || undefined}>
                  {failed ? `读取失败 · ${failureReason(work.analysis.lastErrorCode)}` : summaryLine(work)}
                </span>
              </div>
            </summary>
            <div className={styles.workBody}>
              <p className={styles.workSource}>
                {acquisitionText(work)} · 收录于 {moment(work.firstAdded, work.firstAddedDisplay)} · 指标观察 {moment(work.likesObservedAt, work.likesObservedDisplay)}
              </p>
              <div className={styles.workActions}>
                <a href={evidenceLink(link, item.creatorKey, work.workRef)} data-return-key={item.creatorKey}>
                  打开原作品 →
                </a>
                <CreatorWorkResearch domain={ctx.data.domain.domainRef} work={work.workRef} disabled={!ctx.writable} />
              </div>
              {statusVisible ? <AnalysisLine analysis={work.analysis} value={work.relevance} /> : null}
              <p className={styles.workProvenance}>{workAttention(work.analysis)}</p>
              {WORK_FIELDS.map(field => (
                <WorkJudgment key={field} ctx={ctx} work={work} field={field} />
              ))}
            </div>
          </details>
        );
      })}
      <details className={styles.criteriaBlock}>
        <summary>判断标准</summary>
        {WORK_FIELDS.map(field => (
          <p key={field} className={styles.help}>
            <strong>{FIELD_NAMES[field] ?? field}：</strong>
            {CRITERIA[field] ?? ''}
          </p>
        ))}
      </details>
    </>
  );
}

/** 本领域的观察记录：只展示接口已有事实，最近变化只取最近一次，不补造历史。 */
function ObservationRecordSection({ ctx }: { ctx: PanelContext }) {
  const { item, data } = ctx;
  const observation = item.observation;
  const joined = observation.inCurrentDomain === true;
  const record = useObservationRecord(data.domain.domainRef, item.creatorKey, joined);
  const stateText = observedText(observation);
  return (
    <section aria-label="观察记录">
      <div className={styles.observationHead}>
        <div>
          <h3>观察状态</h3>
          <strong className={styles.observationState}>{stateText}</strong>
        </div>
        {observation.targetRef ? <a href={targetHref(data.domain.domainRef, observation.targetRef)}>查看目标 →</a> : null}
      </div>
      {observation.lookup === 'unavailable' || observation.inCurrentDomain === null ? (
        <p className={styles.help}>观察记录暂不可用：现在读不到这位作者与本领域的关系。</p>
      ) : !joined ? (
        <p className={styles.help}>
          {observation.lifecycleState === 'paused' || observation.lifecycleState === 'dismissed'
            ? '这位作者在其他领域或过去的观察已暂停／忽略，但在本领域还没有观察关系，所以没有本领域的巡查记录。'
            : '未加入本领域观察，所以没有巡查记录。加入后才有巡查与带回结果。'}
        </p>
      ) : record.pending ? (
        <p className={styles.help}>观察记录读取中…</p>
      ) : record.failed || record.record === null ? (
        <p className={styles.help}>观察记录暂不可用：这次没有读到。</p>
      ) : record.record.state !== 'joined' ? (
        <p className={styles.help}>服务器上已经没有这位作者与本领域的观察关系。</p>
      ) : (
        <ObservationRecordFacts record={record.record} />
      )}
    </section>
  );
}

function ObservationRecordFacts({ record }: { record: ObservationRecord }) {
  const patrol = record.patrol;
  const role = record.usageRole === 'reference' ? '参照' : record.usageRole === 'primary' ? '主研究' : '用途未记录';
  const status = [
    `${lifecycleLabel(record.lifecycleState)}${record.lifecycleChangedAt ? `（${record.lifecycleChangedAt} 起）` : ''}`,
    record.monitoringState === 'monitoring'
      ? '自动巡查已开启'
      : record.monitoringState === 'not_enabled'
        ? '自动巡查未开启'
        : null,
  ]
    .filter(Boolean)
    .join(' · ');
  const lastPatrol =
    patrol === null || patrol.state === 'unavailable'
      ? '暂不可用'
      : patrol.lastSucceededAt
        ? `成功于 ${patrol.lastSucceededAt}`
        : patrol.lastDispatchedAt
          ? `派出于 ${patrol.lastDispatchedAt}，还没有成功结果`
          : '没有巡查记录';
  const patrolResult =
    patrol === null || patrol.state === 'unavailable'
      ? '暂不可用'
      : patrol.latestHits !== null
        ? `${formatCount(patrol.latestHits)} 篇${patrol.latestNew !== null ? `，其中 ${formatCount(patrol.latestNew)} 篇新见` : ''}`
        : patrol.lastSucceededAt
          ? '作品数未记录'
          : '还没有成功巡查的结果';
  const next =
    patrol === null || patrol.state === 'unavailable'
      ? '暂不可用'
      : patrol.nextRunAt ?? (patrol.automaticEnabled ? '尚未排定' : '自动巡查未开启');
  const change = record.lastTransition
    ? record.lastTransition.from
      ? `${record.lastTransition.at} 从「${lifecycleLabel(record.lastTransition.from)}」到「${lifecycleLabel(record.lastTransition.to)}」：${transitionReasonLabel(record.lastTransition.reasonCode, record.lastTransition.reason)}`
      : `${record.lastTransition.at} 建立为「${lifecycleLabel(record.lastTransition.to)}」：${transitionReasonLabel(record.lastTransition.reasonCode, record.lastTransition.reason)}`
    : '没有变化记录';
  return (
    <>
      <dl className={styles.observationFacts}>
        <div>
          <dt>加入本领域</dt>
          <dd>{record.joinedAt ?? '时间未记录'} · {role}</dd>
        </div>
        <div>
          <dt>上次巡查</dt>
          <dd>{lastPatrol}</dd>
        </div>
        <div>
          <dt>带回作品</dt>
          <dd>{patrolResult}</dd>
        </div>
        <div>
          <dt>下次巡查</dt>
          <dd>{next}</dd>
        </div>
      </dl>
      <div className={styles.lastChange}>
        <h4>最近一次变化</h4>
        <p>{change}</p>
      </div>
      <details className={styles.archiveInfo}>
        <summary>更多档案信息</summary>
        <dl className={styles.observationFacts}>
          <div>
            <dt>首次保存</dt>
            <dd>{record.firstStoredAt ?? '未记录'}</dd>
          </div>
          <div>
            <dt>当前状态</dt>
            <dd>{status}</dd>
          </div>
          {patrol !== null && patrol.missedRuns > 0 ? (
            <div>
              <dt>错过计划</dt>
              <dd>{formatCount(patrol.missedRuns)} 次</dd>
            </div>
          ) : null}
        </dl>
      </details>
    </>
  );
}

/** 观察记录只承担观察与巡查；公开简介、身份线索与材料范围都在概览。 */
export function ProfilePanel({ ctx }: { ctx: PanelContext }) {
  return <ObservationRecordSection ctx={ctx} />;
}
