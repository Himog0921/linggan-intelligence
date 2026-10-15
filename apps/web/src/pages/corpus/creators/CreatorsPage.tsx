import { useQuery } from '@tanstack/react-query';
import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { useLocation, useSearchParams } from 'react-router';
import { CorpusFrame } from '../../../corpus/CorpusFrame';
import { useCorpusShell } from '../../../corpus/useCorpusShell';
import { ApiError } from '../../../shared/api';
import { humanMoment } from '../../../shared/formatting';
import { CreatorControls } from './CreatorControls';
import { CreatorCorrectionDialog, type CorrectionTarget } from './CreatorCorrectionDialog';
import { CreatorDrawer, type DrawerSnapshot } from './CreatorDrawer';
import { CreatorFilterDialog } from './CreatorFilterDialog';
import { CreatorJoinDialog, type JoinTarget } from './CreatorJoinDialog';
import { CreatorPolicyDialog } from './CreatorPolicyDialog';
import { CreatorScopeDialog } from './CreatorScopeDialog';
import { CreatorTable, type DrawerTab } from './CreatorTable';
import { fetchCreators } from './creatorsApi';
import { errorText, type Feedback } from './creatorsFeedback';
import { authorName, effectiveJudgement, preservedLifecycle, workTitle } from './creatorsPresent';
import {
  applyQuick,
  clearExtras,
  formatCount,
  parseQuery,
  removeCondition,
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
import { mergeResources, useWorkResources } from './useWorkResources';
import styles from './CreatorsPage.module.css';

type Dialog =
  | { kind: 'filter' }
  | { kind: 'scope' }
  | { kind: 'policy'; seq: number }
  | { kind: 'correction'; seq: number; target: CorrectionTarget }
  | { kind: 'join'; seq: number; target: JoinTarget };

interface DrawerState {
  creatorKey: string;
  tab: DrawerTab;
  /** 每次打开都是新的实例：上一位作者的状态、进行中的请求都不会带过来。 */
  seq: number;
  /** 打开那一刻的列表项。列表之后刷新、而这位作者暂时不在结果里时，抽屉仍显示这一份。 */
  snapshot: CreatorItem;
  restore: Partial<DrawerSnapshot> | null;
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

  // ── 抽屉与对话框：同一时间一个抽屉 + 一个对话框；范围一变就全部收起 ────────────
  const [drawer, setDrawer] = useState<DrawerState | null>(null);
  const [dialog, setDialog] = useState<Dialog | null>(null);
  const [notice, setNotice] = useState<Feedback | null>(null);
  const sequence = useRef(0);
  const scopeKey = `${domain ?? ''}|${apiKey}`;
  const [seenScope, setSeenScope] = useState(scopeKey);
  if (seenScope !== scopeKey) {
    setSeenScope(scopeKey);
    setDrawer(null);
    setDialog(null);
    setNotice(null);
  }

  const drawerItem = drawer ? (shown?.items.find(item => item.creatorKey === drawer.creatorKey) ?? drawer.snapshot) : null;
  const drawerRefs = useMemo(() => (drawerItem ? drawerItem.representatives.map(work => work.workRef) : []), [drawerItem]);
  const drawerOnly = useWorkResources(drawerItem ? domain : null, drawerRefs);
  const drawerResources = useMemo(() => mergeResources(listResources, drawerOnly), [listResources, drawerOnly]);

  const searchHandle = useRef<SearchFieldHandle>(null);
  const scrollRef = useRef<HTMLDivElement>(null);

  // ── 从证据库返回：恢复表格位置、作者焦点和打开的抽屉 ──────────────────────
  const [pendingReturn] = useState(() => peekReturn(pageUrl));
  const [restored, setRestored] = useState(pendingReturn === null);
  if (!restored && shown) {
    setRestored(true);
    const item = pendingReturn?.drawer ? shown.items.find(candidate => candidate.creatorKey === pendingReturn.creatorKey) : undefined;
    if (pendingReturn && item) {
      // 序号 0 留给恢复出来的这一个抽屉；之后每次用户打开的序号都从 1 起，不会撞上。
      setDrawer({
        creatorKey: item.creatorKey,
        tab: pendingReturn.tab ?? 'overview',
        seq: 0,
        snapshot: item,
        restore: { tab: pendingReturn.tab ?? 'overview', drawerScroll: pendingReturn.drawerScroll ?? 0, expanded: pendingReturn.expanded ?? [] },
      });
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

  // 「/」把焦点放进搜索框（没有打开的对话框、也不在输入框里时）。
  useEffect(() => {
    function onKeyDown(event: KeyboardEvent) {
      if (event.key !== '/' || event.ctrlKey || event.metaKey || event.altKey) return;
      if (document.querySelector('dialog[open]')) return;
      if ((event.target as HTMLElement | null)?.closest('input, textarea, select, [contenteditable="true"]')) return;
      event.preventDefault();
      searchHandle.current?.focus();
    }
    document.addEventListener('keydown', onKeyDown);
    return () => document.removeEventListener('keydown', onKeyDown);
  }, []);

  const link: LinkContext | null = domain ? { domain, query, returnTo: pageUrl } : null;
  const domainActive = data?.domain.status === 'active';
  const hasThreshold = data !== null && data.policy.likeThreshold !== null;

  // ── 动作 ───────────────────────────────────────────────────────────────
  const open = useCallback((item: CreatorItem, tab: DrawerTab = 'overview') => {
    sequence.current += 1;
    setDrawer({ creatorKey: item.creatorKey, tab, seq: sequence.current, snapshot: item, restore: null });
  }, []);

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

  function openCorrection(item: CreatorItem, field: string, workRef: string | null) {
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
        subjectTitle: work ? workTitle(work, drawerResources) : authorName(item),
        currentValue: current,
        manual: manual ? { reason: manual.judgement.reason, supportFragmentIds: manual.supportFragmentIds } : null,
        supports,
        workTitles: new Map(item.representatives.map(candidate => [candidate.workRef, workTitle(candidate, drawerResources)] as const)),
      },
    });
  }

  function openFilter() {
    searchHandle.current?.flush();
    setDialog({ kind: 'filter' });
  }

  function quick(key: QuickKey) {
    if ((key === 'outside' || key === 'high') && data?.stats.outside === null) return;
    go(current => applyQuick(current, key, hasThreshold));
    setDialog(null);
  }

  function refresh() {
    if (query.page !== 0) go(current => ({ ...current, page: 0 }), true);
    else void list.refetch();
  }

  // 去证据库看原作品之前，记下位置。抽屉里的链接由抽屉自己记（它还知道分区与展开项）。
  function rememberPosition(target: EventTarget) {
    const anchor = (target as HTMLElement).closest<HTMLAnchorElement>('a[href^="/corpus/evidence?"]');
    if (!anchor || anchor.closest('dialog')) return;
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
      if (!data.policy.analysisEnabled && (query.traits.length > 0 || query.focus || query.topicHint)) notes.push('仅检索已有判断；自动分析尚未开启。');
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
      foot="作者来自当前领域已有作品"
    >
      <div className={styles.page} onClickCapture={event => rememberPosition(event.target)}>
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
        {shown && link && domain ? (
          <CreatorTable
            key={apiKey}
            data={shown}
            query={query}
            link={link}
            resources={listResources}
            selectedKey={drawer?.creatorKey ?? null}
            analysisEnabled={shown.policy.analysisEnabled}
            domainActive={domainActive}
            scrollRef={scrollRef}
            busy={list.isFetching}
            onOpen={(creatorKey, tab) => {
              const item = shown.items.find(candidate => candidate.creatorKey === creatorKey);
              if (item) open(item, tab);
            }}
            onJoin={openJoin}
            onSort={(sort: SortKey) => go(current => ({ ...current, sort, page: 0 }))}
            onClearExtras={() => go(clearExtras)}
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
        {drawer && drawerItem && shown && link ? (
          <CreatorDrawer
            key={drawer.seq}
            item={drawerItem}
            data={shown}
            query={query}
            link={link}
            resources={drawerResources}
            initialTab={drawer.tab}
            restore={drawer.restore}
            onClose={() => setDrawer(null)}
            onCorrect={(field, workRef) => openCorrection(drawerItem, field, workRef)}
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
        ) : null}
        {dialog?.kind === 'filter' && shown ? (
          <CreatorFilterDialog
            query={query}
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
            onSaved={() => {
              setDialog(null);
              setNotice({ tone: 'success', text: '判断已保存；自动分析不会覆盖这一项，可随时恢复自动判断。', code: null });
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
