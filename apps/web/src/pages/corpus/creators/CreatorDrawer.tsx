import { useId, useLayoutEffect, useRef, useState, type KeyboardEvent } from 'react';
import { ActionButton } from '../../../shared/ActionButton';
import { ModalDialog } from '../../../corpus/ModalDialog';
import { findAuthorButton } from './findAuthor';
import { feedbackForError, type Feedback } from './creatorsFeedback';
import { authorName, observedText, preservedLifecycle, targetHref } from './creatorsPresent';
import { formatCount, type CreatorQuery, type LinkContext } from './creatorsQuery';
import type { CreatorItem, CreatorList } from './creatorsTypes';
import type { RetryScope } from './creatorsApi';
import { Metric, type DrawerTab } from './CreatorTable';
import { EvidencePanel, OverviewPanel, ProfilePanel, type PanelContext } from './DrawerPanels';
import { FeedbackLine } from './FeedbackLine';
import { Avatar } from './Media';
import { useJoinPending, useRetryMutation } from './useCreatorWrites';
import { useWriteGuard } from './useWriteGuard';
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
  { id: 'evidence', label: '作品与依据' },
  { id: 'profile', label: '作者资料' },
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
  const begin = useWriteGuard(item.creatorKey);
  const retry = useRetryMutation();
  const body = useRef<HTMLDivElement>(null);
  // 每个分区各记一个滚动位置；从证据库返回时，恢复离开时所在分区的位置。
  const positions = useRef<Record<DrawerTab, number>>({ overview: 0, evidence: 0, profile: 0, [restore?.tab ?? initialTab]: restore?.drawerScroll ?? 0 });
  const inFlight = useRef(new Set<string>());
  const tabButtons = useRef<Record<DrawerTab, HTMLButtonElement | null>>({ overview: null, evidence: null, profile: null });
  const [tab, setTab] = useState<DrawerTab>(initialTab);
  const [expanded, setExpanded] = useState<ReadonlySet<string>>(() => new Set(restore?.expanded ?? []));
  const [retrying, setRetrying] = useState<ReadonlySet<string>>(() => new Set());
  const [status, setStatus] = useState<Feedback | null>(null);
  const joinPending = useJoinPending(item.creatorKey);

  const domainActive = data.domain.status === 'active';
  const analysisEnabled = data.policy.analysisEnabled;
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

  async function runRetry(scope: RetryScope, id: string) {
    if (inFlight.current.has(id)) return;
    const isCurrent = begin();
    inFlight.current.add(id);
    setRetrying(new Set(inFlight.current));
    setStatus(null);
    try {
      const receipt = await retry.mutateAsync({ domain: data.domain.domainRef, scope });
      if (isCurrent()) {
        setStatus({ tone: 'success', text: receipt.queued ? '已排队，符合现有配置及额度后执行。' : '当前没有可重试的失败分析。', code: null });
      }
    } catch (error) {
      if (isCurrent()) setStatus(feedbackForError(error, { retryNote: '重试只把失败的分析重新排队，重复提交不会产生第二次分析。' }));
    } finally {
      inFlight.current.delete(id);
      if (isCurrent()) setRetrying(new Set(inFlight.current));
    }
  }

  const context: PanelContext = {
    item,
    data,
    query,
    link,
    resources,
    analysisEnabled,
    writable: domainActive,
    onCorrect,
    onRetry: (scope, id) => void runRetry(scope, id),
    retrying,
    onTab: switchTab,
  };

  const observation = item.observation;
  const kept = preservedLifecycle(observation);
  const blocked = query.usageRole === 'primary' && observation.hasOtherPrimaryDomain === true;
  const role = blocked ? 'reference' : query.usageRole;
  const joinLabel = blocked ? '按参照用途加入' : kept ? `加入并保持${kept}` : role === 'reference' ? '按参照用途加入' : '加入本领域观察';

  return (
    <ModalDialog
      labelledBy={titleId}
      className={styles.drawer}
      onClose={onClose}
      restoreFocus={() => findAuthorButton(item.creatorKey)}
      setup={positionBelowHeader}
      closeOnBackdrop
    >
      <header className={styles.head}>
        <div className={styles.title}>
          <Avatar name={name} url={avatarUrl} pending={resources.pending} className={`${pageStyles.avatar} ${pageStyles.avatarLarge}`} />
          <div>
            <small>创作者 · 判断依据</small>
            <h2 id={titleId} title={name}>
              {name}
            </h2>
            <p>
              {data.domain.name ?? '当前领域'} · {query.usageRole === 'reference' ? '参照材料' : '主研究材料'} · 小红书 ·{' '}
              {item.displayNameKind === 'observation_target' ? '观察目标名称 · ' : ''}粉丝 <Metric value={item.followerCount} />
            </p>
          </div>
          <button type="button" className={`${styles.textButton} ${styles.close}`} aria-label="关闭发现依据" onClick={onClose}>
            关闭
          </button>
        </div>
        <div className={styles.metrics}>
          <span>
            相关作品<strong>{formatCount(item.relatedWorkCount)}</strong>
          </span>
          <span>
            待判断<strong>{formatCount(item.unknownRelevanceWorkCount)}</strong>
          </span>
          <span>
            相关最高赞<strong><Metric value={item.maxRelatedLikeCount} /></strong>
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
          <FeedbackLine feedback={status} />
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
