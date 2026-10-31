import { useId, useLayoutEffect, useRef, useState, type KeyboardEvent, type PointerEvent } from 'react';
import { ActionButton } from '../../../shared/ActionButton';
import { ModalDialog } from '../../../corpus/ModalDialog';
import { humanMoment } from '../../../shared/formatting';
import { findAuthorButton } from './findAuthor';
import { authorName, observedText, preservedLifecycle, targetHref } from './creatorsPresent';
import { formatCount, hasWorkFilter, type CreatorQuery, type LinkContext } from './creatorsQuery';
import type { CreatorItem, CreatorList } from './creatorsTypes';
import { Metric, type DrawerTab } from './CreatorTable';
import { EvidencePanel, OverviewPanel, ProfilePanel, type PanelContext } from './DrawerPanels';
import { CreatorHomepagePanel } from './CreatorHomepagePanel';
import { Avatar } from './Media';
import { useJoinPending } from './useCreatorWrites';
import type { WorkResources } from './useWorkResources';
import pageStyles from './CreatorsPage.module.css';
import styles from './CreatorDrawer.module.css';

/** 离开本页去看原作品时记下的抽屉位置，回来时原样恢复。 */
export interface DrawerSnapshot {
  tab: DrawerTab;
  drawerScroll: number;
  expanded: string[];
}

interface CreatorDrawerProps {
  item: CreatorItem;
  data: CreatorList;
  query: CreatorQuery;
  link: LinkContext;
  resources: WorkResources;
  initialTab: DrawerTab;
  /** 从证据库返回后恢复的抽屉状态。 */
  restore: Partial<DrawerSnapshot> | null;
  onClose: () => void;
  onCorrect: (field: string, workRef: string | null) => void;
  onJoin: (item: CreatorItem, role: 'primary' | 'reference') => void;
  /** 点了去证据库的链接：把此刻的抽屉状态交给页面保存。 */
  onLeave: (creatorKey: string, snapshot: DrawerSnapshot) => void;
}

const TABS: readonly { id: DrawerTab; label: string }[] = [
  { id: 'overview', label: '概览' },
  { id: 'works', label: '主页作品' },
  { id: 'evidence', label: '研究线索' },
  { id: 'profile', label: '观察记录' },
];

/** 抽屉自页头下沿起，覆盖右侧；页头高度会随窗口变化（窄屏页头会折行），所以量实际位置。 */
function positionBelowHeader(dialog: HTMLDialogElement): () => void {
  const measure = () => {
    const shell = document.querySelector('.v7-shell');
    const top = shell ? Math.max(0, Math.min(shell.getBoundingClientRect().top, window.innerHeight)) : 0;
    dialog.style.setProperty('--creator-shell-top', `${top}px`);
  };
  measure();
  window.addEventListener('resize', measure);
  const header = document.querySelector('.v7-global-header');
  const observer = header && typeof ResizeObserver !== 'undefined' ? new ResizeObserver(measure) : null;
  if (header) observer?.observe(header);
  return () => {
    window.removeEventListener('resize', measure);
    observer?.disconnect();
  };
}

export function CreatorDrawer({ item, data, query, link, resources, initialTab, restore, onClose, onCorrect, onJoin, onLeave }: CreatorDrawerProps) {
  const titleId = useId();
  const idBase = useId();
  const body = useRef<HTMLDivElement>(null);
  // 每个分区各记一个滚动位置；从证据库返回时，恢复离开时所在分区的位置。
  const positions = useRef<Record<DrawerTab, number>>({ overview: 0, works: 0, evidence: 0, profile: 0, [restore?.tab ?? initialTab]: restore?.drawerScroll ?? 0 });
  const tabButtons = useRef<Record<DrawerTab, HTMLButtonElement | null>>({ overview: null, works: null, evidence: null, profile: null });
  const [tab, setTab] = useState<DrawerTab>(initialTab);
  const [expanded, setExpanded] = useState<ReadonlySet<string>>(() => new Set(restore?.expanded ?? []));
  const joinPending = useJoinPending(item.creatorKey);
  const resizeStart = useRef<{ x: number; width: number } | null>(null);

  const domainActive = data.domain.status === 'active';
  const name = authorName(item);
  const first = item.representatives[0];
  const avatarUrl = first ? (resources.byRef.get(first.workRef)?.avatarUrl ?? null) : null;

  // 切换分区时记住离开的那一区滚到哪里，回来时放回原位；首次打开恢复返回前的位置。
  useLayoutEffect(() => {
    if (body.current) body.current.scrollTop = positions.current[tab];
  }, [tab]);

  function switchTab(next: DrawerTab) {
    if (next === tab) return;
    if (body.current) positions.current[tab] = body.current.scrollTop;
    setTab(next);
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
    onCorrect,
    onTab: switchTab,
  };

  function onResizePointerDown(event: PointerEvent<HTMLDivElement>) {
    if (window.innerWidth <= 600) return;
    const dialog = event.currentTarget.closest('dialog');
    if (!dialog) return;
    resizeStart.current = { x: event.clientX, width: dialog.getBoundingClientRect().width };
    event.currentTarget.setPointerCapture(event.pointerId);
  }
  function onResizePointerMove(event: PointerEvent<HTMLDivElement>) {
    if (!resizeStart.current) return;
    const dialog = event.currentTarget.closest('dialog');
    if (!dialog) return;
    const width = resizeStart.current.width + resizeStart.current.x - event.clientX;
    dialog.style.width = `${Math.min(window.innerWidth, Math.max(600, width))}px`;
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
  const followerNote =
    item.followerCount === null
      ? '未记录'
      : item.followerObservedAt
        ? `主页资料 · ${humanMoment(item.followerObservedAt)}`
        : '主页资料 · 时间未记录';

  return (
    <ModalDialog
      labelledBy={titleId}
      className={styles.drawer}
      onClose={onClose}
      restoreFocus={() => findAuthorButton(item.creatorKey)}
      setup={positionBelowHeader}
      closeOnBackdrop
    >
      <div
        className={styles.resizeHandle}
        role="separator"
        tabIndex={0}
        aria-label="调整作者研究抽屉宽度"
        aria-orientation="vertical"
        onPointerDown={onResizePointerDown}
        onPointerMove={onResizePointerMove}
        onPointerUp={onResizePointerUp}
        onPointerCancel={onResizePointerUp}
        onKeyDown={event => {
          if (event.key !== 'ArrowLeft' && event.key !== 'ArrowRight') return;
          event.preventDefault();
          const dialog = event.currentTarget.closest('dialog');
          if (!dialog) return;
          const width = dialog.getBoundingClientRect().width + (event.key === 'ArrowLeft' ? 24 : -24);
          dialog.style.width = `${Math.min(window.innerWidth, Math.max(Math.min(window.innerWidth, 600), width))}px`;
        }}
      />
      <header className={styles.head}>
        <div className={styles.title}>
          <Avatar name={name} url={avatarUrl} pending={resources.pending} className={`${pageStyles.avatar} ${pageStyles.avatarLarge}`} />
          <div>
            <small>创作者 · 作者研究</small>
            <h2 id={titleId} title={name}>
              {name}
            </h2>
            <p>
              {data.domain.name ?? '当前领域'} · {query.usageRole === 'reference' ? '参照材料' : '主研究材料'} · 小红书
              {item.displayNameKind === 'observation_target' ? ' · 名称来自观察目标档案' : ''}
            </p>
          </div>
          <button type="button" className={`${styles.textButton} ${styles.close}`} aria-label="关闭作者研究" onClick={onClose}>
            关闭
          </button>
        </div>
        <div className={styles.metrics}>
          <span>
            <small>粉丝</small>
            <strong><Metric value={item.followerCount} /></strong>
            <em>{followerNote}</em>
          </span>
          <span>
            <small>{query.usageRole === 'reference' ? '本领域参照作品' : '本领域研究作品'}</small>
            <strong>{formatCount(item.collectedWorkCount)}</strong>
            <em>当前材料范围内已取得</em>
          </span>
          <span>
            <small>{filtered ? '命中最高赞' : '领域最高赞'}</small>
            <strong><Metric value={item.candidateHighLikeCount} /></strong>
            <em>{filtered ? '当前命中作品，含待判断' : '当前材料范围，含待判断'}</em>
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
    </ModalDialog>
  );
}
