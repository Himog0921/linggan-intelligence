import { useQueryClient } from '@tanstack/react-query';
import { useRef, useState } from 'react';
import { CorpusFrame } from '../../../corpus/CorpusFrame';
import { useCorpusShell } from '../../../corpus/useCorpusShell';
import { DomainChoiceDialog } from './DomainChoiceDialog';
import { EvidenceControls } from './EvidenceControls';
import { Inspector } from './Inspector';
import { ReadReceipt, ResultsArea } from './ResultsArea';
import { ReturnBanners } from './ReturnBanners';
import { SelectionBar } from './SelectionBar';
import { listKey } from './evidenceKeys';
import { readoutFacts, readoutTitle } from './evidencePresent';
import { DEFAULT_QUERY, scopeProblem, toApiParams, type EvidenceQuery, type PanelWidth } from './evidenceQuery';
import type { WorkItem } from './evidenceTypes';
import { assignLocation } from './navigation';
import { useCollectionSelection } from './useCollectionSelection';
import { useDrawerWork, useEvidenceAddress } from './useEvidenceAddress';
import { DRAWER_QUERY, matches, useMediaQuery } from './useMediaQuery';
import { usePanelWidthPreference } from './usePanelWidthPreference';
import { useWorkDetail } from './useWorkDetail';
import { useWorkList } from './useWorkList';
import styles from './EvidencePage.module.css';

const ROW = '[data-work-ref]';
const SELECT_TARGET = '[data-select-target]';

/** 证据库（语料）：检索与筛选、作品列表（研读／表格／封面）、检查器、批量动作。状态都在地址里。 */
export function EvidencePage() {
  const queryClient = useQueryClient();
  const { location, domainParam, query, go } = useEvidenceAddress();

  // ── 领域：以服务器解析为准，缺省或无效就是「未选」，不替用户挑 ─────────────────────
  const shellQuery = useCorpusShell(domainParam);
  const shell = shellQuery.data ?? null;
  const domain = shell?.selectedDomain ?? null;
  const [choiceDismissed, setChoiceDismissed] = useState(false);

  // ── 列表与详情 ────────────────────────────────────────────────────────────────
  const problem = domain === null ? null : scopeProblem(query);
  const { list, items, last, apiKey } = useWorkList(domain, query, problem !== null);
  // V2: 浏览态不自动打开首篇；显式作品地址仍可直接读取，不要求作品出现在当前批次。
  const selected = domain !== null ? query.work : null;
  const listItem = items.find(item => item.ref === selected) ?? null;
  const { detail, refetch: refetchDetail } = useWorkDetail({ domain, selected, listItem, collectionRunRef: query.collectionRunRef });

  // ── 检查器：桌面是一栏（可关、三档宽度），≤1180px 是盖在列表上的抽屉 ───────────────────
  const drawer = useMediaQuery(DRAWER_QUERY);
  const [drawerWork, setDrawerWork] = useDrawerWork(query.work);
  const drawerOpen = drawerWork !== null && drawerWork === selected;
  const [preferred, rememberWidth] = usePanelWidthPreference();
  const width: PanelWidth = query.panel !== null && query.panel !== 'closed' ? query.panel : preferred;
  const closed = query.panel === 'closed' || selected === null;
  const widthBeforeClose = useRef<PanelWidth | null>(null);
  const listRef = useRef<HTMLDivElement>(null);

  function rowOf(ref: string | null): HTMLElement | null {
    if (ref === null) return null;
    return Array.from(listRef.current?.querySelectorAll<HTMLElement>(ROW) ?? []).find(row => row.dataset.workRef === ref) ?? null;
  }

  function select(item: WorkItem, source: 'user' | 'auto') {
    // 用户点选＝新增一条历史，并重新打开已关闭的检查器；方向键移动＝替换历史。
    const reopen = source === 'user' && query.panel === 'closed';
    const remembered = widthBeforeClose.current;
    go(current => ({ ...current, work: item.ref, ...(reopen ? { panel: remembered === null || remembered === preferred ? null : remembered } : {}) }), source === 'auto');
    if (source === 'user' || drawerOpen) setDrawerWork(item.ref);
  }

  function closeInspector() {
    if (drawer) setDrawerWork(null);
    else {
      widthBeforeClose.current = width;
      go(current => ({ ...current, panel: 'closed' }), true);
    }
    rowOf(selected)?.querySelector<HTMLElement>(SELECT_TARGET)?.focus({ preventScroll: true });
  }

  function backToList() {
    setDrawerWork(null);
    const row = rowOf(selected);
    if (row === null) return;
    row.scrollIntoView({ behavior: matches('(prefers-reduced-motion: reduce)') ? 'auto' : 'smooth', block: 'center' });
    window.requestAnimationFrame(() => row.querySelector<HTMLElement>(SELECT_TARGET)?.focus({ preventScroll: true }));
  }

  // ── 勾选与批量动作 ──────────────────────────────────────────────────────────────
  const { selection, toggle, togglePage, clear } = useCollectionSelection(domain);
  const [selecting, setSelecting] = useState(false);
  const loadedRefs = items.map(item => item.ref);
  const visibleSelected = loadedRefs.filter(ref => selection.has(ref)).length;
  const allPageSelected = loadedRefs.length > 0 && visibleSelected === loadedRefs.length;
  function cancelSelection() { clear(); setSelecting(false); }

  function deepCollect() {
    if (domain === null) return;
    // 勾选集合已随每次变化写入 sessionStorage；采集页读同一个键。键名、数组格式与这条跳转形状在阶段 G 完成前冻结。
    assignLocation(`/collection/tasks?domain=${encodeURIComponent(domain)}&selected=1&returnTo=${encodeURIComponent(location.pathname + location.search)}`);
  }

  // ── 查询动作：提交与清空会重新读取，即使条件没有变（旧页的行为） ─────────────────────────
  function reread(next: EvidenceQuery) {
    go(() => next);
    if (domain !== null && toApiParams(domain, next).toString() === apiKey) void queryClient.resetQueries({ queryKey: listKey(domain, apiKey), exact: true });
  }

  const facts = readoutFacts(items);
  const hasDomain = domain !== null;
  const shownQuery: EvidenceQuery = hasDomain ? query : { ...DEFAULT_QUERY, layout: query.layout };
  return (
    <CorpusFrame
      page="evidence"
      pageLabel="证据库"
      action="/corpus/evidence"
      shell={shell}
      meta={null}
      foot={null}
      evidenceCount={hasDomain ? { count: facts.works, title: readoutTitle(facts) } : null}
    >
      <div className={styles.page}>
        <div className={styles.command}>
          <EvidenceControls
            query={shownQuery}
            hasDomain={hasDomain}
            scope={domain}
            busy={list.isFetching && !list.isFetchingNextPage}
            selecting={selecting}
            onToggleSelecting={() => selecting ? cancelSelection() : setSelecting(true)}
            onSelectPage={() => togglePage(loadedRefs)}
            allPageSelected={allPageSelected}
            pageHasItems={items.length > 0}
            onChange={go}
            onSubmit={text => reread({ ...query, q: text, view: 'all', work: null })}
            onClearFilters={() => reread({ ...query, lane: '', laneState: '', mediaKind: '', view: 'all', work: null })}
            banners={<ReturnBanners query={query} domainParam={domainParam} />}
          >
            {selecting ? <SelectionBar
              count={selection.size}
              visibleCount={items.length}
              visibleSelected={visibleSelected}
              hasDomain={hasDomain}
              onClear={cancelSelection}
              onDeepCollect={deepCollect}
            /> : null}
          </EvidenceControls>
        </div>
        <ReadReceipt last={last} failure={list.error} count={items.length} />
        <div className={styles.bench} data-inspector={closed ? 'closed' : width}>
          <ResultsArea
            shell={{ pending: shellQuery.isPending, error: shellQuery.error, domainCount: shell?.domains.length ?? null }}
            domainSelected={hasDomain}
            problem={problem}
            list={{
              pending: list.isPending,
              error: list.error,
              hasNext: list.hasNextPage,
              fetchingNext: list.isFetchingNextPage,
              nextFailed: list.isFetchNextPageError,
              loadMore: () => void list.fetchNextPage(),
              retry: () => void list.refetch(),
            }}
            items={items}
            selectedRef={selected}
            layout={query.layout}
            checked={selection}
            selectionMode={selecting}
            listRef={listRef}
            onSelect={select}
            onCheck={toggle}
            onRetryShell={() => void shellQuery.refetch()}
            onClearScope={() => go(current => (problem === 'collectionRunRef' ? { ...current, collectionRunRef: null } : { ...current, creatorKey: null, creatorFilter: null }), true)}
          />
          <div className={styles.inspectorSlot}>
            <Inspector
              domain={domain ?? ''}
              selectedRef={selected}
              listItem={listItem}
              detail={detail}
              tab={query.tab}
              width={width}
              drawer={drawer}
              open={drawer ? drawerOpen : !closed}
              onTab={tab => go(current => ({ ...current, tab }), true)}
              onWidth={next => {
                rememberWidth(next);
                go(current => ({ ...current, panel: next }), true);
              }}
              onClose={closeInspector}
              onBack={backToList}
              onRetry={() => void refetchDetail()}
              onRefresh={refetchDetail}
              onApplied={() => void refetchDetail()}
            />
          </div>
        </div>
        {shell !== null && !hasDomain && !choiceDismissed ? (
          <DomainChoiceDialog domains={shell.domains} pendingWork={query.work} onClose={() => setChoiceDismissed(true)} />
        ) : null}
      </div>
    </CorpusFrame>
  );
}
