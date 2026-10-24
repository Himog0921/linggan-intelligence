import { useState } from 'react';
import { useQuery } from '@tanstack/react-query';
import { requestJson } from '../../../shared/api';
import { decode, obj, optObj, optStr, optNum } from '../../../shared/decode';
import { DossierBlocks, DossierIssues } from './DossierBlocks';
import { emptyMaterialNote, hideCachedReading, materialLabel, readingLabel } from './dossierPresent';
import { ImageCompare } from './ImageCompare';
import { ReadingReviewPanel } from './ReadingReviewPanel';
import { Section } from './Primitives';
import { API_ROOT } from './evidencePaths';
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
  return <div className={styles.materials}>
    {dossier.note.platformTitle !== null && dossier.note.platformTitle !== displayedTitle ? <Section title="平台标题"><p className={styles.text}>{dossier.note.platformTitle}</p></Section> : null}
    {dossier.materials.length === 0 ? <p>当前尚无可读材料；这不等于原作品没有内容。</p> : null}
    {dossier.materials.map((unit, index) => <MaterialSection key={unit.ref} unit={unit} imageNumber={dossier.materials.slice(0, index + 1).filter(material => material.kind === 'image').length} work={work} domain={domain} onOpenImage={onOpenImage} onApplied={onApplied} />)}
  </div>;
}
function MaterialSection({ unit, work, domain, imageNumber, onOpenImage, onApplied }: { unit: DossierMaterial; imageNumber: number; work: string; domain: string; onOpenImage: (unit: DossierMaterial) => void; onApplied: () => void }) {
  const [originalReadable, setOriginalReadable] = useState(true);
  return <section className={styles.material} data-unit-ref={unit.ref}>
      <header className={styles.materialHead}><h3>{materialLabel(unit, imageNumber)}</h3>{unit.reading !== null || unit.blocks.length > 0 ? <span>{readingLabel(unit)}</span> : null}</header>
      {unit.blocks.length === 0 && !unit.issues.some(issue => issue.code === 'not_processed') ? <p className={styles.note}>{emptyMaterialNote(unit)}</p> : null}
      <DossierBlocks blocks={unit.blocks} issues={unit.issues.filter(issue => !(issue.code === 'page_order_unknown' && unit.page === null))} />
      {unit.kind === 'image' || unit.kind === 'video_frame' ? <ImageCompare key={unit.originalUrl} material={unit} onOpenImage={onOpenImage} onReadable={setOriginalReadable} /> : null}
      {unit.kind !== 'tags' && unit.sourceState === 'present' && (unit.currentRevisionRef !== null || unit.kind === 'image') ? <ReadingReviewPanel unit={unit} work={work} domain={domain} onApplied={onApplied} originalReadable={originalReadable} /> : null}
      <details className={styles.disclosure}><summary>来源与读本版本</summary>{unit.pageSource ? <p className={styles.note}>页序由已验证的同图来源确认。</p> : null}<pre className={styles.pre}>{JSON.stringify({ unitRef: unit.ref, sourceRef: unit.sourceRef, sourceVersion: unit.sourceVersion, currentRevisionRef: unit.currentRevisionRef, pageSource: unit.pageSource, reading: unit.reading }, null, 2)}</pre></details>
    </section>;
}
async function readMetadata(work: string, domain: string, asOf: string, signal: AbortSignal) {
  const value = await requestJson(`${API_ROOT}/${encodeURIComponent(work)}/research-dossier/metadata?domain=${encodeURIComponent(domain)}&asOf=${encodeURIComponent(asOf)}&sections=author,interaction`, { signal });
  return decode(value, root => { const data = obj(root, 'metadata'); const author = optObj(data, 'author'); const interaction = optObj(data, 'interaction');
    const metric = (key: string) => { const item = interaction ? optObj(interaction, key) : null; return item ? optNum(item, 'value') : null; };
    return { authorName: author ? optStr(author, 'displayName') : null, likes: metric('likes'), comments: metric('comments'), collects: metric('collects'), raw: value };
  });
}
export function MetaSection({ payload, domain }: { payload: DossierPayload; domain: string }) {
  const [open, setOpen] = useState(false);
  const { workRef, asOf } = payload.dossier;
  const query = useQuery({ queryKey: ['dossier-metadata', domain, workRef, asOf], queryFn: ({ signal }) => readMetadata(workRef, domain, asOf, signal), enabled: open, gcTime: 0 });
  return <details className={styles.advanced} onToggle={event => setOpen(event.currentTarget.open)}><summary>作者与互动信息</summary>
    {open && query.isPending ? <p role="status">正在读取来源信息…</p> : null}
    {query.isError ? <p role="status">来源信息读取失败。<button type="button" onClick={() => void query.refetch()}>重试</button></p> : null}
    {query.data && !hideCachedReading(query.error) ? <><p>{query.data.authorName ?? '作者尚未确认'}</p><p>{`点赞 ${query.data.likes ?? '未知'} · 评论 ${query.data.comments ?? '未知'} · 收藏 ${query.data.collects ?? '未知'}`}</p><details className={styles.disclosure}><summary>观察与来源诊断</summary><pre className={styles.pre}>{JSON.stringify(query.data.raw, null, 2)}</pre></details></> : null}
  </details>;
}
