import { CREATOR_RESEARCH_GUIDANCE } from '../../settings/aiSettingsModel';
import { useQuery } from '@tanstack/react-query';
import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { useLocation, useSearchParams } from 'react-router';
import { CorpusFrame } from '../../../corpus/CorpusFrame';
import { useCorpusShell } from '../../../corpus/useCorpusShell';
import { ApiError } from '../../../shared/api';
import { humanMoment } from '../../../shared/formatting';
import { CreatorBulkBar } from './CreatorBulkBar';
import { CreatorControls } from './CreatorControls';
import { CreatorCorrectionDialog, type CorrectionTarget } from './CreatorCorrectionDialog';
import { CreatorDrawer, type DrawerSnapshot, type FocusRequest } from './CreatorDrawer';
import { CreatorFilterDialog } from './CreatorFilterDialog';
import { CreatorJoinDialog, type JoinTarget } from './CreatorJoinDialog';
import { CreatorPolicyDialog } from './CreatorPolicyDialog';
import { CreatorScopeDialog } from './CreatorScopeDialog';
import { CreatorTable, type DrawerTab } from './CreatorTable';
import { fetchCreators } from './creatorsApi';
import { downloadCreatorsCsv } from './creatorsCsv';
import { errorText, type Feedback } from './creatorsFeedback';
import { authorName, effectiveJudgement, preservedLifecycle, workTitle } from './creatorsPresent';
import {
  applyQuick,
  clearExtras,
  formatCount,
  hasWorkFilter,
  parseQuery,
  presetDates,
  removeCondition,
  selectionScope,
  toApiParams,
  toPageSearch,
  type ConditionChip,
  type CreatorQuery,
  type LinkContext,
  type QuickKey,
  type SortKey,
} from './creatorsQuery';
import type { CreatorItem, CreatorList } from './creatorsTypes';
import { FeedbackLine } from './FeedbackLine';
import { findAuthorButton } from './findAuthor';
import { Pager } from './Pager';
import { clearReturn, peekReturn, saveReturn } from './returnState';
import type { SearchFieldHandle } from './SearchField';
import { useWorkResources, type WorkResources } from './useWorkResources';
import styles from './CreatorsPage.module.css';

type Dialog =
  | { kind: 'filter'; scope: string; query: CreatorQuery }
  | { kind: 'scope' }
  | { kind: 'policy'; seq: number }
  | { kind: 'correction'; seq: number; target: CorrectionTarget }
  | { kind: 'join'; seq: number; target: JoinTarget };

interface PanelState {
  /** 领域 + 作者：同一创作者在不同领域是两份独立会话。 */
  key: string;
  domainRef: string;
  creatorKey: string;
  item: CreatorItem;
  /** 打开时的列表读数；之后刷新只更新仍在当前结果里的项。 */
  data: CreatorList;
  query: CreatorQuery;
  link: LinkContext;
  initialTab: DrawerTab;
  restore: Partial<DrawerSnapshot> | null;
  /** 明确入口要求切换页签时使用；普通打开不覆盖上次页签。 */
  pendingTab: { tab: DrawerTab; seq: number } | null;
  currentTab: DrawerTab;
  focusRequest: FocusRequest | null;
}

/** 状态行上可点的补救动作。这里只描述要做什么，点击时才执行（渲染期间不碰地址与请求）。 */
type StatusActionKind = 'retry-shell' | 'retry-list' | 'clear-observation' | 'high-likes';

interface Status {
  text: string;
  error: boolean;
  action: { label: string; kind: StatusActionKind } | null;
  /** 没有路可走时给出出口（例如一个领域也没有）。 */
  link?: { label: string; href: string };
}

const NO_STATUS: Status = { text: '', error: false, action: null };

export function CreatorsPage() {
  const location = useLocation();
  const [params, setParams] = useSearchParams();
  const domainParam = params.get('domain');
  const query = useMemo(() => parseQuery(params), [params]);
  const pageUrl = location.pathname + location.search;

  // ── 领域外壳：领域列表、解析后的当前领域，全部以服务器为准 ─────────────
  const shellQuery = useCorpusShell(domainParam);
  const shell = shellQuery.data ?? null;
  const domain = shell?.selectedDomain ?? null;

  // ── 查询状态在地址里；所有改动都经过 go()，连续两次同步改动会叠加而不是互相覆盖 ──
  const latest = useRef({ query, domainParam, setParams });
  useEffect(() => {
    latest.current = { query, domainParam, setParams };
  });
  const go = useCallback((update: (current: CreatorQuery) => CreatorQuery, replace = false) => {
    const current = latest.current;
    const next = update(current.query);
    latest.current = { ...current, query: next };
    current.setParams(toPageSearch(current.domainParam, next), { replace });
  }, []);

  // ── 列表 ───────────────────────────────────────────────────────────────
  const apiKey = domain ? toApiParams(domain, query).toString() : '';
  const list = useQuery({
    queryKey: ['creators', domain, apiKey],
    queryFn: ({ signal }) => fetchCreators(domain ?? '', query, signal),
    enabled: domain !== null,
    gcTime: 0,
  });
  const data: CreatorList | null = list.data ?? null;
  const lastPage = data ? Math.max(0, Math.ceil(data.total / query.pageSize) - 1) : 0;
  // 地址里的页码比现有总数还大：回到最后一页，替换这条历史记录；回到之前不显示空表。
  const outOfRange = data !== null && query.page > lastPage;
  useEffect(() => {
    if (outOfRange) go(current => ({ ...current, page: lastPage }), true);
  }, [outOfRange, lastPage, go]);
  const shown = outOfRange ? null : data;

  const listRefs = useMemo(() => (shown ? shown.items.flatMap(item => (item.representatives[0] ? [item.representatives[0].workRef] : [])) : []), [shown]);
  const listResources = useWorkResources(domain, listRefs);

  const link: LinkContext | null = domain ? { domain, query, returnTo: pageUrl } : null;

  // ── 检查器：每个作者在当前页面会话里常驻一个实例；切换/关闭/翻页只隐藏，不卸载 ─────────
  // 这样进行中的主页读取、未确认的精选请求与同一 commandRef 的迟到回执都留在原作者名下。
  const [panels, setPanels] = useState<PanelState[]>([]);
  const [activeKey, setActiveKey] = useState<string | null>(null);
  const [panelWidth, setPanelWidth] = useState<number | null>(null);
  const tabSeq = useRef(0);
  const focusSeq = useRef(0);
  // 明确关闭后要回到哪位作者；翻页/改查询的自动关闭不设置它，也不抢走控件的焦点。
  const closingKey = useRef<string | null>(null);
  const [dialog, setDialog] = useState<Dialog | null>(null);
  const [notice, setNotice] = useState<Feedback | null>(null);
  const sequence = useRef(0);
  const scopeKey = `${domain ?? ''}|${apiKey}`;
  const [seenScope, setSeenScope] = useState(scopeKey);
  if (seenScope !== scopeKey) {
    setSeenScope(scopeKey);
    // 翻页/改查询只关闭当前检查器；仍在页面会话里的作者请求上下文必须保留。
    setActiveKey(null);
    setDialog(current => current?.kind === 'filter' && current.scope === scopeKey ? current : null);
    setNotice(null);
  }

  // 不在本页的实例保留打开时的快照；仍在当前结果里的读数在渲染时直接取最新值，
  // 这样刷新列表不会碰实例内部的主页选择、研究请求与未确认回执。
  const activePanel = panels.find(panel => panel.key === activeKey) ?? null;
  const activeIndex = shown && activePanel ? shown.items.findIndex(item => item.creatorKey === activePanel.creatorKey) : -1;
  const position = shown && activeIndex >= 0 ? { index: activeIndex + 1, total: shown.items.length } : null;

  function openItem(item: CreatorItem, options: { tab?: DrawerTab; focus?: 'heading' | 'nav'; dir?: -1 | 1 } = {}) {
    if (!domain || !shown || !link) return;
    const key = `${domain}|${item.creatorKey}`;
    const request: FocusRequest = {
      kind: options.focus ?? 'heading',
      ...(options.dir === undefined ? {} : { dir: options.dir }),
      seq: ++focusSeq.current,
    };
    const pending = options.tab ? { tab: options.tab, seq: ++tabSeq.current } : null;
    const known = panels.find(panel => panel.key === key);
    // 未访问过的作者沿用当前页签，连续上一位/下一位时不从概览重新开始。
    const initial = options.tab ?? known?.currentTab ?? activePanel?.currentTab ?? 'overview';
    setPanels(current => {
      if (current.some(panel => panel.key === key)) {
        return current.map(panel =>
          panel.key === key
            ? { ...panel, item, data: shown, query, link, focusRequest: request, ...(pending ? { pendingTab: pending } : {}) }
            : panel,
        );
      }
      return [
        ...current,
        {
          key,
          domainRef: domain,
          creatorKey: item.creatorKey,
          item,
          data: shown,
          query,
          link,
          initialTab: initial,
          restore: null,
          pendingTab: pending,
          currentTab: initial,
          focusRequest: request,
        },
      ];
    });
    setActiveKey(key);
  }

  function navigate(delta: -1 | 1) {
    if (!shown || activeIndex < 0) return;
    const next = shown.items[activeIndex + delta];
    if (!next) return;
    openItem(next, { focus: 'nav', dir: delta });
  }

  function closePanel() {
    const closing = panels.find(panel => panel.key === activeKey);
    closingKey.current = closing?.creatorKey ?? null;
    setActiveKey(null);
  }

  // ── 批量选择：同一批条件下翻页、换排序都保留；条件一变就清空，不导出范围外的作者 ──────────
  const selectionKey = domain ? selectionScope(domain, query) : '';
  const [selectedScope, setSelectedScope] = useState(selectionKey);
  const [selected, setSelected] = useState<ReadonlyMap<string, CreatorItem>>(new Map());
  if (selectedScope !== selectionKey) {
    setSelectedScope(selectionKey);
    setSelected(new Map());
  }
  const selectedKeys = useMemo(() => new Set(selected.keys()), [selected]);

  const searchHandle = useRef<SearchFieldHandle>(null);
  const scrollRef = useRef<HTMLDivElement>(null);

  // 检查器隐藏后把焦点放回触发它的作者按钮；作者已不在本页时落到表格区域，绝不掉到 body。
  useEffect(() => {
    if (activeKey !== null) return;
    const creatorKey = closingKey.current;
    if (!creatorKey) return;
    closingKey.current = null;
    const button = findAuthorButton(creatorKey);
    (button ?? scrollRef.current)?.focus({ preventScroll: true });
  }, [activeKey]);

  // ── 从证据库返回：恢复表格位置、作者焦点和打开的检查器 ──────────────────────
  const [pendingReturn] = useState(() => peekReturn(pageUrl));
  const [restored, setRestored] = useState(pendingReturn === null);
  if (!restored && shown && domain && link) {
    setRestored(true);
    const item = pendingReturn?.drawer ? shown.items.find(candidate => candidate.creatorKey === pendingReturn.creatorKey) : undefined;
    if (pendingReturn && item) {
      const key = `${domain}|${item.creatorKey}`;
      const tab = pendingReturn.tab ?? 'overview';
      const request: FocusRequest = { kind: 'heading', seq: 0 };
      setPanels(current => [
        ...current.filter(panel => panel.key !== key),
        {
          key,
          domainRef: domain,
          creatorKey: item.creatorKey,
          item,
          data: shown,
          query,
          link,
          initialTab: tab,
          restore: { tab, drawerScroll: pendingReturn.drawerScroll ?? 0, expanded: pendingReturn.expanded ?? [] },
          pendingTab: null,
          currentTab: tab,
          focusRequest: request,
        },
      ]);
      setActiveKey(key);
    }
  }
  useEffect(() => {
    if (!restored || !pendingReturn) return;
    const wrap = scrollRef.current;
    if (wrap) {
      wrap.scrollTop = pendingReturn.scrollY;
      wrap.scrollLeft = pendingReturn.scrollX;
    }
    if (pendingReturn.creatorKey && !pendingReturn.drawer) findAuthorButton(pendingReturn.creatorKey)?.focus({ preventScroll: true });
    clearReturn();
  }, [restored, pendingReturn]);

  // 「/」把焦点放进搜索框（没有打开的对话框、不在输入框里、也不在检查器里时）。
  useEffect(() => {
    function onKeyDown(event: KeyboardEvent) {
      if (event.key !== '/' || event.ctrlKey || event.metaKey || event.altKey) return;
      if (document.querySelector('dialog[open]')) return;
      const target = event.target as HTMLElement | null;
      if (target?.closest('[data-creator-inspector]')) return;
      if (target?.closest('input, textarea, select, [role="combobox"], [role="listbox"], [contenteditable="true"]')) return;
      event.preventDefault();
      searchHandle.current?.focus();
    }
    document.addEventListener('keydown', onKeyDown);
    return () => document.removeEventListener('keydown', onKeyDown);
  }, []);

  const domainActive = data?.domain.status === 'active';
  const hasThreshold = data !== null && data.policy.likeThreshold !== null;

  // ── 动作 ───────────────────────────────────────────────────────────────
  function openJoin(item: CreatorItem, role: 'primary' | 'reference') {
    if (!shown || !domain) return;
    sequence.current += 1;
    setDialog({
      kind: 'join',
      seq: sequence.current,
      target: {
        domain,
        domainName: shown.domain.name ?? '当前领域',
        creatorKey: item.creatorKey,
        authorName: authorName(item),
        role,
        blockedPrimary: query.usageRole === 'primary' && item.observation.hasOtherPrimaryDomain === true,
        keptState: preservedLifecycle(item.observation),
      },
    });
  }

  function openCorrection(item: CreatorItem, field: string, workRef: string | null, resources: WorkResources) {
    if (!shown || !domain) return;
    const work = workRef === null ? null : item.representatives.find(candidate => candidate.workRef === workRef);
    if (workRef !== null && !work) return;
    const analysis = work ? work.analysis : item.authorAnalysis;
    const manual = analysis.manual[field];
    const current = field === 'focus' ? (manual?.judgement.value ?? item.focus.value) : effectiveJudgement(analysis, field).value;
    const supports = work || field !== 'institution_or_brand' ? analysis.supportFragments : analysis.identitySupportFragments;
    sequence.current += 1;
    setDialog({
      kind: 'correction',
      seq: sequence.current,
      target: {
        domain,
        domainName: shown.domain.name ?? '当前领域',
        creatorKey: item.creatorKey,
        workRef,
        field,
        subjectTitle: work ? workTitle(work, resources) : authorName(item),
        currentValue: current,
        manual: manual ? { reason: manual.judgement.reason, supportFragmentIds: manual.supportFragmentIds } : null,
        supports,
        workTitles: new Map(item.representatives.map(candidate => [candidate.workRef, workTitle(candidate, resources)] as const)),
      },
    });
  }

  function openFilter() {
    searchHandle.current?.flush();
    const pending = latest.current.query;
    setDialog({ kind: 'filter', query: pending, scope: `${domain ?? ''}|${domain ? toApiParams(domain, pending).toString() : ''}` });
  }

  function quick(key: QuickKey) {
    const leadRule = data?.leadRule ?? null;
    if ((key === 'outside' || key === 'high') && (data === null || data.stats.outside === null)) return;
    if (key === 'inside' && (data === null || data.stats.inside === null)) return;
    if (key === 'high' && leadRule === null) return;
    const recentFrom = presetDates(7, new Date()).from;
    go(current => applyQuick(current, key, leadRule, recentFrom));
    setDialog(null);
  }

  function refresh() {
    if (query.page !== 0) go(current => ({ ...current, page: 0 }), true);
    else void list.refetch();
  }

  // ── 批量选择与导出 ─────────────────────────────────────────────────────
  const toggleSelected = useCallback((item: CreatorItem, checked: boolean) => {
    setSelected(current => {
      const next = new Map(current);
      if (checked) next.set(item.creatorKey, item);
      else next.delete(item.creatorKey);
      return next;
    });
  }, []);

  const selectPage = useCallback((checked: boolean) => {
    if (!shown) return;
    setSelected(current => {
      const next = new Map(current);
      for (const item of shown.items) {
        if (checked) next.set(item.creatorKey, item);
        else next.delete(item.creatorKey);
      }
      return next;
    });
  }, [shown]);

  function exportSelected() {
    if (selected.size === 0) return;
    // 当前页上还在的作者用最新读数，其他页保留选中时的快照。
    const items = [...selected.values()].map(item => shown?.items.find(candidate => candidate.creatorKey === item.creatorKey) ?? item);
    downloadCreatorsCsv(items, hasWorkFilter(query));
    setNotice({ tone: 'success', text: `已导出 ${formatCount(selected.size)} 位创作者。`, code: null });
  }

  // 去证据库看原作品之前，记下位置。抽屉里的链接由抽屉自己记（它还知道分区与展开项）。
  function rememberPosition(target: EventTarget) {
    const anchor = (target as HTMLElement).closest<HTMLAnchorElement>('a[href^="/corpus/evidence?"]');
    if (!anchor || anchor.closest('[data-creator-inspector], dialog')) return;
    saveReturn({
      url: pageUrl,
      scrollY: scrollRef.current?.scrollTop ?? 0,
      scrollX: scrollRef.current?.scrollLeft ?? 0,
      ...(anchor.dataset.returnKey ? { creatorKey: anchor.dataset.returnKey } : {}),
    });
  }

  // ── 状态行：读取中、失败与重试、未选领域、局限 ─────────────────────────────
  const status = describeStatus();
  function describeStatus(): Status {
    if (shellQuery.isPending) return { text: '正在读取领域…', error: false, action: null };
    if (shellQuery.isError) {
      return { text: failureText(shellQuery.error), error: true, action: { label: '重试读取', kind: 'retry-shell' } };
    }
    if (shell && shell.domains.length === 0) {
      return { text: '还没有可选择的研究领域。', error: false, action: null, link: { label: '前往领域管理 →', href: '/collection/domains' } };
    }
    if (domain === null) return { text: '请选择领域。', error: false, action: null };
    if (list.isPending) return { text: '读取创作者与作品…', error: false, action: null };
    if (list.isError && !data) {
      const code = list.error instanceof ApiError && list.error.failure.kind === 'http' ? list.error.failure.code : null;
      const action: Status['action'] =
        code === 'observation_lookup_unavailable'
          ? { label: '清除观察条件后重试', kind: 'clear-observation' }
          : code === 'viral_threshold_unset'
            ? { label: '改看高赞线索', kind: 'high-likes' }
            : { label: '重试读取', kind: 'retry-list' };
      return { text: failureText(list.error), error: true, action };
    }
    const notes: string[] = [];
    // 刷新失败但页面上还有上一次读到的内容：如实说明下面显示的不是最新的。
    if (list.isError && data) notes.push('最新读取没有成功，下方仍是上一次读取到的内容。');
    if (data) {
      if (query.traits.length > 0 || query.focus || query.topicHint) notes.push(`仅检索已有判断；${CREATOR_RESEARCH_GUIDANCE}。`);
      if (data.domain.status !== 'active') notes.push('当前领域已暂停，历史材料仍可读，写入操作不可用。');
    }
    if (listResources.failed) notes.push('媒体暂不可用；文字、作品链接与查询仍可使用。');
    if (notes.length === 0) return NO_STATUS;
    const stale = list.isError && data !== null;
    return { text: notes.join(' '), error: stale, action: stale ? { label: '重试读取', kind: 'retry-list' } : null };
  }

  function runStatusAction(kind: StatusActionKind) {
    if (kind === 'retry-shell') void shellQuery.refetch();
    else if (kind === 'retry-list') void list.refetch();
    else if (kind === 'clear-observation') go(current => ({ ...current, observation: '', viral: false, highLikes: false, page: 0 }));
    else go(current => ({ ...current, viral: false, highLikes: true, sort: 'high_likes', page: 0 }));
  }

  const resultText = shellQuery.isError || (list.isError && !data)
    ? '读取失败'
    : domain === null && shell !== null
      ? '请选择领域'
      : shown
        ? `${formatCount(shown.total)} 位创作者`
        : '读取中…';
  const readout = shown
    ? shown.asOf
      ? `读取于 ${humanMoment(shown.asOf)}`
      : shown.displayAsOf
        ? `读取于 ${shown.displayAsOf}`
        : '读取时间未知'
    : domain !== null && list.isPending
      ? '读取中…'
      : '';

  return (
    <CorpusFrame
      page="creators"
      pageLabel="创作者"
      action="/corpus/creators"
      shell={shell}
      meta={
        <span className={styles.readout} aria-live="polite">
          {readout}
        </span>
      }
    >
      <div className={styles.page} onClickCapture={event => rememberPosition(event.target)}>
        <div className={styles.workspace}>
          <div className={styles.main}>
            <CreatorControls
              query={query}
              data={shown}
              resultText={resultText}
              searchHandle={searchHandle}
              onSearchCommit={text => go(current => ({ ...current, query: text, page: 0 }))}
              onSearchMode={(mode, text) => go(current => ({ ...current, searchMode: mode, query: text, page: 0 }))}
              onOpenFilter={openFilter}
              onSort={(sort: SortKey) => go(current => ({ ...current, sort, page: 0 }))}
              onRefresh={refresh}
              refreshing={list.isFetching}
              onQuick={quick}
              onOpenPolicy={() => {
                sequence.current += 1;
                setDialog({ kind: 'policy', seq: sequence.current });
              }}
              onOpenScope={() => setDialog({ kind: 'scope' })}
              onRemoveChip={(chip: ConditionChip) => go(current => removeCondition(current, chip))}
              onClearChips={() => go(clearExtras)}
            />
            <p className={styles.status} role="status" aria-live="polite" data-error={status.error}>
              {status.text}
              {status.link ? (
                <span className={styles.statusActions}>
                  <a href={status.link.href}>{status.link.label}</a>
                </span>
              ) : null}
              {status.action ? (
                <span className={styles.statusActions}>
                  <button type="button" className={styles.textButton} onClick={() => status.action && runStatusAction(status.action.kind)}>
                    {status.action.label}
                  </button>
                </span>
              ) : null}
            </p>
            <FeedbackLine feedback={notice} />
            <CreatorBulkBar count={selected.size} onExport={exportSelected} onClear={() => setSelected(new Map())} />
            {shown && link && domain ? (
              <CreatorTable
                key={apiKey}
                data={shown}
                query={query}
                link={link}
                resources={listResources}
                selectedKey={activePanel?.creatorKey ?? null}
                selectedKeys={selectedKeys}
                domainActive={domainActive}
                scrollRef={scrollRef}
                busy={list.isFetching}
                onOpen={(creatorKey, tab) => {
                  const item = shown.items.find(candidate => candidate.creatorKey === creatorKey);
                  if (item) openItem(item, tab ? { tab } : {});
                }}
                onJoin={openJoin}
                onSort={(sort: SortKey) => go(current => ({ ...current, sort, page: 0 }))}
                onClearExtras={() => go(clearExtras)}
                onToggleSelect={toggleSelected}
                onSelectPage={selectPage}
              />
            ) : (
              <div className={styles.tableWrap} aria-busy={shellQuery.isPending || (list.isPending && list.fetchStatus !== 'idle')} />
            )}
            <Pager
              page={query.page}
              pageSize={query.pageSize}
              total={shown ? shown.total : null}
              onPage={page => go(current => ({ ...current, page }))}
              onPageSize={pageSize => go(current => ({ ...current, pageSize, page: 0 }))}
            />
          </div>
          {panels.map(panel => {
            const liveItem = shown && domain === panel.domainRef
              ? shown.items.find(candidate => candidate.creatorKey === panel.creatorKey)
              : undefined;
            return (
              <CreatorDrawer
                key={panel.key}
                active={panel.key === activeKey}
                item={liveItem ?? panel.item}
                data={liveItem && shown ? shown : panel.data}
                query={liveItem ? query : panel.query}
                link={liveItem && link ? link : panel.link}
                listResources={listResources}
                initialTab={panel.initialTab}
                restore={panel.restore}
                pendingTab={panel.pendingTab}
                focusRequest={panel.focusRequest}
                position={panel.key === activeKey ? position : null}
                width={panelWidth}
                onTab={tab => {
                  setPanels(current => current.map(entry => (entry.key === panel.key ? { ...entry, currentTab: tab } : entry)));
                }}
                onNavigate={navigate}
                onWidth={setPanelWidth}
                onClose={closePanel}
                onCorrect={(item, field, workRef, resources) => openCorrection(item, field, workRef, resources)}
                onJoin={openJoin}
                onLeave={(creatorKey, snapshot) =>
                  saveReturn({
                    url: pageUrl,
                    scrollY: scrollRef.current?.scrollTop ?? 0,
                    scrollX: scrollRef.current?.scrollLeft ?? 0,
                    creatorKey,
                    drawer: true,
                    ...snapshot,
                  })
                }
              />
            );
          })}
        </div>
        {dialog?.kind === 'filter' && shown ? (
          <CreatorFilterDialog
            query={dialog.query}
            hasThreshold={hasThreshold}
            observationAvailable={shown.stats.outside !== null}
            onApply={next => {
              setDialog(null);
              go(() => next);
            }}
            onClose={() => setDialog(null)}
          />
        ) : null}
        {dialog?.kind === 'scope' && shown && link ? (
          <CreatorScopeDialog
            data={shown}
            usageRole={query.usageRole}
            hasDateRange={query.publishedFrom !== '' || query.publishedTo !== ''}
            link={link}
            onQuick={quick}
            onShowAllDates={() => {
              go(current => ({ ...current, publishedFrom: '', publishedTo: '', page: 0 }));
              setDialog(null);
            }}
            onClose={() => setDialog(null)}
          />
        ) : null}
        {dialog?.kind === 'policy' && shown && domain ? (
          <CreatorPolicyDialog
            key={dialog.seq}
            domain={domain}
            policy={shown.policy}
            domainActive={domainActive}
            onSaved={() => {
              setDialog(null);
              setNotice({ tone: 'success', text: '发现设置已保存。保存本身不启动采集，也不会立刻调用模型。', code: null });
            }}
            onClose={() => setDialog(null)}
          />
        ) : null}
        {dialog?.kind === 'correction' ? (
          <CreatorCorrectionDialog
            key={dialog.seq}
            target={dialog.target}
            onSaved={cleared => {
              setDialog(null);
              setNotice({ tone: 'success', text: cleared ? '人工调整已撤销；显示已有研究结果，尚无结果则显示尚不能判定。未启动新分析。' : '判断已保存；这一项使用你的判断，可随时撤销人工调整。', code: null });
            }}
            onClose={() => setDialog(null)}
          />
        ) : null}
        {dialog?.kind === 'join' ? <CreatorJoinDialog key={dialog.seq} target={dialog.target} onClose={() => setDialog(null)} /> : null}
      </div>
    </CorpusFrame>
  );
}

/** 读取失败的说明：有中文说明就用，没有就如实说「读取暂不可用」，不显示机器码。 */
function failureText(error: unknown): string {
  if (error instanceof ApiError) {
    if (error.failure.kind === 'http') return errorText(error.failure.code) ?? '读取暂不可用，请重试。';
    if (error.failure.kind === 'network') return '读取没有得到答复，请检查本机服务后重试。';
    return '服务返回的内容无法识别，页面没有采用它。';
  }
  return '读取暂不可用，请重试。';
}
