import { useEffect, useRef, type RefObject } from 'react';
import { Avatar } from './Media';
import { authorName, FIRST_SEEN_LABELS, observedText, preservedLifecycle, targetHref, workTitle } from './creatorsPresent';
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
import { useTitleTooltip } from '../evidence/TitleTooltip';
import { useJoinPending } from './useCreatorWrites';
import { Metric } from './Metric';
import type { WorkResources } from './useWorkResources';
import { WorkPreviewCell } from './WorkPreviewTip';
import styles from './CreatorsPage.module.css';

export type DrawerTab = 'overview' | 'works' | 'evidence' | 'profile';

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

/** 截断文本的完整内容：悬停约 250ms、键盘聚焦立即可得，Esc 先关提示。 */
function TipText({ text, className, label, tabIndex }: { text: string; className: string; label?: string; tabIndex?: number }) {
  const { anchorRef, onEnter, onLeave, onFocus, onBlur, tooltip } = useTitleTooltip<HTMLSpanElement>(text, 'width', { delay: 250, maxWidth: 320 });
  return (
    <span
      ref={anchorRef}
      className={className}
      tabIndex={tabIndex}
      aria-label={label}
      onMouseEnter={onEnter}
      onMouseLeave={onLeave}
      onFocus={onFocus}
      onBlur={onBlur}
    >
      {text}
      {tooltip}
    </span>
  );
}

/** 「+N」展开的是同一批已有主题，不生成新主题。 */
function MoreTopics({ topics }: { topics: string[] }) {
  const text = topics.join('、');
  const { anchorRef, onEnter, onLeave, onFocus, onBlur, tooltip } = useTitleTooltip<HTMLButtonElement>(text, 'width', { always: true, delay: 250, maxWidth: 320 });
  return (
    <button
      ref={anchorRef}
      type="button"
      className={styles.topicMore}
      aria-label={`其余 ${topics.length} 个主题：${text}`}
      onMouseEnter={onEnter}
      onMouseLeave={onLeave}
      onFocus={onFocus}
      onBlur={onBlur}
    >
      +{formatCount(topics.length)}
      {tooltip}
    </button>
  );
}

function Topics({ hints }: { hints: string[] }) {
  const topics = hints.filter(Boolean);
  if (topics.length === 0) return <span className={styles.secondary}>—</span>;
  const shown = topics.slice(0, 2);
  const rest = topics.slice(2);
  return (
    <span className={styles.topicTags}>
      {shown.map(topic => <span key={topic} className={styles.topicTag}>{topic}</span>)}
      {rest.length > 0 ? <MoreTopics topics={rest} /> : null}
    </span>
  );
}

function HeaderInfo({ text }: { text: string }) {
  const { anchorRef, onEnter, onLeave, onFocus, onBlur, tooltip } = useTitleTooltip<HTMLSpanElement>(text, 'width', { always: true, delay: 250, maxWidth: 320 });
  return (
    <span
      ref={anchorRef}
      className={styles.headInfo}
      tabIndex={0}
      role="note"
      aria-label={text}
      onMouseEnter={onEnter}
      onMouseLeave={onLeave}
      onFocus={onFocus}
      onBlur={onBlur}
    >
      {tooltip}
    </span>
  );
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
          <col className={styles.colRelated} />
          <col className={styles.colLikes} />
          <col className={styles.colWork} />
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
            <th scope="col" className={styles.number} aria-label={filtered ? '命中最高赞' : '领域最高赞'} aria-sort={query.sort === 'high_likes' ? 'descending' : undefined}>
              <span className={styles.headInfoWrap}>
                <button type="button" onClick={() => onSort('high_likes')}>
                  {filtered ? '命中最高赞' : '领域最高赞'}
                </button>
                <HeaderInfo text={filtered ? '当前命中作品中点赞最高的一篇（含待判定）' : '本领域已取得作品中点赞最高的一篇（含待判定，不含已判无关）'} />
              </span>
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
          {data.items.map(item => <CreatorRow key={item.creatorKey} item={item} {...props} filtered={filtered} />)}
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

function CreatorName({ item, onOpen }: { item: CreatorItem; onOpen: (creatorKey: string) => void }) {
  const name = authorName(item);
  const { anchorRef, onEnter, onLeave, onFocus, onBlur, onPointerDown, tooltip } = useTitleTooltip<HTMLButtonElement>(name, 'lines', { delay: 250, maxWidth: 320 });
  return (
    <button
      ref={anchorRef}
      type="button"
      className={`${styles.link} ${styles.name}`}
      data-author-key={item.creatorKey}
      onMouseEnter={onEnter}
      onMouseLeave={onLeave}
      onFocus={onFocus}
      onBlur={onBlur}
      onPointerDown={onPointerDown}
      onClick={() => onOpen(item.creatorKey)}
    >
      {name}
      {tooltip}
    </button>
  );
}

function CreatorRow(props: CreatorTableProps & { item: CreatorItem; filtered: boolean }) {
  const { item, link, resources, selectedKey, selectedKeys, onOpen, onToggleSelect, filtered } = props;
  const work = item.representatives[0];
  const matched = filtered ? item.matchedRelatedWorkCount : item.relatedWorkCount;
  const name = authorName(item);
  const keyword = item.discoveryKeywords[0] ?? '';
  const nameSource = item.displayNameKind === 'observation_target' ? '观察目标名称 · ' : '';
  const matchedHref = filtered ? workLink(link, item.creatorKey, { relevance: 'related' }) : scopeWorkLink(link, item.creatorKey, 'related');
  const firstSeenLabel = item.firstSeenPrecision ? FIRST_SEEN_LABELS[item.firstSeenPrecision] : undefined;
  const date = item.firstSeenDate ?? item.firstSeenDisplay;
  const state = observedText(item.observation);
  const lifecycle = item.observation.lifecycleState;
  const stateKind = item.observation.lookup === 'unavailable' || item.observation.inCurrentDomain === null
    ? 'unknown'
    : item.observation.inCurrentDomain
      ? 'joined'
      : lifecycle === 'paused'
        ? 'paused'
        : lifecycle === 'dismissed'
          ? 'dismissed'
          : 'none';
  return (
    <tr data-open={selectedKey === item.creatorKey} data-batch={selectedKeys.has(item.creatorKey)}>
      <td>
        <div className={`${styles.cell} ${styles.authorCell}`}>
          <input
            type="checkbox"
            className={styles.selectBox}
            checked={selectedKeys.has(item.creatorKey)}
            onChange={event => onToggleSelect(item, event.target.checked)}
            aria-label={`选择 ${name}`}
          />
          <Avatar name={name} url={resources.byRef.get(work?.workRef ?? '')?.avatarUrl ?? null} pending={resources.pending} className={styles.avatar} />
          <div className={styles.cellCopy}>
            <CreatorName item={item} onOpen={onOpen} />
            <span className={styles.secondary}>
              {nameSource}粉丝 <Metric value={item.followerCount} />
            </span>
          </div>
        </div>
      </td>
      <td>
        <div className={styles.cell}>
          <TipText
            text={keyword || '未记录发现词'}
            className={`${styles.ellipsis} ${keyword ? styles.keyword : styles.secondary}`}
            label={item.discoveryKeywords.length > 1 ? item.discoveryKeywords.join('、') : undefined}
            tabIndex={keyword.length > 10 ? 0 : undefined}
          />
        </div>
      </td>
      <td>
        <div className={styles.cell}>
          <Topics hints={item.topicHints} />
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
          {item.unknownRelevanceWorkCount > 0 ? (
            <a className={styles.secondary} href={scopeWorkLink(link, item.creatorKey, 'unknown')} data-return-key={item.creatorKey}>
              待判断 {formatCount(item.unknownRelevanceWorkCount)}
            </a>
          ) : filtered ? (
            <span className={styles.secondary}>范围内 {formatCount(item.relatedWorkCount)}</span>
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
            <WorkPreviewCell
              work={work}
              title={workTitle(work, resources)}
              creatorKey={item.creatorKey}
              link={link}
              resources={resources}
            />
          ) : (
            <span className={styles.secondary}>暂无可用作品</span>
          )}
        </div>
      </td>
      <td>
        <div className={styles.cell}>
          {date ? (
            <>
              <span className={styles.nowrap}>{date}</span>
              {firstSeenLabel ? (
                <span className={item.firstSeenPrecision === 'recent' ? styles.recentMark : styles.secondary}>{firstSeenLabel}</span>
              ) : null}
            </>
          ) : (
            <span className={styles.secondary}>—</span>
          )}
        </div>
      </td>
      <td>
        <div className={styles.cell}>
          <span className={styles.stateTag} data-state={stateKind}>{state}</span>
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
