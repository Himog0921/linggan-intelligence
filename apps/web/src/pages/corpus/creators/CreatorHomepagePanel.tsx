import { useQuery } from '@tanstack/react-query';
import { useEffect, useMemo, useRef, useState } from 'react';
import { ActionButton } from '../../../shared/ActionButton';
import { humanMoment } from '../../../shared/formatting';
import { Cover } from './Media';
import { Metric } from './CreatorTable';
import { formatCount, workLink } from './creatorsQuery';
import { getHomepagePreview, newCommandRef, startHomepagePreview, stopHomepagePreview, studyCreatorWorks, type HomepageNote } from './creatorHomepageApi';
import { useWorkResources } from './useWorkResources';
import type { PanelContext } from './DrawerPanels';
import styles from './CreatorHomepagePanel.module.css';

type View = 'grid' | 'list';
type Preset = 'standard' | 'text_only';

// A stable empty list keeps the memoised ref list from changing on every render before data arrives.
const NO_NOTES: HomepageNote[] = [];

function suggested(notes: HomepageNote[]): string[] {
  if (notes.length === 0) return [];
  const chosen: string[] = [];
  const add = (ref: string | undefined) => {
    if (ref && !chosen.includes(ref)) chosen.push(ref);
  };
  // The first card the domain already holds as research material (the server says so, whatever filter
  // the page is showing), the highest observed like count, and a separate comparison card.
  // This does not claim to detect a semantic counterexample.
  add(notes.find(w => w.inDomain)?.workRef);
  const ranked = [...notes].filter(w => w.likes !== null).sort((a, b) => (b.likes ?? 0) - (a.likes ?? 0));
  add(ranked[0]?.workRef);
  add(notes.find(w => !chosen.includes(w.workRef))?.workRef);
  add(notes[0]?.workRef);
  return chosen.slice(0, Math.min(3, notes.length));
}

function statusText(state: string, count: number): string {
  switch (state) {
    case 'running': return `正在读取 · 已取得 ${formatCount(count)} 篇`;
    case 'complete': return `本次样本 ${formatCount(count)} 篇`;
    case 'partial': return `本次取得 ${formatCount(count)} 篇 · 范围不完整`;
    case 'stopped': return `已停止 · 保留 ${formatCount(count)} 篇`;
    default: return '尚无主页样本';
  }
}

/** 范围切换沿用母版的朱红手绘短线；封面/列表只用黑色下划线区分。 */
function SegmentMark() {
  return (
    <svg className={styles.segmentMark} viewBox="0 0 54 8" preserveAspectRatio="none" aria-hidden="true" focusable="false">
      <path d="M2 4.5C13 2 29 6 52 3.5M7 6c15-1 26-.8 39-1.4" />
    </svg>
  );
}

export function CreatorHomepagePanel({ ctx }: { ctx: PanelContext }) {
  const { item, data, link, resources } = ctx;
  const domain = data.domain.domainRef;
  const query = useQuery({
    queryKey: ['creator-homepage-preview', domain, item.creatorKey],
    queryFn: ({ signal }) => getHomepagePreview(domain, item.creatorKey, signal),
    refetchOnWindowFocus: false,
    refetchInterval: current => current.state.data?.state === 'running' ? 6500 : false,
    retry: 1,
  });
  const preview = query.data;
  const notes = preview?.works ?? NO_NOTES;
  const noteRefs = useMemo(() => notes.map(note => note.workRef), [notes]);
  const media = useWorkResources(domain, noteRefs);
  const [selected, setSelected] = useState<ReadonlySet<string>>(new Set());
  const [recommended, setRecommended] = useState<ReadonlySet<string>>(new Set());
  const [view, setView] = useState<View>('grid');
  const [filter, setFilter] = useState<'all' | 'selected'>('all');
  const [preset, setPreset] = useState<Preset>('standard');
  const [showConfirm, setShowConfirm] = useState(false);
  const [busy, setBusy] = useState<'preview' | 'stop' | 'study' | null>(null);
  const [feedback, setFeedback] = useState<string | null>(null);
  const [studyRun, setStudyRun] = useState<string | null>(null);
  const previewCommand = useRef<string | null>(null);
  // The identity of an unconfirmed study request is only valid for the run it was made against.
  const studyCommand = useRef<{ run: string; key: string } | null>(null);
  const stopCommand = useRef<string | null>(null);
  const confirmRef = useRef<HTMLElement>(null);
  const studyButtonRef = useRef<HTMLButtonElement>(null);
  const returnFocus = useRef(false);
  const [seededRun, setSeededRun] = useState<string | null>(null);
  const [edited, setEdited] = useState(false);

  // The confirmation sits below the sticky selection dock in a scrolling panel. Bring it into view
  // when it opens, move focus into it, and let Escape close it without writing anything.
  useEffect(() => {
    if (!showConfirm) return;
    const node = confirmRef.current;
    node?.scrollIntoView?.({ block: 'center' });
    node?.focus?.({ preventScroll: true });
    const onKey = (event: KeyboardEvent) => {
      if (event.key !== 'Escape') return;
      event.preventDefault();
      returnFocus.current = true;
      setShowConfirm(false);
    };
    document.addEventListener('keydown', onKey);
    return () => document.removeEventListener('keydown', onKey);
  }, [showConfirm]);

  useEffect(() => {
    if (showConfirm || !returnFocus.current) return;
    returnFocus.current = false;
    studyButtonRef.current?.focus();
  }, [showConfirm]);

  // Seed the suggested cards once per run; later polling and the user's own toggles never reseed.
  if (preview?.runRef && preview.works.length > 0 && seededRun !== preview.runRef) {
    const picks = suggested(preview.works);
    setSeededRun(preview.runRef);
    setSelected(new Set(picks));
    setRecommended(new Set(picks));
    setEdited(false);
  }

  const displayed = filter === 'selected' ? notes.filter(n => selected.has(n.workRef)) : notes;
  const selectedNotes = notes.filter(note => selected.has(note.workRef));
  const pending = busy !== null;
  const canWrite = data.domain.status === 'active';
  const state = preview?.state ?? 'not_requested';
  const limit = preview?.requestedLimit ?? 30;

  async function start() {
    if (pending || !canWrite) return;
    setBusy('preview'); setFeedback(null);
    previewCommand.current ??= newCommandRef();
    try {
      await startHomepagePreview(domain, item.creatorKey, previewCommand.current);
      previewCommand.current = null;
      setStudyRun(null);
      await query.refetch();
    } catch {
      setFeedback('读取请求尚未确认。可再次点击，系统会按同一请求识别重试。');
    } finally { setBusy(null); }
  }

  async function stop() {
    if (pending || !preview?.runRef || preview.revision === null) return;
    setBusy('stop'); setFeedback(null);
    stopCommand.current ??= newCommandRef();
    try {
      await stopHomepagePreview(domain, preview.runRef, preview.revision, stopCommand.current);
      stopCommand.current = null;
      await query.refetch();
    } catch {
      setFeedback('停止请求尚未确认，请核对当前状态再试；已取得的卡片不会删除。');
    } finally { setBusy(null); }
  }

  function closeConfirm() {
    returnFocus.current = true;
    setShowConfirm(false);
  }

  function toggle(ref: string) {
    if (pending) return;
    setShowConfirm(false);
    setStudyRun(null);
    studyCommand.current = null;
    setEdited(true);
    setSelected(current => {
      const next = new Set(current);
      if (next.has(ref)) next.delete(ref);
      else if (next.size < 30) next.add(ref);
      return next;
    });
  }

  async function study() {
    if (pending || !canWrite || !preview?.runRef || !preview.scopeFrozenAt || selectedNotes.length === 0) return;
    setBusy('study'); setFeedback(null);
    if (studyCommand.current?.run !== preview.runRef) studyCommand.current = { run: preview.runRef, key: newCommandRef() };
    try {
      const receipt = await studyCreatorWorks(
        domain, preview.runRef, selectedNotes.map(note => note.workRef), preset, studyCommand.current.key,
      );
      studyCommand.current = null;
      setStudyRun(receipt.runRef);
      setShowConfirm(false);
      setFeedback('已按所选作品建立有界任务；完成状况以采集系统的实际回执为准。');
    } catch {
      setFeedback('精选任务未得到确定回执。请保留当前选择后重试，不要扩大作品范围。');
    } finally { setBusy(null); }
  }

  return (
    <div className={styles.root}>
      {query.isPending ? <p className={styles.info}>读取已有主页快照…</p> : null}
      {query.isError ? (
        <div className={styles.alert} role="alert">
          主页样本读取暂不可用，已有领域作品仍可以正常查阅。
          <button type="button" onClick={() => void query.refetch()}>重新读取</button>
        </div>
      ) : null}

      {state === 'not_requested' && !query.isPending && !query.isError ? (
        <div className={styles.emptyState}>
          <span>尚无主页样本。</span>
          <ActionButton disabled={!canWrite} busy={busy === 'preview'} onClick={() => void start()}>
            读取主页（最多 {formatCount(limit)} 篇）
          </ActionButton>
        </div>
      ) : null}

      {preview?.runRef && state !== 'not_requested' ? (
        <div className={styles.snapshot} data-state={state}>
          <div className={styles.topLine}>
            <div className={styles.sampleLine}>
              <strong>{statusText(state, notes.length)}</strong>
              {preview.scopeFrozenAt ? <span>{humanMoment(preview.scopeFrozenAt)}</span> : null}
              {state === 'running' && canWrite ? (
                <button type="button" className={styles.textButton} disabled={pending} onClick={() => void stop()}>
                  {busy === 'stop' ? '停止中…' : '停止这次读取'}
                </button>
              ) : null}
            </div>
            <ActionButton onClick={() => void query.refetch()} busy={query.isFetching && !pending} disabled={pending}>
              刷新读取
            </ActionButton>
          </div>
          {state === 'partial' || state === 'stopped' || (state === 'complete' && notes.length === 0) ? (
            <p className={styles.rangeNote}>
              {state === 'partial'
                ? '本次范围不完整，已取得的卡片保留；未读取的数量不推算。'
                : state === 'stopped'
                  ? '本次已停止，已取得的卡片保留。'
                  : '本次没有取得可读的主页卡片。'}
              {canWrite ? (
                <button type="button" className={styles.textButton} disabled={pending} onClick={() => void start()}>
                  {busy === 'preview' ? '读取中…' : '重新读取'}
                </button>
              ) : null}
            </p>
          ) : null}
          {preview.frozenCount !== null && preview.frozenCount > notes.length ? (
            <p className={styles.rangeNote}>另有 {formatCount(preview.frozenCount - notes.length)} 篇暂时无法读取；稍后刷新读取重试。</p>
          ) : null}
        </div>
      ) : null}

      {notes.length > 0 ? (
        <>
          <div className={styles.tools}>
            <div className={`${styles.segment} ${styles.rangeSegment}`} role="group" aria-label="主页作品范围">
              <button type="button" aria-pressed={filter === 'all'} onClick={() => setFilter('all')}>
                全部 {formatCount(notes.length)}
                {filter === 'all' ? <SegmentMark /> : null}
              </button>
              <button type="button" aria-pressed={filter === 'selected'} onClick={() => setFilter('selected')}>
                已选 {formatCount(selectedNotes.length)}
                {filter === 'selected' ? <SegmentMark /> : null}
              </button>
            </div>
            <div className={styles.segment} role="group" aria-label="作品排版">
              <button type="button" aria-pressed={view === 'grid'} onClick={() => setView('grid')}>封面</button>
              <button type="button" aria-pressed={view === 'list'} onClick={() => setView('list')}>列表</button>
            </div>
          </div>
          <div className={view === 'grid' ? styles.grid : styles.list}>
            {displayed.map(note => {
              const resource = media.byRef.get(note.workRef) ?? resources.byRef.get(note.workRef);
              const chosen = selected.has(note.workRef);
              const title = resource?.title || note.title || '标题暂未取得';
              return (
                <div key={note.workRef} className={styles.note} data-selected={chosen}>
                  <button className={styles.noteChoice} type="button" onClick={() => toggle(note.workRef)} aria-pressed={chosen} aria-label={chosen ? `移除 ${title}` : `选择 ${title}`}>
                    {note.inDomain ? <span className={styles.sticker}>本领域已见</span> : null}
                    <span className={styles.check} aria-hidden="true">{chosen ? '✓' : ''}</span>
                    <Cover url={resource?.coverUrl ?? null} pending={media.pending} className={styles.cover} fallback={view === 'grid' ? '封面未取得' : '▧'} />
                  </button>
                  <div className={styles.noteInfo}>
                    <a className={styles.noteTitle} href={workLink(link, item.creatorKey, {}, note.workRef)} data-return-key={item.creatorKey} title={title}>{title}</a>
                    <div className={styles.noteStats}>
                      <span>赞 <Metric value={note.likes} /></span>
                      {!edited && recommended.has(note.workRef) ? <span className={styles.recommend}>系统推荐</span> : null}
                    </div>
                  </div>
                </div>
              );
            })}
          </div>
          {filter === 'selected' && displayed.length === 0 ? <p className={styles.info}>尚未选中任何作品。</p> : null}
          <div className={styles.studyDock}>
            <span className={styles.selectedCount}>已选 {formatCount(selectedNotes.length)} 篇</span>
            <ActionButton ref={studyButtonRef} aria-expanded={showConfirm} aria-controls="creator-study-confirm" disabled={!canWrite || selectedNotes.length === 0 || pending || !preview?.scopeFrozenAt || state === 'running'} variant="primary" onClick={() => { setShowConfirm(true); setFeedback(null); }}>
              研究已选 {formatCount(selectedNotes.length)} 篇
            </ActionButton>
          </div>
          {showConfirm ? (
            <section ref={confirmRef} id="creator-study-confirm" className={styles.confirm} aria-label="精选研究确认" tabIndex={-1}>
              <div className={styles.confirmHead}><h3>确认本次补充范围</h3><button className={styles.textButton} type="button" onClick={closeConfirm}>收起</button></div>
              <p>只处理当前快照中精确选定的 <strong>{formatCount(selectedNotes.length)}</strong> 篇，不自动展开其余主页作品，也不自动加入长期观察。</p>
              <div className={styles.presets}>
                <button type="button" aria-pressed={preset === 'standard'} onClick={() => { setPreset('standard'); studyCommand.current = null; }}>
                  <strong>标准研究资料</strong><span>正文、实际需要的媒体及识别、每篇最多 30 条评论与回复</span>
                </button>
                <button type="button" aria-pressed={preset === 'text_only'} onClick={() => { setPreset('text_only'); studyCommand.current = null; }}>
                  <strong>仅补文字资料</strong><span>详情页正文，不新采评论或媒体；已有内容优先复用</span>
                </button>
              </div>
              <div className={styles.confirmActions}>
                <button type="button" className={styles.textButton} onClick={closeConfirm}>返回选篇</button>
                <ActionButton variant="primary" busy={busy === 'study'} disabled={pending} onClick={() => void study()}>确认精选范围</ActionButton>
              </div>
            </section>
          ) : null}
        </>
      ) : null}
      {feedback ? <p className={styles.feedback} role="status">{feedback}</p> : null}
      {studyRun ? (
        <p className={styles.success}>
          已建立精选任务。<a href={`/collection/tasks?domain=${encodeURIComponent(domain)}`}>前往采集任务 →</a>
        </p>
      ) : null}
    </div>
  );
}
