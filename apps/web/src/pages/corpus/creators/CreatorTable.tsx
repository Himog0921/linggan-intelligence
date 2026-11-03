import { useEffect, useRef, type ReactNode, type RefObject } from 'react';
import { Avatar } from './Media';
import { analysisStatus, authorName, FIRST_SEEN_LABELS, targetHref, workTitle } from './creatorsPresent';
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

/** 发现词只显示第一个；多个时完整列表悬停可见、键盘可聚焦取得。 */
function KeywordCell({ item }: { item: CreatorItem }) {
  const list = item.discoveryKeywords;
  const keyword = list[0] ?? '';
  const all = list.join('、');
  const { anchorRef, onEnter, onLeave, onFocus, onBlur, tooltip } = useTitleTooltip<HTMLSpanElement>(all || '未记录发现词', 'width', {
    always: list.length > 1,
    delay: 250,
    maxWidth: 320,
  });
  return (
    <span
      ref={anchorRef}
      className={`${styles.ellipsis} ${keyword ? styles.keyword : styles.secondary}`}
      tabIndex={list.length > 1 ? 0 : undefined}
      aria-label={list.length > 1 ? all : undefined}
      onMouseEnter={onEnter}
      onMouseLeave={onLeave}
      onFocus={onFocus}
      onBlur={onBlur}
    >
      {keyword || '未记录发现词'}
      {tooltip}
    </span>
  );
}

/**
 * 单行单元格里的按需说明：可见内容只有一行，完整说明放在悬停/键盘提示里。
 * 内部有链接时由链接的聚焦带出提示，不额外增加 Tab 停靠点。
 */
function TipWrap({ text, children, focusable = false, className }: { text: string; children: ReactNode; focusable?: boolean; className?: string }) {
  const { anchorRef, onEnter, onLeave, onFocus, onBlur, tooltip } = useTitleTooltip<HTMLSpanElement>(text, 'width', { always: true, delay: 250, maxWidth: 320 });
  return (
    <span
      ref={anchorRef}
      className={className}
      tabIndex={focusable ? 0 : undefined}
      onMouseEnter={onEnter}
      onMouseLeave={onLeave}
      onFocus={onFocus}
      onBlur={onBlur}
    >
      {children}
      {tooltip}
    </span>
  );
}

/** 内容方向只占一行：显示第一个主题，其余数量与完整列表按需提示；读取失败不写成「—」。 */
function Topics({ item }: { item: CreatorItem }) {
  const topics = item.topicHints.filter(Boolean);
  const text = topics.join('、');
  const { anchorRef, onEnter, onLeave, onFocus, onBlur, tooltip } = useTitleTooltip<HTMLSpanElement>(text || '无内容方向', 'width', {
    always: topics.length > 1,
    delay: 250,
    maxWidth: 320,
  });
  if (topics.length === 0) {
    if (item.authorAnalysis.state === 'failed') {
      return (
        <TipWrap text={analysisStatus(item.authorAnalysis, item.focus.value).text} focusable>
          <span className={styles.secondary}>研究未完成</span>
        </TipWrap>
      );
    }
    return <span className={styles.secondary}>—</span>;
  }
  const [first, ...rest] = topics;
  return (
    <span
      ref={anchorRef}
      className={styles.topicTags}
      tabIndex={rest.length > 0 ? 0 : undefined}
      aria-label={rest.length > 0 ? `内容方向：${text}` : undefined}
      onMouseEnter={onEnter}
      onMouseLeave={onLeave}
      onFocus={onFocus}
      onBlur={onBlur}
    >
      <span className={styles.topicTag}>{first}</span>
      {rest.length > 0 ? <span className={styles.topicMoreText}>+{formatCount(rest.length)}</span> : null}
      {tooltip}
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
          <col className={styles.colFollowers} />
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
            <th scope="col" className={styles.number}>粉丝</th>
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

function CreatorName({ item, onOpen, sourceName }: { item: CreatorItem; onOpen: (creatorKey: string) => void; sourceName: boolean }) {
  const name = authorName(item);
  const hint = sourceName ? `${name}（名称来自观察目标档案）` : name;
  const { anchorRef, onEnter, onLeave, onFocus, onBlur, onPointerDown, tooltip } = useTitleTooltip<HTMLButtonElement>(hint, 'width', { always: sourceName, delay: 250, maxWidth: 320 });
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

/** 相关作品只显示一个数；待判断与范围说明按需提示，不再是第二行。 */
function RelatedCell({ item, filtered, matched, href }: { item: CreatorItem; filtered: boolean; matched: number; href: string }) {
  const note = filtered
    ? item.unknownRelevanceWorkCount > 0
      ? `范围内共 ${formatCount(item.relatedWorkCount)} 篇；另有 ${formatCount(item.unknownRelevanceWorkCount)} 篇待判断`
      : `范围内共 ${formatCount(item.relatedWorkCount)} 篇`
    : item.unknownRelevanceWorkCount > 0
      ? `另有 ${formatCount(item.unknownRelevanceWorkCount)} 篇待判断`
      : '';
  const body = matched > 0
    ? <a className={styles.metric} href={href} data-return-key={item.creatorKey}>{formatCount(matched)}</a>
    : <span className={styles.metric}>{formatCount(matched)}</span>;
  return note ? <TipWrap text={note} focusable={matched === 0}>{body}</TipWrap> : body;
}

/** 首次发现只显示日期；时间精度与完整时间在提示里。 */
function FirstSeenCell({ item }: { item: CreatorItem }) {
  const date = item.firstSeenDate ?? item.firstSeenDisplay;
  const label = item.firstSeenPrecision ? FIRST_SEEN_LABELS[item.firstSeenPrecision] : undefined;
  const full = item.firstSeenDisplay ?? item.firstSeenDate;
  const text = full ? `${full}${label ? ` · ${label}` : ''}` : '首次发现时间未记录';
  return (
    <div className={styles.cell}>
      {date ? (
        <TipWrap text={text} focusable className={styles.nowrap}>{date}</TipWrap>
      ) : (
        <TipWrap text={text} focusable className={styles.secondary}>—</TipWrap>
      )}
    </div>
  );
}

function CreatorRow(props: CreatorTableProps & { item: CreatorItem; filtered: boolean }) {
  const { item, link, resources, selectedKey, selectedKeys, onOpen, onToggleSelect, filtered } = props;
  const work = item.representatives[0];
  const matched = filtered ? item.matchedRelatedWorkCount : item.relatedWorkCount;
  const name = authorName(item);
  const matchedHref = filtered ? workLink(link, item.creatorKey, { relevance: 'related' }) : scopeWorkLink(link, item.creatorKey, 'related');
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
          <CreatorName item={item} onOpen={onOpen} sourceName={item.displayNameKind === 'observation_target'} />
        </div>
      </td>
      <td className={styles.number}>
        <div className={styles.cell}>
          <Metric value={item.followerCount} className={styles.metric} />
        </div>
      </td>
      <td>
        <div className={styles.cell}>
          <KeywordCell item={item} />
        </div>
      </td>
      <td>
        <div className={styles.cell}>
          <Topics item={item} />
        </div>
      </td>
      <td className={styles.number}>
        <div className={styles.cell}>
          <RelatedCell item={item} filtered={filtered} matched={matched} href={matchedHref} />
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
        <FirstSeenCell item={item} />
      </td>
      <td>
        <div className={`${styles.cell} ${styles.rowAction}`}>
          <RowAction {...props} />
        </div>
      </td>
    </tr>
  );
}

/** 观察格只保留一个对应当前关系的入口：已观察去目标，未加入直接打开目标设置。 */
function RowAction({ item, query, data, domainActive, onJoin }: CreatorTableProps & { item: CreatorItem }) {
  const pending = useJoinPending(item.creatorKey);
  const observation = item.observation;
  if (observation.lookup === 'unavailable' || observation.inCurrentDomain === null) {
    return <span className={styles.secondary}>关系待读取</span>;
  }
  if (observation.inCurrentDomain) {
    return observation.targetRef ? <a href={targetHref(data.domain.domainRef, observation.targetRef)}>查看目标</a> : <span className={styles.secondary}>目标待核对</span>;
  }
  return (
    <button type="button" disabled={!domainActive || pending} onClick={() => onJoin(item, query.usageRole)}>
      {pending ? '加入中…' : '加入观察'}
    </button>
  );
}
