import { useEffect, useId, useRef, useState, type KeyboardEvent, type PointerEvent } from 'react';
import { ActionButton } from '../../../shared/ActionButton';
import { IntelligenceIcon } from '../../../shared/IntelligenceIcon';
import { authorName, observedText, preservedLifecycle, targetHref } from './creatorsPresent';
import { formatCount, hasWorkFilter, type CreatorQuery, type LinkContext } from './creatorsQuery';
import type { CreatorItem, CreatorList } from './creatorsTypes';
import { Metric } from './Metric';
import type { DrawerTab } from './CreatorTable';
import { EvidencePanel, OverviewPanel, ProfilePanel, type PanelContext } from './DrawerPanels';
import { CreatorHomepagePanel } from './CreatorHomepagePanel';
import { Avatar } from './Media';
import { useJoinPending } from './useCreatorWrites';
import { mergeResources, useWorkResources, type WorkResources } from './useWorkResources';
import pageStyles from './CreatorsPage.module.css';
import styles from './CreatorDrawer.module.css';

/** 离开本页去看原作品时记下的检查器位置，回来时原样恢复。 */
export interface DrawerSnapshot {
  tab: DrawerTab;
  drawerScroll: number;
  expanded: string[];
}

export interface FocusRequest {
  kind: 'heading' | 'nav';
  dir?: -1 | 1;
  seq: number;
}

export interface CreatorDrawerProps {
  /** 同一页面会话内每个作者常驻一个实例；非活动实例只隐藏，不卸载。 */
  active: boolean;
  item: CreatorItem;
  data: CreatorList;
  query: CreatorQuery;
  link: LinkContext;
  /** 列表已读到的作品资源；检查器自己的作品资源在内部合并。 */
  listResources: WorkResources;
  initialTab: DrawerTab;
  /** 从证据库返回后恢复的检查器状态。 */
  restore: Partial<DrawerSnapshot> | null;
  /** 外部要求切换到某个页签（例如列表的「查看加入方式」明确指定观察记录）。 */
  pendingTab: { tab: DrawerTab; seq: number } | null;
  focusRequest: FocusRequest | null;
  /** 当前页顺序中的位置；作者不在本页时为 null。 */
  position: { index: number; total: number } | null;
  width: number | null;
  onTab: (tab: DrawerTab) => void;
  onNavigate: (delta: -1 | 1) => void;
  onWidth: (width: number) => void;
  onClose: () => void;
  onCorrect: (item: CreatorItem, field: string, workRef: string | null, resources: WorkResources) => void;
  onJoin: (item: CreatorItem, role: 'primary' | 'reference') => void;
  /** 点了去证据库的链接：把此刻的检查器状态交给页面保存。 */
  onLeave: (creatorKey: string, snapshot: DrawerSnapshot) => void;
}

const TABS: readonly { id: DrawerTab; label: string }[] = [
  { id: 'overview', label: '概览' },
  { id: 'works', label: '主页作品' },
  { id: 'evidence', label: '研究线索' },
  { id: 'profile', label: '观察记录' },
];

const MIN_WIDTH = 560;
const LIST_MIN_WIDTH = 600;

export function CreatorDrawer({
  active,
  item,
  data,
  query,
  link,
  listResources,
  initialTab,
  restore,
  pendingTab,
  focusRequest,
  position,
  width,
  onTab,
  onNavigate,
  onWidth,
  onClose,
  onCorrect,
  onJoin,
  onLeave,
}: CreatorDrawerProps) {
  const titleId = useId();
  const idBase = useId();
  const aside = useRef<HTMLElement>(null);
  const heading = useRef<HTMLHeadingElement>(null);
  const prevButton = useRef<HTMLButtonElement>(null);
  const nextButton = useRef<HTMLButtonElement>(null);
  const body = useRef<HTMLDivElement>(null);
  // 每个分区各记一个滚动位置；从证据库返回时，恢复离开时所在分区的位置。
  const positions = useRef<Record<DrawerTab, number>>({ overview: 0, works: 0, evidence: 0, profile: 0, [restore?.tab ?? initialTab]: restore?.drawerScroll ?? 0 });
  const tabButtons = useRef<Record<DrawerTab, HTMLButtonElement | null>>({ overview: null, works: null, evidence: null, profile: null });
  const [tab, setTab] = useState<DrawerTab>(initialTab);
  const [expanded, setExpanded] = useState<ReadonlySet<string>>(() => new Set(restore?.expanded ?? []));
  const joinPending = useJoinPending(item.creatorKey);
  const resizeStart = useRef<{ x: number; width: number } | null>(null);
  const wasActive = useRef(false);

  const ownRefs = [
    ...item.representatives.map(work => work.workRef),
    ...(item.firstSeenWorkRef ? [item.firstSeenWorkRef] : []),
  ];
  const ownResources = useWorkResources(data.domain.domainRef, ownRefs);
  const resources = mergeResources(listResources, ownResources);

  const domainActive = data.domain.status === 'active';
  const name = authorName(item);
  const first = item.representatives[0];
  const avatarUrl = first ? (resources.byRef.get(first.workRef)?.avatarUrl ?? null) : null;

  // 打开/切换作者后按来源放焦点：列表点开落到身份标题，上一位/下一位留在按钮上。
  useEffect(() => {
    if (!active) {
      wasActive.current = false;
      return;
    }
    if (focusRequest) {
      if (focusRequest.kind === 'heading') {
        heading.current?.focus({ preventScroll: true });
      } else {
        const preferred = focusRequest.dir === -1 ? prevButton.current : nextButton.current;
        const other = focusRequest.dir === -1 ? nextButton.current : prevButton.current;
        if (preferred && !preferred.disabled) preferred.focus({ preventScroll: true });
        else if (other && !other.disabled) other.focus({ preventScroll: true });
        else heading.current?.focus({ preventScroll: true });
      }
    } else if (!wasActive.current) {
      heading.current?.focus({ preventScroll: true });
    }
    wasActive.current = true;
  }, [active, focusRequest]);

  // 外部明确指定页签（例如「查看加入方式」）：跟它走，不被通用记忆覆盖。
  useEffect(() => {
    if (!active || !pendingTab) return;
    switchTab(pendingTab.tab);
    // switchTab 依赖当前 tab 与 body；只在外部请求变化时执行。
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [active, pendingTab?.seq]);

  // 内容区滚动位置：打开/切页签后下一帧恢复；内容晚到重试；用户自己操作即放弃。
  useEffect(() => {
    const element = body.current;
    if (!element) return;
    let target: number | null = positions.current[tab] ?? 0;
    let observer: ResizeObserver | null = null;
    let frame = 0;
    let stopped = false;
    const stop = () => {
      target = null;
      observer?.disconnect();
      observer = null;
    };
    const apply = () => {
      if (stopped || target === null) return;
      element.scrollTop = target;
      if (Math.abs(element.scrollTop - target) <= 1) stop();
    };
    const observe = () => {
      if (typeof ResizeObserver === 'undefined') return;
      observer = new ResizeObserver(apply);
      for (const panel of element.children) observer.observe(panel);
    };
    const onKeyDown = (event: globalThis.KeyboardEvent) => {
      if (['PageDown', 'PageUp', 'ArrowDown', 'ArrowUp', 'Home', 'End', ' '].includes(event.key)) stop();
    };
    element.addEventListener('wheel', stop, { passive: true });
    element.addEventListener('touchstart', stop, { passive: true });
    element.addEventListener('pointerdown', stop);
    element.addEventListener('keydown', onKeyDown);
    frame = requestAnimationFrame(() => {
      if (stopped) return;
      apply();
      observe();
      apply();
    });
    return () => {
      stopped = true;
      cancelAnimationFrame(frame);
      stop();
      element.removeEventListener('wheel', stop);
      element.removeEventListener('touchstart', stop);
      element.removeEventListener('pointerdown', stop);
      element.removeEventListener('keydown', onKeyDown);
    };
  }, [tab, active]);

  function switchTab(next: DrawerTab) {
    if (next === tab) return;
    if (body.current) positions.current[tab] = body.current.scrollTop;
    setTab(next);
    onTab(next);
  }

  function onTabKeyDown(event: KeyboardEvent<HTMLElement>) {
    if (!['ArrowLeft', 'ArrowRight', 'Home', 'End'].includes(event.key)) return;
    event.preventDefault();
    const index = TABS.findIndex(entry => entry.id === tab);
    const next =
      event.key === 'Home' ? 0 : event.key === 'End' ? TABS.length - 1 : (index + (event.key === 'ArrowRight' ? 1 : -1) + TABS.length) % TABS.length;
    const target = TABS[next]?.id ?? 'overview';
    switchTab(target);
    tabButtons.current[target]?.focus();
  }

  const context: PanelContext = {
    item,
    data,
    query,
    link,
    resources,
    writable: domainActive,
    active,
    onCorrect: (field, workRef) => onCorrect(item, field, workRef, resources),
    onTab: switchTab,
  };

  function maxWidth(): number {
    const parent = aside.current?.parentElement;
    const available = parent ? parent.getBoundingClientRect().width : window.innerWidth;
    return Math.max(320, available - LIST_MIN_WIDTH);
  }

  function onResizePointerDown(event: PointerEvent<HTMLDivElement>) {
    if (window.innerWidth <= 600) return;
    resizeStart.current = { x: event.clientX, width: aside.current?.getBoundingClientRect().width ?? 640 };
    event.currentTarget.setPointerCapture(event.pointerId);
  }
  function onResizePointerMove(event: PointerEvent<HTMLDivElement>) {
    if (!resizeStart.current) return;
    const limit = maxWidth();
    const proposed = resizeStart.current.width + resizeStart.current.x - event.clientX;
    onWidth(Math.min(limit, Math.max(Math.min(MIN_WIDTH, limit), proposed)));
  }
  function onResizePointerUp(event: PointerEvent<HTMLDivElement>) {
    resizeStart.current = null;
    if (event.currentTarget.hasPointerCapture(event.pointerId)) event.currentTarget.releasePointerCapture(event.pointerId);
  }

  const observation = item.observation;
  const kept = preservedLifecycle(observation);
  const blocked = query.usageRole === 'primary' && observation.hasOtherPrimaryDomain === true;
  const role = blocked ? 'reference' : query.usageRole;
  const joinLabel = blocked ? '按参照用途加入' : kept ? `加入并保持${kept}` : role === 'reference' ? '按参照用途加入' : '加入本领域观察';
  const filtered = hasWorkFilter(query);
  const leadLabel = filtered ? '命中最高赞' : '领域最高赞';
  const followerDay = item.followerObservedAt?.match(/^\d{4}-(\d{2})-(\d{2})/);
  const followerNote =
    item.followerCount === null
      ? null
      : item.followerObservedAt
        ? `主页资料 ${followerDay ? `${followerDay[1]}-${followerDay[2]}` : item.followerObservedAt}`
        : '主页资料 时间未记录';
  const canPrev = position !== null && position.index > 1;
  const canNext = position !== null && position.index < position.total;

  function onAsideKeyDown(event: KeyboardEvent<HTMLElement>) {
    if (event.key !== 'Escape' || event.defaultPrevented) return;
    // 内联研究确认先收自己；提示/浮层在 document 捕获阶段已处理。
    if ((event.target as HTMLElement).closest('[data-inline-confirm]')) return;
    event.preventDefault();
    onClose();
  }

  return (
    <aside
      ref={aside}
      className={styles.drawer}
      aria-labelledby={titleId}
      hidden={!active}
      data-creator-inspector=""
      data-creator-key={item.creatorKey}
      style={width ? { width: `${width}px` } : undefined}
      onKeyDown={onAsideKeyDown}
    >
      <div
        className={styles.resizeHandle}
        role="separator"
        tabIndex={0}
        aria-label="调整作者检查器宽度"
        aria-orientation="vertical"
        onPointerDown={onResizePointerDown}
        onPointerMove={onResizePointerMove}
        onPointerUp={onResizePointerUp}
        onPointerCancel={onResizePointerUp}
        onKeyDown={event => {
          if (event.key !== 'ArrowLeft' && event.key !== 'ArrowRight') return;
          event.preventDefault();
          const limit = maxWidth();
          const current = aside.current?.getBoundingClientRect().width ?? 640;
          const proposed = current + (event.key === 'ArrowLeft' ? 24 : -24);
          onWidth(Math.min(limit, Math.max(Math.min(MIN_WIDTH, limit), proposed)));
        }}
      />
      <header className={styles.head}>
        <div className={styles.title}>
          <Avatar name={name} url={avatarUrl} pending={resources.pending} className={`${pageStyles.avatar} ${pageStyles.avatarLarge}`} />
          <div className={styles.identity}>
            <h2 id={titleId} ref={heading} tabIndex={-1}>{name}</h2>
            <p>
              小红书 · {query.usageRole === 'reference' ? '参照材料' : '主研究材料'}
              {item.displayNameKind === 'observation_target' ? ' · 名称来自观察目标档案' : ''}
            </p>
          </div>
          <div className={styles.navCluster}>
            <div className={styles.navButtons}>
              <button ref={prevButton} type="button" className={styles.navButton} disabled={!canPrev} onClick={() => onNavigate(-1)}>
                上一位
              </button>
              <button ref={nextButton} type="button" className={styles.navButton} disabled={!canNext} onClick={() => onNavigate(1)}>
                下一位
              </button>
              <button type="button" className={styles.iconButton} aria-label="关闭作者检查器" onClick={onClose}>
                <IntelligenceIcon name="close" />
              </button>
            </div>
            <span className={styles.position}>{position ? `本页 ${position.index}/${position.total}` : '本页 —'}</span>
          </div>
        </div>
        <div className={styles.metrics}>
          <span>
            <span className={styles.metricRow}>
              <small>粉丝</small>
              <strong><Metric value={item.followerCount} /></strong>
            </span>
            {followerNote ? <em>{followerNote}</em> : null}
          </span>
          <span>
            <span className={styles.metricRow}>
              <small>相关作品</small>
              <strong>{formatCount(item.relatedWorkCount)}</strong>
            </span>
          </span>
          <span>
            <span className={styles.metricRow}>
              <small>{leadLabel}</small>
              <strong><Metric value={item.candidateHighLikeCount} /></strong>
            </span>
          </span>
        </div>
        <div className={styles.tabs} role="tablist" aria-label="作者详情分区" onKeyDown={onTabKeyDown}>
          {TABS.map(entry => (
            <button
              key={entry.id}
              ref={element => {
                tabButtons.current[entry.id] = element;
              }}
              id={`${idBase}-tab-${entry.id}`}
              type="button"
              role="tab"
              aria-selected={tab === entry.id}
              aria-controls={`${idBase}-panel-${entry.id}`}
              tabIndex={tab === entry.id ? 0 : -1}
              onClick={() => switchTab(entry.id)}
            >
              {entry.label}
            </button>
          ))}
        </div>
      </header>
      <div
        ref={body}
        className={styles.body}
        onClickCapture={event => {
          const anchor = (event.target as HTMLElement).closest('a[href^="/corpus/evidence?"]');
          if (!anchor || !body.current) return;
          positions.current[tab] = body.current.scrollTop;
          onLeave(item.creatorKey, { tab, drawerScroll: body.current.scrollTop, expanded: [...expanded] });
        }}
      >
        <div role="tabpanel" id={`${idBase}-panel-overview`} aria-labelledby={`${idBase}-tab-overview`} hidden={tab !== 'overview'}>
          <OverviewPanel ctx={context} />
        </div>
        <div role="tabpanel" id={`${idBase}-panel-works`} aria-labelledby={`${idBase}-tab-works`} hidden={tab !== 'works'}>
          <CreatorHomepagePanel ctx={context} />
        </div>
        <div role="tabpanel" id={`${idBase}-panel-evidence`} aria-labelledby={`${idBase}-tab-evidence`} hidden={tab !== 'evidence'}>
          <EvidencePanel
            ctx={context}
            expanded={expanded}
            onToggleWork={(workRef, open) =>
              setExpanded(current => {
                if (current.has(workRef) === open) return current;
                const next = new Set(current);
                if (open) next.add(workRef);
                else next.delete(workRef);
                return next;
              })
            }
          />
        </div>
        <div role="tabpanel" id={`${idBase}-panel-profile`} aria-labelledby={`${idBase}-tab-profile`} hidden={tab !== 'profile'}>
          <ProfilePanel ctx={context} />
        </div>
      </div>
      <span className="v7-sr-only" aria-live="polite">{active ? name : ''}</span>
      <footer className={styles.footer}>
        <div>
          <span>{observedText(observation)}</span>
        </div>
        <div className={styles.actions}>
          {observation.lookup === 'unavailable' || observation.inCurrentDomain === null ? (
            <ActionButton disabled>观察关系暂不可用</ActionButton>
          ) : observation.inCurrentDomain ? (
            observation.targetRef ? (
              <a href={targetHref(data.domain.domainRef, observation.targetRef)}>查看观察目标 →</a>
            ) : (
              <span>目标待核对</span>
            )
          ) : (
            <>
              <ActionButton variant="primary" disabled={!domainActive} busy={joinPending} onClick={() => onJoin(item, role)}>
                {joinPending ? '加入中…' : joinLabel}
              </ActionButton>
              {blocked ? <small>其他领域已有主研究关系；此操作仅加入参照用途，不改变原巡查规则。</small> : null}
              {observation.targetRef ? <a href={targetHref(data.domain.domainRef, observation.targetRef)}>查看原目标</a> : null}
            </>
          )}
        </div>
      </footer>
    </aside>
  );
}
