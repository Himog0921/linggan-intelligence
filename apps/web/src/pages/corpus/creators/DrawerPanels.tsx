import { humanMoment } from '../../../shared/formatting';
import { Cover } from './Media';
import { JudgmentBlock } from './JudgmentBlock';
import {
  acquisitionText,
  analysisStatus,
  authorFocus,
  COEXISTENCE,
  CRITERIA,
  effectiveJudgement,
  failureReason,
  observedText,
  relevanceLabel,
  sampleBasisText,
  sampleText,
  workTitle,
} from './creatorsPresent';
import { LABELS, evidenceLink, formatCount, workLink, type LinkContext, type CreatorQuery } from './creatorsQuery';
import { Metric, type DrawerTab } from './CreatorTable';
import type { Analysis, CreatorItem, CreatorList, Work } from './creatorsTypes';
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
          {relevanceLabel(work.relevance)} · 赞 <Metric value={work.likes} /> · 藏 <Metric value={work.collects} /> · 评 <Metric value={work.comments} />
        </span>
      </div>
    </div>
  );
}

/** 概览：先给结论（作者在本领域的倾向），再给方向与表达、当前条件下的作品预览。 */
export function OverviewPanel({ ctx }: { ctx: PanelContext }) {
  const { item, data } = ctx;
  const analysis = item.authorAnalysis;
  const focus = authorFocus(item);
  return (
    <>
      <section aria-label="作者内容倾向">
        <h3>作者内容倾向</h3>
        <JudgmentBlock
          analysis={analysis}
          field="focus"
          judgement={focus}
          creatorKey={item.creatorKey}
          link={ctx.link}
          workRef={null}
          disabled={!ctx.writable}
          onCorrect={ctx.onCorrect}
          lead
        />
        <AnalysisLine analysis={analysis} value={focus.value} />
        <p className={styles.help}>
          {sampleText(analysis)}
          仅使用本领域已取得材料，不代表账号全部作品。
        </p>
      </section>
      <section aria-label="内容方向与作品表达">
        <h3>内容方向与作品表达</h3>
        <p>{item.topicHints.join(' · ') || '内容方向尚未形成'}</p>
        <div className={styles.tags}>
          {item.traits
            .filter(key => LABELS[key])
            .map(key => (
              <span key={key}>{LABELS[key]}</span>
            ))}
        </div>
        <p className={styles.help}>表达属于作品层面，可以同时出现，也与上面的作者倾向并存、互不推出。</p>
      </section>
      <section aria-label="当前条件的作品预览">
        <div className={styles.fieldHead}>
          <h3>当前条件的作品预览</h3>
          <button type="button" className={styles.textButton} onClick={() => ctx.onTab('evidence')}>
            查看依据 →
          </button>
        </div>
        {item.representatives.length === 0 ? <p className={styles.help}>暂无可用作品。</p> : item.representatives.map(work => <WorkPreview key={work.workRef} ctx={ctx} work={work} />)}
        <p className={styles.help}>预览最多三篇；不是本次作者分析的全部样本。</p>
        <a href={workLink(ctx.link, item.creatorKey)} data-return-key={item.creatorKey}>
          查看全部 {formatCount(item.matchedWorkRefs.length)} 篇命中作品 →
        </a>
      </section>
      <details>
        <summary>判断标准与材料范围</summary>
        <p className={styles.help}>{sampleBasisText(analysis)}。作者倾向基于本领域累计材料；当前列表作品按所选日期和条件。</p>
        <p className={styles.help}>
          已判相关 {formatCount(item.relatedWorkCount)} 篇，待判断 {formatCount(item.unknownRelevanceWorkCount)} 篇，已判无关 {formatCount(item.unrelatedWorkCount)} 篇。
        </p>
        <p className={styles.help}>
          {data.policy.likeThreshold === null
            ? '未设置爆款标准。'
            : `爆款最低点赞 ${formatCount(data.policy.likeThreshold)}；当前范围相关爆款 ${formatCount(item.viralWorkCount)} 篇。`}
        </p>
        <p className={styles.help}>{CRITERIA.focus}</p>
        <p className={styles.help}>{COEXISTENCE}</p>
      </details>
    </>
  );
}

/** 作品与依据：按作品分组，每篇作品的四项判断与各自的引文放在一起。 */
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
      <p className={styles.help}>
        范围内 {formatCount(item.collectedWorkCount)} 篇 · 当前条件命中 {formatCount(item.matchedWorkRefs.length)} 篇。下方预览最多三篇。
      </p>
      <a href={workLink(link, item.creatorKey)} data-return-key={item.creatorKey}>
        查看全部命中作品 →
      </a>
      {item.representatives.map(work => (
        <details
          key={work.workRef}
          className={styles.workGroup}
          data-work={work.workRef}
          open={expanded.has(work.workRef)}
          onToggle={event => onToggleWork(work.workRef, event.currentTarget.open)}
        >
          <summary>
            <Cover
              url={ctx.resources.byRef.get(work.workRef)?.coverUrl ?? null}
              pending={ctx.resources.pending}
              className={pageStyles.cover}
            />
            <div>
              <span className={styles.clamp} title={workTitle(work, ctx.resources)}>
                {workTitle(work, ctx.resources)}
              </span>
              <span className={styles.secondary}>
                {relevanceLabel(work.relevance)} · 赞 <Metric value={work.likes} />
              </span>
            </div>
          </summary>
          <div className={styles.workBody}>
            <p className={styles.help}>
              {acquisitionText(work)} · 收录于 {moment(work.firstAdded, work.firstAddedDisplay)} · 指标观察 {moment(work.likesObservedAt, work.likesObservedDisplay)}
            </p>
            <a href={evidenceLink(link, item.creatorKey, work.workRef)} data-return-key={item.creatorKey}>
              打开原作品 →
            </a>
            {(['relevance', 'personal_experience', 'professional_output', 'explicit_promotion'] as const).map(field => (
              <JudgmentBlock
                key={field}
                analysis={work.analysis}
                field={field}
                judgement={effectiveJudgement(work.analysis, field)}
                creatorKey={item.creatorKey}
                link={link}
                workRef={work.workRef}
                disabled={!ctx.writable}
                onCorrect={ctx.onCorrect}
              />
            ))}
            <AnalysisLine analysis={work.analysis} value={work.relevance} />
            {work.analysis.lastErrorCode ? (
              <details>
                <summary>处理信息</summary>
                <p className={styles.help}>
                  {failureReason(work.analysis.lastErrorCode)} <code>{work.analysis.lastErrorCode}</code>
                </p>
              </details>
            ) : null}
          </div>
        </details>
      ))}
    </>
  );
}

/** 作者资料：公开简介、公开身份线索、材料与来源；机器标识放在折叠的处理信息里。 */
export function ProfilePanel({ ctx }: { ctx: PanelContext }) {
  const { item, data, query } = ctx;
  const analysis = item.authorAnalysis;
  return (
    <>
      <section aria-label="公开简介">
        <h3>公开简介</h3>
        <p className={styles.biography}>{item.biography ?? '公开简介尚未取得。'}</p>
      </section>
      <section aria-label="公开身份线索">
        <h3>公开身份线索</h3>
        <JudgmentBlock
          analysis={analysis}
          field="institution_or_brand"
          judgement={effectiveJudgement(analysis, 'institution_or_brand')}
          creatorKey={item.creatorKey}
          link={ctx.link}
          workRef={null}
          disabled={!ctx.writable}
          onCorrect={ctx.onCorrect}
        />
        <p className={styles.help}>仅依据公开资料，不根据昵称、头像或粉丝量推断身份。</p>
      </section>
      <section aria-label="材料与来源">
        <h3>材料与来源</h3>
        <dl className={styles.profileList}>
          <dt>来源平台</dt>
          <dd>小红书</dd>
          <dt>当前领域</dt>
          <dd>{data.domain.name ?? '当前领域'}</dd>
          <dt>材料用途</dt>
          <dd>{query.usageRole === 'reference' ? '参照' : '主研究'}</dd>
          <dt>范围内作品</dt>
          <dd>{formatCount(item.collectedWorkCount)} 篇</dd>
          <dt>当前命中</dt>
          <dd>{formatCount(item.matchedWorkRefs.length)} 篇</dd>
          <dt>观察关系</dt>
          <dd>{observedText(item.observation)}</dd>
          <dt>实际取样</dt>
          <dd>{sampleText(analysis)}</dd>
        </dl>
      </section>
      <details>
        <summary>处理信息</summary>
        <p className={styles.help}>平台账号标识：{item.authorExternalId ?? '未取得'}</p>
        <p className={styles.help}>
          {analysis.resultAt || analysis.resultAtDisplay ? `自动依据更新于 ${moment(analysis.resultAt, analysis.resultAtDisplay)}` : '自动依据更新时间未记录'}
        </p>
        <p className={styles.help}>错误代码：{analysis.lastErrorCode ?? '未记录'}</p>
        <p className={styles.help}>来源采样：{analysis.sampleBasis ?? '未记录'}</p>
        {analysis.sampleLimitations.map(text => (
          <p key={text} className={styles.help}>
            {text}
          </p>
        ))}
      </details>
    </>
  );
}
