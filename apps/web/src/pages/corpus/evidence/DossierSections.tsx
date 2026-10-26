import { useRef, useState } from 'react';
import { useQuery } from '@tanstack/react-query';
import { requestJson } from '../../../shared/api';
import { decode, obj, optObj, optStr, optNum } from '../../../shared/decode';
import { DossierIssues } from './DossierBlocks';
import { DossierReading } from './DossierReading';
import { emptyMaterialNote, hideCachedReading, materialIdentity, materialLabel, readingLabel } from './dossierPresent';
import { ImageCompare } from './ImageCompare';
import { ReadingReviewPanel } from './ReadingReviewPanel';
import { API_ROOT } from './evidencePaths';
import { momentText } from './evidencePresent';
import type { DossierMaterial, DossierPayload } from './dossierTypes';
import styles from './Dossier.module.css';

export function CoverageSection({ payload }: { payload: DossierPayload }) {
  const { imageInventory: images, issues } = payload.dossier.coverage;
  const materialIssues = payload.dossier.materials.flatMap(unit => [...unit.issues, ...unit.blocks.flatMap(block => block.issues ?? [])]);
  const summaryIssues = issues.filter(issue => !(issue.code === 'image_total_unknown' && images.expectedPages === null) && !materialIssues.some(local => local.key === issue.key));
  return <section className={styles.coverage} aria-label="材料覆盖">
    <p>{images.expectedPages === null ? `已取得 ${images.acquiredPages} 张图片，已有读本 ${images.processedPages} 页，原作总页数未知。` : `原作 ${images.expectedPages} 页，已取得 ${images.acquiredPages} 页，已有读本 ${images.processedPages} 页。`}</p>
    {images.missingPages.length > 0 ? <p>尚未取得第 {images.missingPages.join('、')} 页。</p> : null}
    {images.unknownOrder ? <p className={styles.note}>部分图片的原作顺序尚未确认。</p> : null}
    {payload.dossier.selection.mode === 'explicit' ? <p className={styles.note}>当前是显式选择的材料范围，另有 {payload.dossier.selection.omittedUnits} 项未选入。</p> : null}
    <DossierIssues issues={summaryIssues} />
  </section>;
}
export function MaterialsSection({ payload, work, domain, displayedTitle, onOpenImage, onApplied }: { payload: DossierPayload; displayedTitle?: string; work: string; domain: string; onOpenImage: (unit: DossierMaterial) => void; onApplied: () => void }) {
  const { dossier } = payload;
  const [initiallyOpen] = useState(() => dossier.materials.every(unit => unit.blocks.length === 0));
  const disclosure = useRef<HTMLDetailsElement>(null);
  const originals = useRef<HTMLDivElement>(null);
  function openSource(unit: DossierMaterial) {
    if (disclosure.current) disclosure.current.open = true;
    const source = [...(originals.current?.querySelectorAll<HTMLElement>('[data-source-unit-ref]') ?? [])].find(node => node.dataset.sourceUnitRef === unit.ref);
    if (source instanceof HTMLDetailsElement) source.open = true;
    const image = unit.kind === 'image' ? source?.querySelector('details') : null;
    if (image) image.open = true;
    source?.scrollIntoView({ block: 'start', behavior: 'smooth' });
    source?.focus({ preventScroll: true });
  }
  return <>
    {displayedTitle === undefined || displayedTitle !== (dossier.note.displayTitle ?? '无标题') ? <header className={styles.readingTitle}><h2>{dossier.note.displayTitle ?? '无标题'}</h2>{dossier.note.displayTitleBasis === 'cover' ? <span className={styles.note}>封面整理标题</span> : null}</header> : null}
    <DossierReading dossier={dossier} titleRendered onSource={openSource} />
    <details ref={disclosure} className={styles.originals} open={initiallyOpen}><summary>原件与修正 · {dossier.materials.length} 项</summary><div ref={originals}>
      {dossier.materials.length === 0 ? <p className={styles.note}>暂无已取得的原件。</p> : null}
      {dossier.materials.map((unit, index) => <MaterialSource key={materialIdentity(unit)} unit={unit} imageNumber={dossier.materials.slice(0, index + 1).filter(material => material.kind === 'image').length} work={work} domain={domain} onOpenImage={onOpenImage} onApplied={onApplied} />)}
    </div></details>
  </>;
}
function MaterialSource({ unit, work, domain, imageNumber, onOpenImage, onApplied }: { unit: DossierMaterial; imageNumber: number; work: string; domain: string; onOpenImage: (unit: DossierMaterial) => void; onApplied: () => void }) {
  const [originalReadable, setOriginalReadable] = useState(true);
  if (unit.kind === 'body' && unit.blocks.length === 0 && unit.sourceState !== 'present') return <details className={`${styles.disclosure} ${styles.anchor}`} data-source-unit-ref={unit.ref} tabIndex={-1}>
    <summary>正文来源状态</summary><p className={styles.note}>{emptyMaterialNote(unit)}</p><DossierIssues issues={unit.issues} />
    <pre className={styles.pre}>{JSON.stringify({ unitRef: unit.ref, sourceState: unit.sourceState, sourceRef: unit.sourceRef, sourceVersion: unit.sourceVersion }, null, 2)}</pre>
  </details>;
  return <section className={`${styles.material} ${styles.anchor}`} data-source-unit-ref={unit.ref} tabIndex={-1}>
    <header className={styles.materialHead}><h4>{materialLabel(unit, imageNumber)} · 原件</h4></header>
    {unit.kind === 'image' ? <ImageCompare key={unit.originalUrl} material={unit} onOpenImage={onOpenImage} onReadable={setOriginalReadable} /> : unit.originalUrl ? <a href={unit.originalUrl} target="_blank" rel="noreferrer">查看音视频原件</a> : null}
    {unit.kind !== 'tags' && unit.sourceState === 'present' && (unit.currentRevisionRef !== null || unit.kind === 'image') ? <ReadingReviewPanel unit={unit} work={work} domain={domain} onApplied={onApplied} originalReadable={originalReadable} /> : null}
    <details className={styles.disclosure}><summary>来源与读本版本</summary>{unit.reading !== null ? <p className={styles.note}>{readingLabel(unit)}</p> : null}{unit.pageSource ? <p className={styles.note}>页序由已验证的同图来源确认。</p> : null}<pre className={styles.pre}>{JSON.stringify({ unitRef: unit.ref, sourceRef: unit.sourceRef, sourceVersion: unit.sourceVersion, currentRevisionRef: unit.currentRevisionRef, processing: unit.processing, issues: unit.issues, pageSource: unit.pageSource, reading: unit.reading }, null, 2)}</pre></details>
  </section>;
}
async function readMetadata(work: string, domain: string, asOf: string, signal: AbortSignal) {
  const value = await requestJson(`${API_ROOT}/${encodeURIComponent(work)}/research-dossier/metadata?domain=${encodeURIComponent(domain)}&asOf=${encodeURIComponent(asOf)}&sections=author,interaction,publication`, { signal });
  return decode(value, root => { const data = obj(root, 'metadata'); const author = optObj(data, 'author'); const interaction = optObj(data, 'interaction'); const publication = obj(data, 'publication');
    const metric = (key: string) => { const item = interaction ? optObj(interaction, key) : null; return item ? optNum(item, 'value') : null; };
    return { authorName: author ? optStr(author, 'displayName') : null, likes: metric('likes'), comments: metric('comments'), collects: metric('collects'), shares: metric('shares'),
      publication: { publishedAt: optStr(publication, 'publishedAt'), localDate: optStr(publication, 'localDate'), sourceText: optStr(publication, 'sourceText'), precision: optStr(publication, 'precision'), sourceKind: optStr(publication, 'sourceKind'), observedAt: optStr(publication, 'observedAt'), referenceObservedAt: optStr(publication, 'referenceObservedAt') }, raw: value };
  });
}
export function MetaSection({ payload, domain }: { payload: DossierPayload; domain: string }) {
  const [open, setOpen] = useState(false);
  const { workRef, asOf } = payload.dossier;
  const query = useQuery({ queryKey: ['dossier-metadata', domain, workRef, asOf], queryFn: ({ signal }) => readMetadata(workRef, domain, asOf, signal), enabled: open, gcTime: 0 });
  return <details className={styles.advanced} onToggle={event => setOpen(event.currentTarget.open)}><summary>作者、时间与互动</summary>
    {open && query.isPending ? <p role="status">正在读取来源信息…</p> : null}
    {query.isError ? <p role="status">来源信息读取失败。<button type="button" onClick={() => void query.refetch()}>重试</button></p> : null}
    {query.data && !hideCachedReading(query.error) ? <><p>{query.data.authorName ?? '作者尚未确认'}</p>
      <p>发布时间：{(query.data.publication.sourceKind === 'source_text' ? query.data.publication.sourceText?.trim() : null) || (query.data.publication.precision === 'day' ? query.data.publication.localDate || query.data.publication.publishedAt : query.data.publication.publishedAt || query.data.publication.localDate) || query.data.publication.sourceText?.trim() || '未知'}{query.data.publication.precision && query.data.publication.precision !== 'unknown' ? ` · 精度：${({ day: '日', hour: '小时', minute: '分钟', second: '秒', millisecond: '毫秒', month: '月', year: '年' } as Record<string, string>)[query.data.publication.precision] ?? '未知'}` : ''}</p>
      <p>观察时间：{momentText(query.data.publication.observedAt, '未知')}</p>
      {query.data.publication.referenceObservedAt && query.data.publication.sourceText ? <p className={styles.note}>来源时间表达的观察时刻：{momentText(query.data.publication.referenceObservedAt, '未知')}</p> : null}
      <p>{`点赞 ${query.data.likes ?? '未知'} · 评论 ${query.data.comments ?? '未知'} · 收藏 ${query.data.collects ?? '未知'} · 分享 ${query.data.shares ?? '未知'}`}</p><details className={styles.disclosure}><summary>观察与来源诊断</summary><pre className={styles.pre}>{JSON.stringify(query.data.raw, null, 2)}</pre></details></> : null}
  </details>;
}
