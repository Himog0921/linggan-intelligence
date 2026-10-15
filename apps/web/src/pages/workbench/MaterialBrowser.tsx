import { useEffect, useMemo, useRef, useState } from 'react';
import { useQuery, useQueryClient } from '@tanstack/react-query';
import { isRecord } from '../../shared/api';
import * as api from './workbenchApi';
import { GROUPS, SOURCE_LABELS, type Citation, type CanvasGroup, type SourceItem, type SourceKind, type ValidatedAnchor, type WorkDocument } from './workbenchModel';
import styles from './WorkbenchPage.module.css';
interface Props {
  contextKey?: string;
  active?: boolean;
  domain: string;
  document: WorkDocument;
  onCollect: (anchor: ValidatedAnchor, group: CanvasGroup) => void;
  focusCitation?: Citation | null;
}
interface Strip {
  id: string;
  source: SourceItem;
  start: number;
  end: number;
  text: string;
  index: number;
}
interface Selection {
  start: number;
  end: number;
  text: string;
}
function strips(source: SourceItem): Strip[] {
  let offset = 0;
  const result: Strip[] = [];
  for (const part of source.text.split(/(?<=\n)/u)) {
    const size = Array.from(part).length;
    if (part.trim()) result.push({
      id: `${source.fragmentId}:${offset}:${offset + size}`,
      source,
      start: offset,
      end: offset + size,
      text: part,
      index: result.length
    });
    offset += size;
  }
  return result;
}
function SourceContextView({
  value
}: {
  value: unknown;
}) {
  if (!isRecord(value)) return null;
  const parent = isRecord(value.parentWork) ? value.parentWork : null;
  const comment = isRecord(value.parentComment) ? value.parentComment : null;
  const dossier = isRecord(value.dossier) ? value.dossier : null;
  const fragments = parent?.fragments;
  const coverage = dossier?.coverage ?? parent?.coverage;
  return <div className={styles.context}>
    {comment && comment.state !== 'not_applicable' ? <section>
<h3>回复语境</h3>{comment.state === 'known' && typeof comment.body === 'string' ? <blockquote>{comment.body}</blockquote> : <p>{comment.state === 'restricted' ? '父评论已受限' : comment.state === 'known' ? '父评论正文未知' : '父评论未知'}</p>}</section> : null}
    {parent ? <section>
<h3>父笔记</h3>{Array.isArray(fragments) ? fragments.map((f, i) => isRecord(f) && typeof f.text === 'string' ? <p className={styles.evidence} key={i}>{f.text}</p> : null) : <p>{parent.state === 'unavailable' ? '父笔记暂不可读' : '父笔记内容未知'}</p>}</section> : null}
    {coverage !== undefined ? <details>
<summary>取得范围</summary>
<pre>{JSON.stringify(coverage, null, 2)}</pre>
</details> : null}
  </div>;
}
function ReadingLeaf({
  strip,
  selection,
  onSelection
}: {
  strip: Strip;
  selection: Selection | null;
  onSelection: (s: Selection) => void;
}) {
  const host = useRef<HTMLDivElement>(null);
  function capture() {
    const root = host.current,
      selected = window.getSelection();
    if (!root || !selected || selected.isCollapsed || !selected.rangeCount) return;
    const range = selected.getRangeAt(0);
    if (!root.contains(range.startContainer) || !root.contains(range.endContainer)) return;
    const before = range.cloneRange();
    before.selectNodeContents(root);
    before.setEnd(range.startContainer, range.startOffset);
    const start = strip.start + Array.from(before.toString()).length;
    onSelection({
      start,
      end: start + Array.from(range.toString()).length,
      text: range.toString()
    });
  }
  const local = selection && selection.end > strip.start && selection.start < strip.end ? {
    start: Math.max(0, selection.start - strip.start),
    end: Math.min(strip.end - strip.start, selection.end - strip.start)
  } : null;
  const text = Array.from(strip.text);
  return <div ref={host} className={styles.sourceText} data-fragment-id={strip.id} data-source-ref={strip.source.sourceRef} data-work-ref={strip.source.workRef ?? undefined} tabIndex={0} aria-label={`可选择原文第 ${strip.index + 1} 段`} onMouseUp={capture} onKeyUp={capture}>
    {local ? <>{text.slice(0, local.start).join('')}<mark>{text.slice(local.start, local.end).join('')}</mark>{text.slice(local.end).join('')}</> : strip.text}
  </div>;
}
export function MaterialBrowser({
  contextKey = 'standalone',
  active: visibleSurface = true,
  domain,
  document,
  onCollect,
  focusCitation
}: Props) {
  const queryClient = useQueryClient();
  const [focused, setFocused] = useState(focusCitation);
  const [kind, setKind] = useState<SourceKind>(focusCitation?.sourceKind ?? 'note');
  const [search, setSearch] = useState(''),
    [query, setQuery] = useState('');
  const [cursor, setCursor] = useState(0),
    [usage, setUsage] = useState('all');
  const [active, setActive] = useState<Strip | null>(null),
    [range, setRange] = useState<Selection | null>(null);
  const [group, setGroup] = useState<CanvasGroup>('inbox');
  const [busy, setBusy] = useState(false),
    [error, setError] = useState('');
  useEffect(() => {
    if (!visibleSurface) {
      void queryClient.cancelQueries({
        queryKey: ['creation-sources', domain, contextKey]
      });
      void queryClient.cancelQueries({
        queryKey: ['creation-source', domain, contextKey]
      });
    }
  }, [visibleSurface, queryClient, domain, contextKey]);
  const list = useQuery({
    queryKey: ['creation-sources', domain, contextKey, kind, query, cursor],
    queryFn: ({
      signal
    }) => api.sources({
      domainRef: domain,
      kind,
      query,
      cursor
    }, signal),
    enabled: visibleSurface,
    staleTime: 0,
    gcTime: 0
  });
  const sourceQuery = useQuery({
    queryKey: ['creation-source', domain, contextKey, active?.source.sourceRef ?? focused?.sourceRef, active?.source.field ?? focused?.field, active?.source.sourceVersion ?? focused?.sourceVersion],
    queryFn: ({
      signal
    }) => {
      const selected = active?.source ?? focused;
      if (!selected) throw new Error('未选来源');
      return api.source(domain, selected, signal);
    },
    enabled: visibleSurface && (active !== null || !!focused),
    staleTime: 0,
    gcTime: 0
  });
  const all = useMemo(() => list.data?.items.flatMap(strips) ?? [], [list.data]);
  function matchingExcerpts(strip: Strip) {
    return document.excerpts.filter(excerpt => {
      const c = excerpt.citation,
        s = strip.source;
      return c.sourceKind === s.sourceKind && c.sourceRef === s.sourceRef && c.sourceVersion === s.sourceVersion && c.field === s.field && c.start < strip.end && c.end > strip.start;
    });
  }
  const visible = all.filter(strip => {
    const matches = matchingExcerpts(strip);
    if (usage === 'collected') return matches.length > 0;
    if (usage === 'uncollected') return matches.length === 0;
    if (usage !== 'all') return matches.some(excerpt => document.canvas.some(card => card.excerptId === excerpt.id && card.group === usage));
    return true;
  });
  const groups = new Map<string, Strip[]>();
  for (const s of visible) {
    const key = s.source.workRef ?? s.source.sourceRef;
    groups.set(key, [...(groups.get(key) ?? []), s]);
  }
  const current = sourceQuery.isSuccess ? sourceQuery.data.item : undefined;
  const requested = active?.source ?? focused;
  const changed = !!current && !!requested && current.sourceVersion !== requested.sourceVersion;
  const currentRange = !current || changed ? null : range ?? (focused && current.sourceRef === focused.sourceRef ? {
    start: focused.start,
    end: focused.end,
    text: Array.from(current.text).slice(focused.start, focused.end).join('')
  } : null);
  const originalStrips = current ? strips(current) : [];
  const selectedStrip = originalStrips.find(s => currentRange && s.start <= currentRange.start && s.end > currentRange.start);
  const selectedParagraph = originalStrips.findIndex(s => currentRange && s.start <= currentRange.start && s.end > currentRange.start);
  const spreadStart = Math.max(0, Math.floor(selectedParagraph / 2) * 2);
  const spreads = new Map<string, Strip>();
  for (const s of visible) {
    const key = `${s.source.fragmentId}:${Math.floor(s.index / 2)}`;
    if (!spreads.has(key)) spreads.set(key, s);
  }
  const spreadList = [...spreads.values()];
  const activeSpread = spreadList.findIndex(s => s.source.fragmentId === current?.fragmentId && Math.floor(s.index / 2) === Math.floor((originalStrips[spreadStart]?.index ?? 0) / 2));
  function clearSelection() {
    setActive(null);
    setFocused(null);
    setRange(null);
    setError('');
  }
  function choose(strip: Strip) {
    setFocused(null);
    setActive(strip);
    setRange({
      start: strip.start,
      end: strip.end,
      text: strip.text
    });
    setError('');
  }
  async function collect() {
    if (!current || !currentRange || currentRange.end <= currentRange.start) return;
    setBusy(true);
    setError('');
    try {
      const a = await api.anchor({
        domainRef: domain,
        sourceKind: current.sourceKind,
        sourceRef: current.sourceRef,
        workRef: current.workRef,
        field: current.field,
        sourceVersion: current.sourceVersion,
        start: currentRange.start,
        end: currentRange.end
      });
      onCollect(a, group);
    } catch (e) {
      setError(api.errorText(e));
    } finally {
      setBusy(false);
    }
  }
  const context = current ? sourceQuery.data?.context : null;
  const media = isRecord(context) && isRecord(context.dossier) && Array.isArray(context.dossier.media) ? context.dossier.media : [];
  const original = media.find(m => isRecord(m) && m.derivativeRef === current?.location?.derivativeRef);
  const imageUrl = isRecord(original) && typeof original.originalAssetUrl === 'string' && original.originalAssetUrl.startsWith('/api/local/media/') ? original.originalAssetUrl : null;
  return <div className={styles.materialBrowser}>
    <div className={styles.filterbar}>
      <div role="group" aria-label="材料类型">{(Object.keys(SOURCE_LABELS) as SourceKind[]).map(k => <button type="button" key={k} aria-pressed={kind === k} onClick={() => {
          setKind(k);
          setCursor(0);
          clearSelection();
        }}>{SOURCE_LABELS[k]}</button>)}</div>
      <form onSubmit={e => {
        e.preventDefault();
        setQuery(search);
        setCursor(0);
        clearSelection();
      }}>
<input aria-label="检索已取得材料" placeholder="检索已取得材料" value={search} onChange={e => setSearch(e.target.value)} />
<button>检索</button>
</form>
    </div>
    <div className={styles.filterbar}>
      <div role="group" aria-label="材料用途">{[{
          id: 'all',
          label: '全部'
        }, {
          id: 'collected',
          label: '已摘录'
        }, {
          id: 'uncollected',
          label: '未摘录'
        }, ...GROUPS.filter(g => g.id !== 'inbox')].map(g => <button type="button" key={g.id} aria-pressed={usage === g.id} onClick={() => {
          setUsage(g.id);
          clearSelection();
        }}>{g.label}</button>)}</div>
      <small>{list.data ? `本页 ${list.data.returnedSourceCount} 个来源 · ${visible.length} 个片段` : '读取中'}</small>
    </div>
    {list.error ? <p role="alert" className={styles.error}>{api.errorText(list.error)} <button onClick={() => void list.refetch()}>重新读取</button>
</p> : null}
    {changed ? <p className={styles.warning}>来源版本已变化。当前显示最新原文，请重新选取准确文字后摘录。</p> : null}
    <div className={styles.materialStage}>
      <section className={styles.readingSpace} aria-label="原文阅读">
        <div className={styles.bookMeta}>
<span>{current ? `${SOURCE_LABELS[current.sourceKind]} · ${current.authorLabel}` : '选择纸条，打开原文'}</span>
<span>{current?.bodyState === 'KNOWN' ? '已取得原文' : current?.bodyState === 'PARTIAL' ? '部分取得' : current ? '正文状态未知' : ''}</span>
</div>
        <div className={styles.openBook} key={current ? `${current.fragmentId}:${spreadStart}` : 'empty'}>
          {[0, 1].map(side => {
            const paragraph = originalStrips[spreadStart + side];
            return <article className={side === 0 ? styles.leftLeaf : styles.rightLeaf} key={side}>
            <div className={styles.running}>{side === 0 ? current?.title ?? '材料书页' : current ? '原文上下文' : '研读取材'}</div>
            {current && paragraph ? <>{side === 0 ? <h2>{current.title}</h2> : null}<ReadingLeaf strip={paragraph} selection={currentRange} onSelection={s => {
                  setRange(s);
                  setFocused(null);
                  setActive({
                    ...paragraph,
                    source: current
                  });
                }} />
<footer className={styles.leafFooter}>第 {paragraph.index + 1} 段</footer>
</> : sourceQuery.isFetching ? <p>读取来源中…</p> : sourceQuery.error ? <p className={styles.error} role="alert">{api.errorText(sourceQuery.error)}</p> : current && side === 1 ? <>
<SourceContextView value={context} />
<p className={styles.empty}>这一篇读到这里。</p>
</> : <p className={styles.empty}>从右侧选一条材料</p>}
          </article>;
          })}
        </div>
        <div className={styles.bookNav}>
<button disabled={activeSpread <= 0} onClick={() => {
            const s = spreadList[activeSpread - 1];
            if (s) choose(s);
          }}>← 上一展开页</button>
<span>{current ? `${spreadStart + 1}–${Math.min(spreadStart + 2, originalStrips.length)} / ${originalStrips.length} 段` : '未选择'}</span>
<button disabled={activeSpread < 0 || activeSpread >= spreadList.length - 1} onClick={() => {
            const s = spreadList[activeSpread + 1];
            if (s) choose(s);
          }}>下一展开页 →</button>
</div>
        <nav className={styles.distribution} aria-label="材料分布索引">{[...groups.entries()].map(([id, items], i) => <div className={styles.indexRow} key={id}>
<span>{String(i + 1).padStart(2, '0')}</span>{items.map(s => <button key={s.id} title={s.text} aria-label={`定位：${s.text}`} aria-current={selectedStrip?.id === s.id ? 'location' : undefined} data-current-source={current?.fragmentId === s.source.fragmentId} onClick={() => choose(s)}>
<i />
</button>)}</div>)}</nav>
        {current ? <details className={styles.sourceDrawer}>
<summary>原文上下文 / 来源详情</summary>
<SourceContextView value={context} />{current.location ? <>
<h3>图片文字位置</h3>{imageUrl ? <div className={styles.ocrImage}>
<img src={imageUrl} alt="OCR 来源原图" />{current.location.lineSpans.filter(s => !currentRange || s.end > currentRange.start && s.start < currentRange.end).map(l => <span key={l.lineRef} style={{
                left: `${l.bbox[0] * 100}%`,
                top: `${l.bbox[1] * 100}%`,
                width: `${l.bbox[2] * 100}%`,
                height: `${l.bbox[3] * 100}%`
              }} />)}</div> : <p>原图尚未取得；文字位置仍保留。</p>}</> : null}<dl>
<dt>资格</dt>
<dd>{current.qualification === 'readable' ? '可读' : current.qualification === 'restricted' ? '受限' : current.qualification === 'changed' ? '来源已变化' : '暂不可用'}</dd>
<dt>版本</dt>
<dd>{current.sourceVersion}</dd>
<dt>原文位置</dt>
<dd>{currentRange ? `${currentRange.start}–${currentRange.end}` : '未选取'}</dd>
</dl>
</details> : null}
      </section>
      <section className={styles.snippetWall} aria-label="分组摘录纸条">
        <div className={styles.wallMeta}>
<span>材料纸条</span>
<small>{list.data?.searchScope === 'current_anonymous_comment_text' ? '当前匿名评论' : list.data?.searchScope === 'current_export_title_body' ? '我方导出内容' : '已接纳标题 / 正文 / 历史目录'}</small>
</div>
        <div className={styles.snippetGroups}>{[...groups.entries()].map(([id, items], i) => <section key={id} className={styles.snippetGroup} data-tone={i % 3}>
<h3>{items[0]?.source.title || '未命名来源'} <small>{items.length}</small>
</h3>
<div>{items.map(s => <button type="button" key={s.id} className={styles.paper} aria-pressed={selectedStrip?.id === s.id} data-fragment-id={s.id} data-collected={matchingExcerpts(s).length > 0} data-focus={focusCitation?.sourceRef === s.source.sourceRef} onClick={() => choose(s)}>{s.text}</button>)}</div>
</section>)}</div>
        {list.data && !visible.length ? <p className={styles.empty}>当前范围没有可读材料。更换类型或关键词继续。</p> : null}
        <div className={styles.row}>
<button disabled={cursor === 0} onClick={() => {
            setCursor(Math.max(0, cursor - 30));
            clearSelection();
          }}>上一页来源</button>
<span>来源目录第 {Math.floor(cursor / 30) + 1} 页</span>
<button disabled={list.data?.nextCursor == null} onClick={() => {
            if (list.data?.nextCursor != null) {
              setCursor(list.data.nextCursor);
              clearSelection();
            }
          }}>更多来源</button>
</div>
      </section>
    </div>
    <div className={styles.selectedStrip}>
<div>
<small>{currentRange ? '已选原文' : '选择原文后摘录'}</small>{currentRange ? <blockquote>{currentRange.text}</blockquote> : null}{error ? <p className={styles.error} role="alert">{error}</p> : null}</div>
<div className={styles.row}>
<select aria-label="摘录归组" value={group} onChange={e => setGroup(e.target.value as CanvasGroup)}>{GROUPS.map(g => <option key={g.id} value={g.id}>{g.label}</option>)}</select>
<button className={styles.primary} disabled={!currentRange || !current || busy || !!sourceQuery.error} onClick={() => void collect()}>{busy ? '核验中…' : '摘入画布'}</button>
</div>
</div>
  </div>;
}
