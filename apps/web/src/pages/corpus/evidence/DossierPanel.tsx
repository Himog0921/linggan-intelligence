import { ActionButton } from '../../../shared/ActionButton';
import { useQuery, useQueryClient } from '@tanstack/react-query';
import { useEffect, useRef, useState, type Ref } from 'react';
import { hideCachedReading } from './dossierPresent';
import { fetchDossier } from './dossierApi';
import { CoverageSection, MaterialsSection, MetaSection } from './DossierSections';
import type { DossierMaterial, DossierPayload } from './dossierTypes';
import { FeedbackLine, Section } from './Primitives';
import { DOSSIER_KEY, dossierKey } from './evidenceKeys';
import { downloadDossier, downloadDossierMarkdown } from './exportDossier';
import { useRelayer } from './useRelayer';
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
function Maintenance({ payload, relayer, register }: { payload: DossierPayload; relayer: ReturnType<typeof useRelayer>; register: Ref<HTMLDetailsElement> }) {
  const plan = payload.relayerPlan;
  return <details ref={register} className={styles.disclosure}><summary>整理已有识别版面</summary>
    <p className={styles.note}>{`可整理 ${plan.layoutCount} · 待整理 ${plan.pendingLayouts} · 已是当前规则 ${plan.alreadyCurrent} · 待取得 ${plan.needsAcquisition} · 待识字 ${plan.needsOcr} · 待人工核对 ${plan.pendingReviewCount} 张图片`}</p>
    <ActionButton disabled={plan.layoutCount === 0 || relayer.sending} onClick={relayer.run}>重新整理</ActionButton>
    <FeedbackLine feedback={relayer.feedback} />
    {relayer.sending ? <p role="status">正在整理已有识别结果…</p> : null}
  </details>;
}
/** The human reading and on-demand exports share the same canonical snapshot. */
export function DossierPanel({ work, domain, active, displayedTitle, onOpenImage, onApplied }: Props) {
  const [activated, setActivated] = useState(active); if (active && !activated) setActivated(true);
  const client = useQueryClient();
  const details = useRef<HTMLDetailsElement>(null); const maintenance = useRef<HTMLDetailsElement>(null);
  function openMaintenance() {
    if (details.current) details.current.open = true;
    if (!maintenance.current) return;
    maintenance.current.open = true;
    const summary = maintenance.current.querySelector('summary');
    summary?.focus({ preventScroll: true });
    const inspector = summary?.closest('aside'); const header = inspector?.querySelector(':scope > header');
    if (inspector && header && summary) {
      const inset = parseFloat(getComputedStyle(inspector).getPropertyValue('--lgi-space-3'));
      inspector.scrollTop += summary.getBoundingClientRect().top - header.getBoundingClientRect().bottom - inset;
    } else summary?.scrollIntoView({ block: 'center' });
  }
  const query = useQuery({ queryKey: dossierKey(domain, work), queryFn: ({ signal }) => fetchDossier(work, domain, signal), enabled: activated, staleTime: 30_000, gcTime: 0 });
  const payload = hideCachedReading(query.error) ? null : query.data ?? null;
  const applied = () => { void client.invalidateQueries({ queryKey: [DOSSIER_KEY] }); onApplied(); };
  const relayer = useRelayer({ work, domain, revision: payload?.dossier.revision ?? '', onApplied: applied });
  if (payload === null) return query.isError ? <div className={styles.panel}><p role="status">研究底稿暂时不可读。</p><ActionButton onClick={() => void query.refetch()}>重试读取底稿</ActionButton></div> : activated ? <p className={styles.busy} aria-busy="true">正在读取研究底稿…</p> : null;
  return <div className={styles.panel} aria-busy={query.isFetching}>
    <Head key={`${domain}:${work}`} payload={payload} domain={domain} busy={query.isFetching} onRefresh={() => void query.refetch()} />
    {query.isError ? <p role="status">刷新未完成，当前保留上一次读到的版本。<button type="button" onClick={() => void query.refetch()}>重试刷新</button></p> : null}
    <MaterialsSection displayedTitle={displayedTitle} payload={payload} work={work} domain={domain} onOpenImage={onOpenImage} onApplied={applied} onOpenMaintenance={openMaintenance} />
    <details ref={details} className={styles.advanced}><summary>材料详情</summary><CoverageSection payload={payload} /><Maintenance payload={payload} relayer={relayer} register={maintenance} /><MetaSection payload={payload} domain={domain} /><pre className={styles.pre}>{JSON.stringify({ revision: payload.dossier.revision, materialRevision: payload.dossier.materialRevision, asOf: payload.dossier.asOf, titleSource: payload.dossier.note.titleSource, titleBlockRefs: payload.dossier.note.titleBlockRefs }, null, 2)}</pre></details>
  </div>;
}
