import { ActionButton } from '../../../shared/ActionButton';
import { useQuery, useQueryClient } from '@tanstack/react-query';
import { useEffect, useRef, useState } from 'react';
import { hideCachedReading } from './dossierPresent';
import { fetchDossier } from './dossierApi';
import { CoverageSection, MaterialsSection, MetaSection } from './DossierSections';
import type { DossierMaterial, DossierPayload } from './dossierTypes';
import { Section } from './Primitives';
import { DOSSIER_KEY, dossierKey } from './evidenceKeys';
import { downloadDossier, downloadDossierMarkdown } from './exportDossier';
import { OcrRecognition } from './OcrRecognition';
import styles from './Dossier.module.css';
interface Props { work: string; domain: string; active: boolean; displayedTitle?: string; onOpenImage: (material: DossierMaterial) => void; onApplied: () => void }
function Head({ payload, domain, busy, onRefresh }: { payload: DossierPayload; domain: string; busy: boolean; onRefresh: () => void }) {
  const [exporting, setExporting] = useState(false); const [message, setMessage] = useState('');
  const mounted = useRef(true); useEffect(() => { mounted.current = true; return () => { mounted.current = false; }; }, []);
  async function markdown() {
    setExporting(true); setMessage('');
    try { await downloadDossierMarkdown(payload, domain); } catch { if (mounted.current) setMessage('Markdown 导出未完成，请重试读取当前版本。'); }
    finally { if (mounted.current) setExporting(false); }
  }
  return <Section title="研究底稿">
    <div className={styles.toolbar}>
      <ActionButton variant="outline" disabled={exporting} onClick={() => void markdown()}>导出 Markdown</ActionButton>
      <ActionButton variant="outline" onClick={() => downloadDossier(payload)}>导出 JSON</ActionButton>
      <ActionButton variant="quiet" disabled={busy} onClick={onRefresh}>刷新底稿</ActionButton>
    </div>{message ? <p role="status">{message}</p> : null}
  </Section>;
}
/** The human reading and on-demand exports share the same canonical snapshot. */
export function DossierPanel({ work, domain, active, displayedTitle, onOpenImage, onApplied }: Props) {
  const [activated, setActivated] = useState(active); if (active && !activated) setActivated(true);
  const client = useQueryClient();
  const query = useQuery({ queryKey: dossierKey(domain, work), queryFn: ({ signal }) => fetchDossier(work, domain, signal), enabled: activated, staleTime: 30_000, gcTime: 0 });
  const payload = hideCachedReading(query.error) ? null : query.data ?? null;
  const applied = () => { void client.invalidateQueries({ queryKey: [DOSSIER_KEY] }); onApplied(); };
  if (payload === null) return query.isError ? <div className={styles.panel}><p role="status">研究底稿暂时不可读。</p><ActionButton onClick={() => void query.refetch()}>重试读取底稿</ActionButton></div> : activated ? <p className={styles.busy} aria-busy="true">正在读取研究底稿…</p> : null;
  return <div className={styles.panel} aria-busy={query.isFetching}>
    <Head key={`head:${domain}:${work}`} payload={payload} domain={domain} busy={query.isFetching} onRefresh={() => void query.refetch()} />
    {query.isError ? <p role="status">刷新未完成，当前保留上一次读到的版本。<button type="button" onClick={() => void query.refetch()}>重试刷新</button></p> : null}
    <MaterialsSection displayedTitle={displayedTitle} payload={payload} work={work} domain={domain} onOpenImage={onOpenImage} onApplied={applied} />
    <OcrRecognition key={`ocr:${domain}:${work}`} workRef={work} active={active} readingRevision={payload.dossier.materialRevision} onApplied={applied} />
    <MetaSection payload={payload} domain={domain} />
    <details className={styles.advanced}><summary>材料详情</summary><CoverageSection payload={payload} /><pre className={styles.pre}>{JSON.stringify({ revision: payload.dossier.revision, materialRevision: payload.dossier.materialRevision, asOf: payload.dossier.asOf, titleSource: payload.dossier.note.titleSource, titleBlockRefs: payload.dossier.note.titleBlockRefs }, null, 2)}</pre></details>
  </div>;
}
