import { useQuery } from '@tanstack/react-query';
import { useEffect, useMemo, useRef, useState } from 'react';
import { ActionButton } from '../../../shared/ActionButton';
import { Cover } from './Media';
import { Metric } from './CreatorTable';
import { formatCount, workLink } from './creatorsQuery';
import { getHomepagePreview, newCommandRef, startHomepagePreview, stopHomepagePreview, studyCreatorWorks, type HomepageNote } from './creatorHomepageApi';
import { useWorkResources } from './useWorkResources';
import type { PanelContext } from './DrawerPanels';
import styles from './CreatorHomepagePanel.module.css';

type View = 'grid' | 'list';
type Preset = 'standard' | 'text_only';

function suggested(notes: HomepageNote[], knownDomainRefs: readonly string[]): string[] {
  if (notes.length === 0) return [];
  const chosen: string[] = [];
  const add = (ref: string | undefined) => {
    if (ref && !chosen.includes(ref)) chosen.push(ref);
  };
  // The first discovered domain work, the highest observed like count,
  // and a separate comparison card. This does not claim to detect a semantic counterexample.
  add(notes.find(w => knownDomainRefs.includes(w.workRef))?.workRef);
  const ranked = [...notes].filter(w => w.likes !== null).sort((a, b) => (b.likes ?? 0) - (a.likes ?? 0));
  add(ranked[0]?.workRef);
  add(notes.find(w => !chosen.includes(w.workRef))?.workRef);
  add(notes[0]?.workRef);
  return chosen.slice(0, Math.min(3, notes.length));
}

function statusText(state: string, count: number): string {
  switch (state) {
    case 'running': return `正在浅看 · 本次已取得 ${formatCount(count)} 篇`;
    case 'complete': return `本次主页样本 ${formatCount(count)} 篇`;
    case 'partial': return `本次取得 ${formatCount(count)} 篇 · 范围不完整`;
    case 'stopped': return `已停止 · 保留 ${formatCount(count)} 篇`;
    default: return '尚未展开主页';
  }
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
  const notes = preview?.works ?? [];
  const noteRefs = useMemo(() => notes.map(note => note.workRef), [notes]);
  const media = useWorkResources(domain, noteRefs);
  const [selected, setSelected] = useState<ReadonlySet<string>>(new Set());
  const [view, setView] = useState<View>('grid');
  const [filter, setFilter] = useState<'all' | 'selected'>('all');
  const [preset, setPreset] = useState<Preset>('standard');
  const [showConfirm, setShowConfirm] = useState(false);
  const [busy, setBusy] = useState<'preview' | 'stop' | 'study' | null>(null);
  const [feedback, setFeedback] = useState<string | null>(null);
  const [studyRun, setStudyRun] = useState<string | null>(null);
  const previewCommand = useRef<string | null>(null);
  const studyCommand = useRef<string | null>(null);
  const stopCommand = useRef<string | null>(null);
  const selectedForRun = useRef<string | null>(null);
  const domainRefs = item.matchedWorkRefs;

  useEffect(() => {
    if (!preview?.runRef || selectedForRun.current === preview.runRef) return;
    if (preview.works.length === 0) return;
    selectedForRun.current = preview.runRef;
    setSelected(new Set(suggested(preview.works, domainRefs)));
  }, [preview?.runRef, preview?.works, domainRefs]);

  const displayed = filter === 'selected' ? notes.filter(n => selected.has(n.workRef)) : notes;
  const selectedNotes = notes.filter(note => selected.has(note.workRef));
  const seenLikes = notes.filter(note => note.likes !== null).length;
  const pending = busy !== null;
  const canWrite = data.domain.status === 'active';
  const state = preview?.state ?? 'not_requested';

  async function start() {
    if (pending || !canWrite) return;
    setBusy('preview'); setFeedback(null);
    previewCommand.current ??= newCommandRef();
    try {
      await startHomepagePreview(domain, item.creatorKey, previewCommand.current);
      previewCommand.current = null;
      await query.refetch();
    } catch {
      setFeedback('浅看请求尚未确认。可再次点击，系统会按同一请求识别重试。');
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

  function toggle(ref: string) {
    if (pending) return;
    setShowConfirm(false);
    setStudyRun(null);
    studyCommand.current = null;
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
    studyCommand.current ??= newCommandRef();
    try {
      const receipt = await studyCreatorWorks(
        domain, preview.runRef, selectedNotes.map(note => note.workRef), preset, studyCommand.current,
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
      <div className={styles.sectionHead}>
        <div>
          <span className={styles.eyebrow}>AUTHOR / SURFACE</span>
          <h3>先浅看，再决定读哪篇</h3>
        </div>
        <span className={styles.limit}>最多 30 篇</span>
      </div>
      {query.isPending ? <p className={styles.info}>读取已有主页快照…</p> : null}
      {query.isError ? (
        <div className={styles.alert} role="alert">
          主页样本读取暂不可用，已有领域作品仍可以正常查阅。
          <button type="button" onClick={() => void query.refetch()}>重新读取</button>
        </div>
      ) : null}
      {state === 'not_requested' && !query.isPending && !query.isError ? (
        <div className={styles.intro}>
          <div className={styles.introGlyph} aria-hidden="true">[ 01 … 30 ]</div>
          <h4>先展开这位作者的主页</h4>
          <p>目前在本领域已有 {formatCount(item.collectedWorkCount)} 篇作品。浅看只取得最多 30 篇主页卡片及有限公开资料，帮助挑选值得深入阅读的笔记。</p>
          <ActionButton variant="primary" disabled={!canWrite} busy={busy === 'preview'} onClick={() => void start()}>
            浅看主页
          </ActionButton>
          <p className={styles.muted}>本次不自动打开全部详情、不采集评论，也不会自动加入长期观察。</p>
        </div>
      ) : null}

      {preview?.runRef ? (
        <div className={styles.snapshot} data-state={state}>
          <div className={styles.stateLine}>
            <span className={styles.stateDot} aria-hidden="true" />
            <strong>{statusText(state, notes.length)}</strong>
            <span>目标上限 {formatCount(preview.requestedLimit)}</span>
            <span className={styles.grow} />
            {state === 'running' && canWrite ? (
              <button type="button" className={styles.textButton} disabled={pending} onClick={() => void stop()}>
                {busy === 'stop' ? '停止中…' : '停止这次浅看'}
              </button>
            ) : null}
            <button type="button" className={styles.textButton} disabled={pending} onClick={() => void query.refetch()}>更新</button>
          </div>
          <p>{state === 'complete'
            ? notes.length >= preview.requestedLimit
              ? '达到本次上限，不代表该作者全部历史作品。'
              : '本次发现执行结束；数量只代表实际取得的主页卡片。'
            : state === 'running'
              ? '正在取得主页卡片。可以在已有结果中先阅读，未结束前不发起精选。'
              : '本次仍有未确定范围，已取得的卡片保留；不把剩余数量推算成缺失篇数。'}</p>
          <div className={styles.snapshotMetrics}>
            <span>已取得 <strong><Metric value={notes.length} /></strong></span>
            <span>点赞已知 <strong><Metric value={seenLikes} /></strong></span>
            <span>已选研究 <strong><Metric value={selected.size} /></strong></span>
          </div>
        </div>
      ) : null}

      {notes.length > 0 ? (
        <>
          <div className={styles.tools}>
            <div className={styles.segment} role="group" aria-label="主页作品范围">
              <button type="button" aria-pressed={filter === 'all'} onClick={() => setFilter('all')}>全部样本 {notes.length}</button>
              <button type="button" aria-pressed={filter === 'selected'} onClick={() => setFilter('selected')}>已选 {selected.size}</button>
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
              const seenBefore = domainRefs.includes(note.workRef);
              return (
                <div key={note.workRef} className={styles.note} data-selected={chosen}>
                  <button className={styles.noteChoice} type="button" onClick={() => toggle(note.workRef)} aria-pressed={chosen} aria-label={chosen ? `移除 ${title}` : `选择 ${title}`}>
                    <span className={styles.check} aria-hidden="true">{chosen ? '✓' : ''}</span>
                    <Cover url={resource?.coverUrl ?? null} pending={media.pending} className={styles.cover} />
                  </button>
                  <div className={styles.noteInfo}>
                    <a className={styles.noteTitle} href={workLink(link, item.creatorKey, {}, note.workRef)} data-return-key={item.creatorKey} title={title}>{title}</a>
                    <div className={styles.noteStats}>赞 <Metric value={note.likes} /> <span>·</span> {seenBefore ? '本领域已见' : '主页新遇见'}</div>
                    <button className={styles.pick} type="button" aria-pressed={chosen} onClick={() => toggle(note.workRef)}>{chosen ? '已选研究' : '加入精选'}</button>
                  </div>
                </div>
              );
            })}
          </div>
          {filter === 'selected' && displayed.length === 0 ? <p className={styles.info}>尚未选中任何作品，切到全部样本查看。</p> : null}
          <div className={styles.studyDock}>
            <div><strong>精选研究</strong><p>默认从已知领域作品、已知高赞和另一篇对照卡片中建议最多 3 篇，你可以自由调整。</p></div>
            <ActionButton disabled={!canWrite || selected.size === 0 || pending || !preview?.scopeFrozenAt || state === 'running'} variant="primary" onClick={() => { setShowConfirm(true); setFeedback(null); }}>
              研究已选 {selected.size} 篇
            </ActionButton>
          </div>
          {showConfirm ? (
            <section className={styles.confirm} aria-label="精选研究确认">
              <div className={styles.sectionHead}><h3>确认本次补充范围</h3><button className={styles.textButton} type="button" onClick={() => setShowConfirm(false)}>收起</button></div>
              <p>只处理当前快照中精确选定的 <strong>{selectedNotes.length}</strong> 篇，不自动展开其余主页作品，也不自动加入长期观察。</p>
              <div className={styles.presets}>
                <button type="button" aria-pressed={preset === 'standard'} onClick={() => { setPreset('standard'); studyCommand.current = null; }}>
                  <strong>标准研究资料</strong><span>正文、实际需要的媒体及识别、每篇最多 30 条评论与回复</span>
                </button>
                <button type="button" aria-pressed={preset === 'text_only'} onClick={() => { setPreset('text_only'); studyCommand.current = null; }}>
                  <strong>仅补文字资料</strong><span>详情页正文，不新采评论或媒体；已有内容优先复用</span>
                </button>
              </div>
              <div className={styles.confirmActions}>
                <button type="button" className={styles.textButton} onClick={() => setShowConfirm(false)}>返回选篇</button>
                <ActionButton variant="primary" busy={busy === 'study'} disabled={pending} onClick={() => void study()}>确认精选范围</ActionButton>
              </div>
            </section>
          ) : null}
        </>
      ) : null}
      {feedback ? <p className={styles.feedback} role="status">{feedback}</p> : null}
      {studyRun ? (
        <div className={styles.success}>
          <strong>精选任务已建立</strong>
          <p>继续查看任务实际取得的正文、评论和媒体状态；任务结束不等于识别结果全部齐备。</p>
          <a href={`/collection/tasks?domain=${encodeURIComponent(domain)}`}>前往采集任务</a>
        </div>
      ) : null}
    </div>
  );
}
