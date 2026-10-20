import type { RefObject } from 'react';
import { Avatar, Cover } from './Media';
import { analysisStatus, authorFocus, authorName, observedText, preservedLifecycle, relevanceLabel, targetHref, valueLabel, workTitle } from './creatorsPresent';
import {
  LABELS,
  formatCount,
  hasWorkFilter,
  highMode,
  scopeWorkLink,
  workLink,
  type CreatorQuery,
  type LinkContext,
  type SortKey,
} from './creatorsQuery';
import type { CreatorItem, CreatorList } from './creatorsTypes';
import { useJoinPending } from './useCreatorWrites';
import type { WorkResources } from './useWorkResources';
import styles from './CreatorsPage.module.css';

export type DrawerTab = 'overview' | 'evidence' | 'profile';

/** 数字单元：≥1 万缩写成「x.x万」，精确值始终在悬停里；未知是「—」，不是 0。 */
export function Metric({ value, className }: { value: number | null; className?: string }) {
  if (value === null) return <span className={className} title="未知">—</span>;
  const abbreviated = value >= 10_000 ? `${(value / 10_000).toFixed(1).replace(/\.0$/, '')}万` : formatCount(value);
  return (
    <span className={className} title={formatCount(value)}>
      {abbreviated}
    </span>
  );
}

interface CreatorTableProps {
  data: CreatorList;
  query: CreatorQuery;
  link: LinkContext;
  resources: WorkResources;
  selectedKey: string | null;
  analysisEnabled: boolean;
  domainActive: boolean;
  scrollRef: RefObject<HTMLDivElement | null>;
  busy: boolean;
  onOpen: (creatorKey: string, tab?: DrawerTab) => void;
  onJoin: (item: CreatorItem, role: 'primary' | 'reference') => void;
  onSort: (sort: SortKey) => void;
  onClearExtras: () => void;
}

export function CreatorTable(props: CreatorTableProps) {
  const { data, query, scrollRef, busy, onSort } = props;
  const filtered = hasWorkFilter(query);
  const high = highMode(query);
  return (
    <div ref={scrollRef} className={styles.tableWrap} tabIndex={0} role="region" aria-label="创作者表格，可横向滚动" aria-busy={busy}>
      <table className={styles.table}>
        <caption className="v7-sr-only">当前领域的创作者与作品判断、观察关系</caption>
        <colgroup>
          <col className={styles.colAuthor} />
          <col className={styles.colDirection} />
          <col className={styles.colFocus} />
          <col className={styles.colExpression} />
          <col className={styles.colCount} />
          <col className={styles.colLikes} />
          <col />
          <col className={styles.colObservation} />
        </colgroup>
        <thead>
          <tr>
            <th scope="col">创作者</th>
            <th scope="col">内容方向</th>
            <th scope="col" title="作者层面的倾向：本领域持续创作 / 多主题分享 / 尚不能判定">
              作者倾向
            </th>
            <th scope="col" title="作品层面的表达（亲历自述、知识与方法讲解、推广表达），可以同时出现，与作者倾向互不推出">
              作品表达
            </th>
            <th scope="col" className={styles.number} aria-sort={query.sort === 'related_works' ? 'descending' : undefined}>
              <button type="button" onClick={() => onSort('related_works')}>
                {filtered ? '命中相关' : '相关作品'}
              </button>
            </th>
            <th scope="col" className={styles.number} aria-sort={query.sort === 'high_likes' ? 'descending' : undefined}>
              <button type="button" onClick={() => onSort('high_likes')}>
                {high ? '命中最高赞' : '相关最高赞'}
              </button>
            </th>
            <th scope="col">作品预览</th>
            <th scope="col">观察状态</th>
          </tr>
        </thead>
        <tbody>
          {data.items.map(item => (
            <CreatorRow key={item.creatorKey} item={item} {...props} filtered={filtered} high={high} />
          ))}
        </tbody>
      </table>
      {data.total === 0 ? (
        <div className={styles.empty}>
          当前条件下没有找到创作者。
          <button type="button" className={styles.textButton} onClick={props.onClearExtras}>
            清除额外条件
          </button>
        </div>
      ) : null}
    </div>
  );
}

function CreatorRow(props: CreatorTableProps & { item: CreatorItem; filtered: boolean; high: boolean }) {
  const { item, link, resources, selectedKey, analysisEnabled, onOpen, filtered, high } = props;
  const work = item.representatives[0];
  const focus = authorFocus(item);
  const state = analysisStatus(item.authorAnalysis, focus.value, analysisEnabled);
  const traits = item.traits.filter(key => key !== 'institution_or_brand' && LABELS[key]);
  const matched = filtered ? item.matchedRelatedWorkCount : item.relatedWorkCount;
  const likes = high ? item.candidateHighLikeCount : item.maxRelatedLikeCount;
  const directions = item.topicHints.filter(Boolean).join(' · ');
  const resource = work ? resources.byRef.get(work.workRef) : undefined;
  const name = authorName(item);
  const nameSource = item.displayNameKind === 'observation_target' ? '观察目标名称 · ' : '';
  const matchedHref = filtered ? workLink(link, item.creatorKey, { relevance: 'related' }) : scopeWorkLink(link, item.creatorKey, 'related');
  return (
    <tr data-selected={selectedKey === item.creatorKey}>
      <td>
        <div className={`${styles.cell} ${styles.authorCell}`}>
          <Avatar name={name} url={resource?.avatarUrl ?? null} pending={resources.pending} className={styles.avatar} />
          <div className={styles.cellCopy}>
            <button
              type="button"
              className={`${styles.link} ${styles.name} ${styles.ellipsis}`}
              data-author-key={item.creatorKey}
              title={name}
              aria-haspopup="dialog"
              onClick={() => onOpen(item.creatorKey)}
            >
              {name}
            </button>
            <span className={styles.secondary}>
              {nameSource}粉丝 <Metric value={item.followerCount} />
            </span>
          </div>
        </div>
      </td>
      <td>
        <div className={styles.cell}>
          <button type="button" className={styles.link} title={directions || '内容方向尚未形成'} onClick={() => onOpen(item.creatorKey)}>
            <span className={styles.clamp}>{directions || '内容方向尚未形成'}</span>
          </button>
        </div>
      </td>
      <td>
        <div className={styles.cell}>
          <button type="button" className={`${styles.link} ${styles.ellipsis}`} title={valueLabel(focus.value)} onClick={() => onOpen(item.creatorKey)}>
            {valueLabel(focus.value)}
          </button>
          <span className={`${styles.secondary} ${styles.analysis}`} data-kind={state.kind} title={state.text}>
            {state.text}
          </span>
        </div>
      </td>
      <td>
        <div className={styles.cell}>
          <button
            type="button"
            className={styles.link}
            title={traits.map(key => LABELS[key]).join('、') || '暂无已确认表达'}
            onClick={() => onOpen(item.creatorKey, 'evidence')}
          >
            <span className={styles.expression}>
              <span>{traits.length > 0 ? LABELS[traits[0] ?? ''] : '—'}</span>
              {traits.length > 1 ? <small>+{traits.length - 1}</small> : null}
            </span>
          </button>
        </div>
      </td>
      <td className={styles.number}>
        <div className={styles.cell}>
          {matched > 0 ? (
            <a className={styles.metric} href={matchedHref} data-return-key={item.creatorKey}>
              {formatCount(matched)}
            </a>
          ) : (
            <span className={styles.metric}>{formatCount(matched)}</span>
          )}
          {filtered ? (
            <span className={styles.secondary}>范围内 {formatCount(item.relatedWorkCount)}</span>
          ) : item.unknownRelevanceWorkCount > 0 ? (
            <a className={styles.secondary} href={scopeWorkLink(link, item.creatorKey, 'unknown')} data-return-key={item.creatorKey}>
              待判断 {formatCount(item.unknownRelevanceWorkCount)}
            </a>
          ) : (
            <span className={styles.secondary}>篇相关作品</span>
          )}
        </div>
      </td>
      <td className={styles.number}>
        <div className={styles.cell}>
          <Metric value={likes} className={styles.metric} />
          <span className={styles.secondary}>{high ? '当前命中作品' : '已判相关作品'}</span>
        </div>
      </td>
      <td>
        <div className={styles.cell}>
          {work ? (
            <div className={styles.workCell}>
              <Cover url={resource?.coverUrl ?? null} pending={resources.pending} className={styles.cover} />
              <div className={styles.cellCopy}>
                <a
                  className={styles.clamp}
                  href={workLink(link, item.creatorKey, {}, work.workRef)}
                  data-return-key={item.creatorKey}
                  title={workTitle(work, resources)}
                >
                  {workTitle(work, resources)}
                </a>
                <span className={styles.secondary}>
                  {relevanceLabel(work.relevance)} · 赞 <Metric value={work.likes} />
                </span>
              </div>
            </div>
          ) : (
            <span className={styles.secondary}>暂无可用作品</span>
          )}
        </div>
      </td>
      <td>
        <div className={styles.cell}>
          <span className={styles.ellipsis} title={observedText(item.observation)}>
            {observedText(item.observation)}
          </span>
          <div className={styles.rowAction}>
            <RowAction {...props} />
          </div>
        </div>
      </td>
    </tr>
  );
}

function RowAction({ item, query, data, domainActive, onOpen, onJoin }: CreatorTableProps & { item: CreatorItem }) {
  const pending = useJoinPending(item.creatorKey);
  const observation = item.observation;
  if (observation.lookup === 'unavailable' || observation.inCurrentDomain === null) {
    return <span className={styles.secondary}>关系待读取</span>;
  }
  if (observation.inCurrentDomain) {
    return observation.targetRef ? <a href={targetHref(data.domain.domainRef, observation.targetRef)}>查看目标</a> : <span className={styles.secondary}>目标待核对</span>;
  }
  const blocked = query.usageRole === 'primary' && observation.hasOtherPrimaryDomain === true;
  if (blocked || preservedLifecycle(observation) !== null) {
    return (
      <button type="button" onClick={() => onOpen(item.creatorKey, 'profile')}>
        查看加入方式
      </button>
    );
  }
  return (
    <button type="button" disabled={!domainActive || pending} onClick={() => onJoin(item, query.usageRole)}>
      {pending ? '加入中…' : '加入观察'}
    </button>
  );
}
