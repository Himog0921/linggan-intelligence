import { useEffect, useRef, useState } from 'react';
import type { AssetRecord, ExportRecord, ImagePage, Revision, WorkDocument } from './workbenchModel';
import { ExportWorkflow, exportPackageUrl, exportPageUrl } from './exportWorkflow';
import type { ExportProgress, ExportTransport } from './exportWorkflow';
import { previewSplit, renderPage } from './renderer';
import styles from './ImagePages.module.css';
import { ApiError } from '../../shared/api';

interface Props {
  document: WorkDocument;
  workId: string;
  onChange: (document: WorkDocument) => void;
  onUploadAsset: (file: File) => Promise<AssetRecord>;
  disabled?: boolean;
  onSave?: () => Promise<Revision>;
  exports?: ExportRecord[];
  onExportComplete?: (record: ExportRecord) => void;
  exportTransport?: ExportTransport;
  onUndo?: () => void;
  onRedo?: () => void;
  canUndo?: boolean;
  canRedo?: boolean;
}
interface BodyChange { pageId: string; blockId: string; before: string; after: string; protected?: boolean }
const LABELS: Record<ImagePage['kind'], string> = { cover: '封面', opinion: '观点', voice: '原声', steps: '步骤' };

function CanvasPreview({ page, workId }: {page: ImagePage; workId: string}) {
  const host = useRef<HTMLCanvasElement>(null);
  const [status, setStatus] = useState('正在检查字体与图片');
  const [error, setError] = useState(false);
  const [statusPage, setStatusPage] = useState<ImagePage | null>(null);
  const [paintedPage, setPaintedPage] = useState<ImagePage | null>(null);
  useEffect(() => {
    const controller = new AbortController();
    // Draw on a detached canvas so an older async render cannot overwrite a newer preview.
    void renderPage(page, { signal: controller.signal,
      assetUrl: page.assetId ? `/ops/api/creation/works/${workId}/assets/${page.assetId}` : undefined,
    }).then(result => {
      if (controller.signal.aborted || !host.current) return;
      host.current.width = result.canvas.width; host.current.height = result.canvas.height;
      host.current.getContext('2d')?.drawImage(result.canvas, 0, 0);
      setStatusPage(page); setPaintedPage(page);
      setError(result.issues.length > 0);
      setStatus(result.issues.length ? [...new Set(result.issues.map(issue => issue.message))].join('；') : '画幅检查通过');
    }).catch((cause: unknown) => {
      if (controller.signal.aborted) return;
      setStatusPage(page); setPaintedPage(null);
      setError(true); setStatus(cause instanceof Error ? cause.message : '预览失败，请重新打开或替换图片');
    });
    return () => controller.abort();
  }, [page, workId]);
  return <figure className={styles.preview}>
    <canvas ref={host} width={1080} height={1440} style={{visibility: paintedPage === page ? 'visible' : 'hidden'}} aria-label={`${LABELS[page.kind]}预览：${page.title || '尚未填写'}`} />
    <figcaption role="status" className={error ? styles.error : styles.note}>{statusPage === page ? status : '正在检查字体与图片'}</figcaption>
  </figure>;
}

function newPage(kind: ImagePage['kind']): ImagePage {
  return { id: crypto.randomUUID(), kind, templateVersion: 'creation-v1', title: '', subtitle: '',
    blocks: [{ id: crypto.randomUUID(), text: '' }], assetId: null, crop: { x: 0, y: 0, width: 1, height: 1 },
    style: { palette: 'cream', fontFamily: 'system-cjk', width: 1080, height: 1440, alignment: 'start' } };
}

const progressLabel = (value: ExportProgress) => ({ saving: '正在保存作品', freezing: '正在冻结保存版本',
  rendering: `正在绘制第 ${value.page ?? ''} / ${value.total ?? ''} 张`, uploading: `正在登记第 ${value.page ?? ''} / ${value.total ?? ''} 张`,
  finalizing: '正在核对并打包', complete: '图文包已生成，可下载' })[value.stage];

export function ImagePages(props: Props) {
  const { document: documentValue, workId, onChange, disabled = false } = props;
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [deleted, setDeleted] = useState<{page: ImagePage; index: number}[]>([]);
  const [titleProposal, setTitleProposal] = useState<string | null>(null);
  const [titleUndo, setTitleUndo] = useState<{pageId: string; before: string; after: string} | null>(null);
  const [split, setSplit] = useState<{original: ImagePage; pages: ImagePage[]} | null>(null);
  const [allPreview, setAllPreview] = useState(false);
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState('');
  const [pendingUpload, setPendingUpload] = useState<{file: File; pageId: string} | null>(null);
  const [error, setError] = useState('');
  const [localExports, setLocalExports] = useState<ExportRecord[]>([]);
  const [bodyChanges, setBodyChanges] = useState<BodyChange[]>([]);
  const dismissedChanges = useRef(new Set<string>());
  const latestDocument = useRef(documentValue);
  const [exportRetryAvailable, setExportRetryAvailable] = useState(false);
  const [exportUnconfirmed, setExportUnconfirmed] = useState(false);
  useEffect(() => { latestDocument.current = documentValue; }, [documentValue]);
  const workflow = useRef<ExportWorkflow | null>(null);
  const page = documentValue.pages.find(item => item.id === selectedId) ?? documentValue.pages[0];
  const selected = page?.id;

  useEffect(() => {
    let active = true;
    const digest = async (value: string) => Array.from(new Uint8Array(await crypto.subtle.digest('SHA-256', new TextEncoder().encode(value))),
      byte => byte.toString(16).padStart(2,'0')).join('');
    // Material-use hashes survive navigation/reload; local component history alone cannot prove A09.
    const findPersistedChanges = async () => {
      const changes: BodyChange[] = [];
      for (const bodyUse of documentValue.uses) {
        if (bodyUse.target.kind !== 'paragraph' || bodyUse.state === 'needs_review') continue;
        const target = bodyUse.target;
        const paragraph = documentValue.paragraphs.find(item => item.id === target.paragraphId);
        if (!paragraph || await digest(paragraph.text) === bodyUse.placedTextHash) continue;
        for (const pageUse of documentValue.uses) {
          if (pageUse.target.kind !== 'pageBlock' || pageUse.canvasId !== bodyUse.canvasId) continue;
          const pageTarget = pageUse.target;
          const block = documentValue.pages.find(item => item.id === pageTarget.pageId)?.blocks.find(item => item.id === pageTarget.blockId);
          if (!block || block.text === paragraph.text) continue;
          const change: BodyChange = {pageId: pageTarget.pageId, blockId: pageTarget.blockId, before: block.text, after: paragraph.text,
            protected: pageUse.state !== 'current' || await digest(block.text) !== pageUse.placedTextHash};
          const key = JSON.stringify([change.pageId, change.blockId, change.after]);
          if (!dismissedChanges.current.has(key) && !changes.some(item => item.pageId === change.pageId && item.blockId === change.blockId && item.after === change.after)) changes.push(change);
        }
      }
      if (active) setBodyChanges(changes);
    };
    void findPersistedChanges().catch(() => { /* Existing unsaved-change prompts remain; no hash guess authorizes replacement. */ });
    return () => { active = false; };
  }, [documentValue]);


  const replacePage = (updated: ImagePage) => onChange({ ...documentValue, pages: documentValue.pages.map(item => item.id === updated.id ? updated : item) });
  const patch = (update: Partial<ImagePage>) => { if (page) replacePage({ ...page, ...update }); setSplit(null); };
  const addPage = (kind: ImagePage['kind']) => {
    const added = newPage(kind);
    onChange({ ...documentValue, pages: kind === 'cover' ? [added, ...documentValue.pages] : [...documentValue.pages, added] });
    setSelectedId(added.id);
  };
  const movePage = (direction: -1 | 1) => {
    if (!page || page.kind === 'cover') return;
    const pages = [...documentValue.pages], index = pages.findIndex(item => item.id === page.id), next = index + direction;
    const neighbor = pages[next];
    if (!neighbor || neighbor.kind === 'cover') return;
    pages[index] = neighbor; pages[next] = page; onChange({ ...documentValue, pages });
  };
  const deletePage = () => {
    if (!page || page.kind === 'cover') return;
    setDeleted(current => [...current, {page, index: documentValue.pages.findIndex(item => item.id === page.id)}]);
    onChange({ ...documentValue, pages: documentValue.pages.filter(item => item.id !== page.id) });
    setSelectedId(null); setSplit(null);
  };
  const recoverPage = () => {
    const last = deleted.at(-1); if (!last) return;
    const pages = [...documentValue.pages]; pages.splice(Math.min(last.index, pages.length), 0, last.page);
    onChange({ ...documentValue, pages }); setDeleted(current => current.slice(0, -1)); setSelectedId(last.page.id);
  };
  const upload = async (file?: File, retryPageId?: string) => {
    if (!file || !page) return;
    const targetId = retryPageId ?? page.id;
    setPendingUpload({file,pageId:targetId});
    setBusy(true); setError('');
    try {
      const asset = await props.onUploadAsset(file);
      const latest = latestDocument.current;
      onChange({ ...latest, pages: latest.pages.map(item => item.id === targetId ? { ...item, assetId: asset.id, crop: {x: 0, y: 0, width: 1, height: 1} } : item) });
      setMessage('自有图片已上传；保存作品后保留图片关联');
      setPendingUpload(null);
    } catch (cause) {
      if (cause instanceof ApiError && cause.answered) { setPendingUpload(null); setError('服务器未接纳这张图片，原图片仍保留。请检查图片格式与大小后重新选择。'); }
      else setError('图片上传结果尚未确认，原文件与原图片保留。请核实原请求并重试图片上传。');
    } finally { setBusy(false); }
  };
  const runExport = async (mode: 'new' | 'retry', record?: ExportRecord, singlePageId?: string) => {
    if (!props.exportTransport || !props.onSave) return;
    if (!workflow.current) workflow.current = new ExportWorkflow(props.exportTransport);
    setBusy(true); setError('');
    try {
      const progress = (value: ExportProgress) => setMessage(progressLabel(value));
      const result = record ? await workflow.current.resume(record, progress) : mode === 'retry' ? await workflow.current.retry(progress) :
        await workflow.current.start(workId, props.onSave, progress, singlePageId);
      setLocalExports(current => [result, ...current.filter(item => item.id !== result.id)]);
      props.onExportComplete?.(result);
      if (singlePageId) setMessage('单页 PNG 已生成，可在下方下载；其余图页可继续生成图文包');
    } catch (cause) {
      const prefix = workflow.current.hasUnconfirmed ? '生成结果尚未确认，请先核实原请求' : '生成未完成';
      setError(cause instanceof Error ? `${prefix}：${cause.message}。可重试当前冻结版本；若已修正文案，请生成新版。` : `${prefix}，请重试当前版本`);
    } finally { setExportRetryAvailable(workflow.current?.canRetry ?? false); setExportUnconfirmed(workflow.current?.hasUnconfirmed ?? false); setBusy(false); }
  };
  const records = [...localExports, ...(props.exports ?? []).filter(item => !localExports.some(local => local.id === item.id))];
  const locked = disabled || busy;
  const affected = bodyChanges.filter(change => change.pageId === selected);
  return <section className={styles.root} aria-label="封面与内页">
    <div className={styles.toolbar}>
      <div className={styles.actions}>
        {props.onUndo && <button disabled={locked || !props.canUndo} onClick={props.onUndo}>撤销图页编辑</button>}
        {props.onRedo && <button disabled={locked || !props.canRedo} onClick={props.onRedo}>重做图页编辑</button>}
        {!documentValue.pages.some(item => item.kind === 'cover') && <button disabled={locked} onClick={() => addPage('cover')}>添加封面</button>}
        {(['opinion', 'voice', 'steps'] as const).map(kind => <button key={kind} disabled={locked || documentValue.pages.length >= 30} onClick={() => addPage(kind)}>添加{LABELS[kind]}页</button>)}
        {deleted.length > 0 && <button disabled={locked || documentValue.pages.length >= 30} onClick={recoverPage}>恢复删除页</button>}
      </div>
      <button aria-pressed={allPreview} onClick={() => setAllPreview(value => !value)}>{allPreview ? '返回编辑' : '整组预览'}</button>
    </div>
    {error && <p role="alert" className={styles.error}>{error}</p>}
    {pendingUpload && <button disabled={locked} onClick={() => { void upload(pendingUpload.file, pendingUpload.pageId); }}>核实并重试原图片上传</button>}
    {message && <p role="status" className={styles.note}>{message}</p>}
    {!page ? <p>还没有图页。添加封面和内页，开始制作图组。</p> : <>
      <nav aria-label="图页顺序" className={styles.pageTabs}>{documentValue.pages.map((item, index) =>
        <button key={item.id} aria-current={item.id === selected ? 'page' : undefined} onClick={() => { setSelectedId(item.id); setTitleProposal(null); setSplit(null); setAllPreview(false); }}>
          {String(index + 1).padStart(2, '0')} {LABELS[item.kind]}{item.title ? ` · ${item.title}` : ' · 尚未填写'}{bodyChanges.some(change => change.pageId === item.id) ? ' · 正文已更新' : ''}
        </button>)}</nav>
      {allPreview ? <div className={styles.previewGrid}>{documentValue.pages.map(item => <div key={item.id}>
        <CanvasPreview page={item} workId={workId}/><button onClick={() => { setSelectedId(item.id); setAllPreview(false); }}>编辑此页</button>
      </div>)}</div> : <div className={styles.editorLayout}>
        <div className={styles.controls}>
          <fieldset disabled={locked}>
            <legend>{LABELS[page.kind]}编辑</legend>
            <div className={styles.actions}><button onClick={() => movePage(-1)} disabled={locked || page.kind === 'cover' || documentValue.pages.indexOf(page) <= 1}>上移</button>
              <button onClick={() => movePage(1)} disabled={locked || page.kind === 'cover' || documentValue.pages.at(-1)?.id === selected}>下移</button>
              {page.kind !== 'cover' && <button onClick={deletePage}>删除此页</button>}</div>
            <label>模板<select aria-label="图页模板" value={page.templateVersion} onChange={event => patch({templateVersion: event.target.value as ImagePage['templateVersion']})}>
              <option value="creation-v1">图像在上</option><option value="creation-v1-editorial">标题在上</option></select></label>
            {page.kind !== 'cover' && <label>内页结构<select aria-label="内页结构" value={page.kind} onChange={event => patch({kind: event.target.value as ImagePage['kind']})}>
              <option value="opinion">观点</option><option value="voice">原声</option><option value="steps">步骤</option></select></label>}
            <label>图页标题<textarea aria-label="图页标题" rows={3} value={page.title} onChange={event => patch({title: event.target.value})}/></label>
            {page.kind === 'cover' && <button onClick={() => setTitleProposal(documentValue.title)}>预览带入正文标题</button>}
            {titleProposal !== null && <div className={styles.notice}><p>将使用：{titleProposal || '正文标题尚未填写'}</p>
              <button disabled={!titleProposal} onClick={() => { setTitleUndo({pageId: page.id, before: page.title, after: titleProposal}); patch({title: titleProposal}); setTitleProposal(null); }}>确认带入标题</button>
              <button onClick={() => setTitleProposal(null)}>取消带入</button></div>}
            {titleUndo?.pageId === page.id && <button disabled={page.title !== titleUndo.after} onClick={() => { patch({title: titleUndo.before}); setTitleUndo(null); }}>撤销标题带入</button>}
            <label>副标题<textarea aria-label="副标题" rows={2} value={page.subtitle} onChange={event => patch({subtitle: event.target.value})}/></label>
            <div className={styles.twoFields}><label>配色<select aria-label="图页配色" value={page.style.palette} onChange={event => patch({style: {...page.style, palette: event.target.value as ImagePage['style']['palette']}})}>
              <option value="cream">米白</option><option value="sage">鼠尾草绿</option><option value="ink">墨绿</option></select></label>
              <label>文字对齐<select aria-label="图页文字对齐" value={page.style.alignment ?? 'start'} onChange={event => patch({style: {...page.style, alignment: event.target.value as ImagePage['style']['alignment']}})}>
                <option value="start">居左</option><option value="center">居中</option><option value="end">居右</option></select></label></div>
            {page.blocks.map((block, index) => <div key={block.id}><label>{page.kind === 'steps' ? `步骤 ${index + 1}` : `文案 ${index + 1}`}<textarea aria-label={page.kind === 'steps' ? `步骤 ${index + 1}` : `文案 ${index + 1}`} rows={4} value={block.text}
              onChange={event => patch({blocks: page.blocks.map(item => item.id === block.id ? {...item, text: event.target.value} : item)})}/></label>
              <button onClick={() => patch({blocks: page.blocks.filter(item => item.id !== block.id)})}>移除文案 {index + 1}</button></div>)}
            <button disabled={page.blocks.length >= 30} onClick={() => patch({blocks: [...page.blocks, {id: crypto.randomUUID(), text: ''}]})}>添加文案</button>
            <label>自有图片<input aria-label="自有图片" type="file" disabled={pendingUpload !== null} accept="image/png,image/jpeg,image/webp" onChange={event => { void upload(event.target.files?.[0]); event.target.value = ''; }}/></label>
            {page.assetId && <><div className={styles.twoFields}>{(['x','y','width','height'] as const).map(key => <label key={key}>{({x:'横向位置',y:'纵向位置',width:'裁切宽度',height:'裁切高度'})[key]}
              <input type="number" min={key === 'width' || key === 'height' ? 1 : 0} max={100} value={Math.round(page.crop[key] * 100)} onChange={event => {
                const raw = Number(event.target.value) / 100;
                const crop = {...page.crop, [key]: Math.min(1, Math.max(key === 'width' || key === 'height' ? 0.01 : 0, raw))};
                crop.x = Math.min(crop.x, 1 - crop.width); crop.y = Math.min(crop.y, 1 - crop.height); patch({crop});
              }}/></label>)}</div><button onClick={() => patch({assetId: null})}>移除图片</button></>}
            <button onClick={() => { setBusy(true); setError(''); void previewSplit(page, {assetUrl: page.assetId ? `/ops/api/creation/works/${workId}/assets/${page.assetId}` : undefined})
              .then(pages => setSplit({original: page, pages})).catch((cause: unknown) => setError(cause instanceof Error ? cause.message : '拆页预览失败')).finally(() => setBusy(false)); }}>预览长文拆页</button>
          </fieldset>
          {affected.length > 0 && <div className={styles.notice}><p>关联正文已修改。核对后可带入此页。</p>{affected.map(change => {
            const current = page.blocks.find(block => block.id === change.blockId);
            const protectedEdit = change.protected || current?.text !== change.before;
            return <div key={change.blockId}><p>{change.after}</p><p className={styles.note}>{protectedEdit ? '此处已手动修改，保留当前文字。' : '确认后替换此段，其余图页保持原稿。'}</p>
              <button disabled={locked || protectedEdit} onClick={() => { patch({blocks: page.blocks.map(block => block.id === change.blockId ? {...block, text: change.after} : block)}); setBodyChanges(items => items.filter(item => item !== change)); }}>带入这一段</button>
              <button onClick={() => { dismissedChanges.current.add(JSON.stringify([change.pageId, change.blockId, change.after])); setBodyChanges(items => items.filter(item => item !== change)); }}>保留图页原稿</button></div>;
          })}</div>}
        </div>
        <div className={styles.editPreview}><CanvasPreview page={page} workId={workId}/>{split && <div className={styles.notice}><p>拆页预览：{split.pages.length} 张；确认后替换当前页。关联文字拆开后需要重新核对出处。</p>
          <div className={styles.previewGrid}>{split.pages.map(item => <CanvasPreview key={item.id} page={item} workId={workId}/>)}</div>
          <button disabled={locked || documentValue.pages.length - 1 + split.pages.length > 30 || page !== split.original} onClick={() => {
            const index = documentValue.pages.findIndex(item => item.id === page.id), pages = [...documentValue.pages]; pages.splice(index, 1, ...split.pages);
            onChange({...documentValue, pages}); setSplit(null);
          }}>确认拆页</button><button onClick={() => setSplit(null)}>取消拆页</button></div>}</div>
      </div>}
    </>}
    <div className={styles.exportArea}>
      <div className={styles.actions}><button className={styles.primary} disabled={locked || exportUnconfirmed || !props.onSave || !props.exportTransport || !documentValue.accountRef || !documentValue.pages.length}
        onClick={() => { void runExport('new'); }}>保存并生成图文包</button>
        <button disabled={locked || exportUnconfirmed || !selected || !props.onSave || !props.exportTransport || !documentValue.accountRef}
          onClick={() => { if (selected) void runExport('new', undefined, selected); }}>保存并生成此页 PNG</button>
        {exportRetryAvailable && error && <button disabled={locked} onClick={() => { void runExport('retry'); }}>重试当前冻结版本</button>}</div>
      {!documentValue.accountRef && <p className={styles.note}>导出前先在作品中选择账号。</p>}
      {records.length > 0 && <details><summary>已生成与中断的图文包（{records.length}）</summary>{records.map(record => <div key={record.id} className={styles.exportRecord}>
        <p>{record.state === 'complete' ? '已完成图文包' : record.state === 'failed' ? '生成失败' : record.pages.some(item => item.sha256) ? '部分图页已生成' : '生成中断'} · 冻结版本 {record.revisionId.slice(0, 8)}</p>
        <div className={styles.actions}>{record.pages.filter(item => item.sha256).map(item =>
          <a key={item.pageId} href={exportPageUrl(record.id, item.pageId)} download={`${String(item.ordinal).padStart(2, '0')}.png`}>下载第 {item.ordinal} 张 PNG</a>)}</div>
        {record.state === 'complete' ? <a href={exportPackageUrl(record.id)} download>重新下载图文包</a> :
          <button disabled={locked || exportUnconfirmed || !props.exportTransport} onClick={() => { void runExport('retry', record); }}>继续生成原冻结版本</button>}
      </div>)}</details>}
    </div>
  </section>;
}
