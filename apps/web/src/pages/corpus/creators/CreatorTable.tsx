import { useEffect, useRef, type RefObject } from 'react';
import { Avatar, Cover } from './Media';
import { authorName, observedText, preservedLifecycle, relevanceLabel, targetHref, workTitle } from './creatorsPresent';
import {
  formatCount,
  hasWorkFilter,
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

export type DrawerTab = 'overview' | 'works' | 'evidence' | 'profile';

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

/** 首次发现的两种补充说明：更早的发现不可知，或最近 7 天才见到。 */
const FIRST_SEEN_LABELS: Record<string, string> = {
  recent: '近 7 天新见',
  earliest_record: '最早已知记录',
  domain_first: '本领域首次',
};

/** `2026-09-12` → `09.12`；格式不对时原样显示，不改写。 */
function shortDay(date: string): string {
  const [, month, day] = date.split('-');
  return month && day ? `${month}.${day}` : date;
}

interface CreatorTableProps {
  data: CreatorList;
  query: CreatorQuery;
  link: LinkContext;
  resources: WorkResources;
  selectedKey: string | null;
  selectedKeys: ReadonlySet<string>;
  domainActive: boolean;
  scrollRef: RefObject<HTMLDivElement | null>;
  busy: boolean;
  onOpen: (creatorKey: string, tab?: DrawerTab) => void;
  onJoin: (item: CreatorItem, role: 'primary' | 'reference') => void;
  onSort: (sort: SortKey) => void;
  onClearExtras: () => void;
  onToggleSelect: (item: CreatorItem, checked: boolean) => void;
  onSelectPage: (checked: boolean) => void;
}

export function CreatorTable(props: CreatorTableProps) {
  const { data, query, scrollRef, busy, onSort, selectedKeys, onSelectPage } = props;
  const filtered = hasWorkFilter(query);
  const allSelected = data.items.length > 0 && data.items.every(item => selectedKeys.has(item.creatorKey));
  const someSelected = data.items.some(item => selectedKeys.has(item.creatorKey));
  return (
    <div ref={scrollRef} className={styles.tableWrap} tabIndex={0} role="region" aria-label="创作者表格，可横向滚动" aria-busy={busy}>
      <table className={styles.table}>
        <caption className="v7-sr-only">当前领域的创作者、发现来源、作品判断与观察关系</caption>
        <colgroup>
          <col className={styles.colAuthor} />
          <col className={styles.colKeyword} />
          <col className={styles.colDirection} />
          <col className={styles.colCount} />
          <col className={styles.colLikes} />
          <col />
          <col className={styles.colFirstSeen} />
          <col className={styles.colObservation} />
        </colgroup>
        <thead>
          <tr>
            <th scope="col">
              <span className={styles.headWithSelect}>
                <PageSelect all={allSelected} some={someSelected} onChange={onSelectPage} />
                创作者
              </span>
            </th>
            <th scope="col">发现词</th>
            <th scope="col">内容方向</th>
            <th scope="col" className={styles.number} aria-sort={query.sort === 'related_works' ? 'descending' : undefined}>
              <button type="button" onClick={() => onSort('related_works')}>
                {filtered ? '命中相关' : '相关作品'}
              </button>
            </th>
            <th
              scope="col"
              className={styles.number}
              aria-sort={query.sort === 'high_likes' ? 'descending' : undefined}
              title={filtered ? '当前命中作品中点赞最高的一篇（含待判定）' : '本领域已取得作品中点赞最高的一篇（含待判定，不含已判无关）'}
            >
              <button type="button" onClick={() => onSort('high_likes')}>
                {filtered ? '命中最高赞' : '领域最高赞'}
              </button>
            </th>
            <th scope="col">作品预览</th>
            <th scope="col" aria-sort={query.sort === 'first_seen' ? 'descending' : undefined}>
              <button type="button" onClick={() => onSort('first_seen')}>
                首次发现
              </button>
            </th>
            <th scope="col">观察状态</th>
          </tr>
        </thead>
        <tbody>
          {data.items.map(item => (
            <CreatorRow key={item.creatorKey} item={item} {...props} filtered={filtered} />
          ))}
        </tbody>
      </table>
      {data.total === 0 ? (
        <div className={styles.empty}>
          {query.firstSeenFrom !== ''
            ? `所选时间内没有新发现的作者。${data.stats.latestAddedDisplay ? `最近入库 ${data.stats.latestAddedDisplay}。` : '最近入库时间未知。'}`
            : '当前条件下没有找到创作者。'}
          <button type="button" className={styles.textButton} onClick={props.onClearExtras}>
            清除额外条件
          </button>
        </div>
      ) : null}
    </div>
  );
}

/** 当前页全选：「部分已选」用半选态表达，不假装全都选中。 */
function PageSelect({ all, some, onChange }: { all: boolean; some: boolean; onChange: (checked: boolean) => void }) {
  const ref = useRef<HTMLInputElement>(null);
  useEffect(() => {
    if (ref.current) ref.current.indeterminate = some && !all;
  }, [some, all]);
  return (
    <input
      ref={ref}
      type="checkbox"
      className={styles.selectBox}
      checked={all}
      onChange={event => onChange(event.target.checked)}
      aria-label="选择当前页全部创作者"
    />
  );
}

function CreatorRow(props: CreatorTableProps & { item: CreatorItem; filtered: boolean }) {
  const { item, link, resources, selectedKey, selectedKeys, onOpen, onToggleSelect, filtered } = props;
  const work = item.representatives[0];
  const matched = filtered ? item.matchedRelatedWorkCount : item.relatedWorkCount;
  const directions = item.topicHints.filter(Boolean).join(' · ');
  const resource = work ? resources.byRef.get(work.workRef) : undefined;
  const name = authorName(item);
  const nameSource = item.displayNameKind === 'observation_target' ? '观察目标名称 · ' : '';
  const matchedHref = filtered ? workLink(link, item.creatorKey, { relevance: 'related' }) : scopeWorkLink(link, item.creatorKey, 'related');
  const firstSeenLabel = item.firstSeenPrecision ? FIRST_SEEN_LABELS[item.firstSeenPrecision] : undefined;
  return (
    <tr data-selected={selectedKey === item.creatorKey || selectedKeys.has(item.creatorKey)}>
      <td>
        <div className={`${styles.cell} ${styles.authorCell}`}>
          <input
            type="checkbox"
            className={styles.selectBox}
            checked={selectedKeys.has(item.creatorKey)}
            onChange={event => onToggleSelect(item, event.target.checked)}
            aria-label={`选择 ${name}`}
          />
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
          <span
            className={`${styles.ellipsis} ${item.discoveryKeywords.length > 0 ? styles.keyword : styles.secondary}`}
            title={item.discoveryKeywords.length > 0 ? item.discoveryKeywords.join('、') : '未记录发现词'}
          >
            {item.discoveryKeywords[0] ?? '未记录发现词'}
          </span>
        </div>
      </td>
      <td>
        <div className={styles.cell}>
          <button type="button" className={styles.link} title={directions || '内容方向尚未形成'} onClick={() => onOpen(item.creatorKey)}>
            <span className={styles.clamp}>{directions || '内容方向尚未形成'}</span>
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
          ) : null}
        </div>
      </td>
      <td className={styles.number}>
        <div className={styles.cell}>
          <Metric value={item.candidateHighLikeCount} className={styles.metric} />
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
          {item.firstSeenDate ? (
            <>
              <span className={styles.ellipsis} title={item.firstSeenDisplay ?? item.firstSeenDate}>
                {shortDay(item.firstSeenDate)}
              </span>
              {firstSeenLabel ? (
                <span className={item.firstSeenPrecision === 'recent' ? styles.recentMark : styles.secondary}>{firstSeenLabel}</span>
              ) : null}
            </>
          ) : (
            <span className={styles.secondary}>首次发现时间未记录</span>
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
