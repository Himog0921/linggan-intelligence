import { ActionButton } from '../../../shared/ActionButton';
import { useQuery, useQueryClient } from '@tanstack/react-query';
import { useRef, useState } from 'react';
import { fetchDossier } from './dossierApi';
import { comparableImages } from './dossierPresent';
import { CoverageSection, FragmentsSection, MetaSection } from './DossierSections';
import type { DossierMedia, DossierPayload } from './dossierTypes';
import { ImageCompare } from './ImageCompare';
import { FeedbackLine, Section } from './Primitives';
import { TaskInput } from './TaskInput';
import { NoteMaterialSection } from './NoteMaterialSection';
import { errorCode } from './evidenceFeedback';
import { DOSSIER_KEY, dossierKey } from './evidenceKeys';
import type { Metric } from './evidenceTypes';
import { downloadDossier } from './exportDossier';
import { useRelayer } from './useRelayer';
import page from './EvidencePage.module.css';
import styles from './Dossier.module.css';

interface DossierPanelProps {
  work: string;
  domain: string;
  /** 标签是否被激活：只有激活后才读取底稿（G10）；之后切走标签状态保留。 */
  active: boolean;
  platformComments: Metric;
  onOpenImage: (media: DossierMedia) => void;
  /** 重新分层成功：列表与详情一并重读。 */
  onApplied: () => void;
}

const READ_FAILED = '暂时无法完成，请重试。已取得的原始材料仍保留。';

function Head({ payload, relayer, busy, onRefresh }: { payload: DossierPayload; relayer: ReturnType<typeof useRelayer>; busy: boolean; onRefresh: () => void }) {
  const { dossier, relayerPlan: plan } = payload;
  const characters = dossier.fragments.reduce((sum, fragment) => sum + fragment.end, 0);
  const unconfirmed = relayer.feedback?.tone === 'unknown' || relayer.feedback?.code === 'dossier_changed_or_scope_unavailable';
  return (
    <Section title="研究底稿">
      <p className={styles.dossierCounts}>{`已整理 ${dossier.fragments.length} 个文字片段 · ${characters} 字符 · ${dossier.coverage.uncertainImages} 张图片待确认`}</p>
      <div className={styles.toolbar}>
        <ActionButton variant="outline" onClick={() => downloadDossier(payload, 'md')}>导出 Markdown</ActionButton>
        <ActionButton variant="outline" onClick={() => downloadDossier(payload, 'json')}>导出 JSON</ActionButton>
        <ActionButton variant="quiet" disabled={busy} onClick={onRefresh}>刷新底稿</ActionButton>
      </div>
      <details className={styles.disclosure}>
        <summary>材料重新分层与高级核对</summary>
        <p className={styles.note}>{`可用布局 ${plan.layoutCount} · 待分层 ${plan.pendingLayouts} · 当前版本 ${plan.alreadyCurrent} · 待识字 ${plan.needsOcr}`}</p>
        <button type="button" disabled={plan.layoutCount === 0 || plan.layoutCount > 100 || relayer.sending}
          onClick={relayer.run}>{`重新分层（${plan.layoutCount} 个版面）`}</button>
        <p className={styles.note}>只整理已有识别版面，不重新识图或发起研究。导出文件保留来源引用。</p>
      </details>
      <FeedbackLine feedback={relayer.feedback}>
        {unconfirmed ? (
          <button type="button" className={page.textButton} disabled={busy} onClick={onRefresh}>
            刷新底稿
          </button>
        ) : null}
      </FeedbackLine>
      {relayer.sending ? <p role="status">正在整理这篇作品已有的识别结果…</p> : null}
    </Section>
  );
}

/**
 * 研究底稿标签（RESEARCH-DOSSIER-001）：完整研究文字、来源与图片分层。标签激活时才读取；读取中 `aria-busy`；
 * 刷新失败保留上一次可读底稿并提示；重新分层是显式的本地写（先对账修订，不重新识图、不启动 AI 研究）。
 */
export function DossierPanel({ work, domain, active, onOpenImage, onApplied }: DossierPanelProps) {
  const [activated, setActivated] = useState(active);
  if (active && !activated) setActivated(true);
  const client = useQueryClient();
  const query = useQuery({ queryKey: dossierKey(domain, work), queryFn: ({ signal }) => fetchDossier(work, domain, signal), enabled: activated, gcTime: 0 });
  const taskSection = useRef<HTMLDivElement>(null);
  const images = useRef(new Map<string, HTMLDetailsElement>());
  const payload = query.data ?? null;
  const relayer = useRelayer({
    work,
    domain,
    revision: payload?.dossier.revision ?? '',
    onApplied: () => {
      void client.invalidateQueries({ queryKey: [DOSSIER_KEY] });
      onApplied();
    },
  });

  function compare(sourceRef: string | null) {
    const target = sourceRef === null ? undefined : images.current.get(sourceRef);
    if (!target) return;
    target.open = true;
    target.scrollIntoView?.({ block: 'start' });
  }

  if (payload === null) {
    if (query.isError) {
      return (
        <div className={styles.panel}>
          <p>{READ_FAILED}</p>
          {errorCode(query.error) ? <code>{errorCode(query.error)}</code> : null}
          <button type="button" onClick={() => void query.refetch()}>
            重试读取底稿
          </button>
        </div>
      );
    }
    return activated ? (
      <p className={styles.busy} aria-busy="true">
        正在整理当前已取得的研究材料…
      </p>
    ) : null;
  }
  const media = comparableImages(payload.dossier.media);
  return (
    <div className={styles.panel} aria-busy={query.isFetching}>
      <Head payload={payload} relayer={relayer} busy={query.isFetching} onRefresh={() => void query.refetch()} />
      {query.isFetching ? <p role="status">正在刷新底稿…</p> : null}
      {query.isError ? (
        <p role="status" className={styles.note}>
          {`${READ_FAILED} 当前保留上一次可读底稿。`}
          <button type="button" onClick={() => void query.refetch()}>
            重试刷新
          </button>
        </p>
      ) : null}
      <FragmentsSection payload={payload} onCompare={compare} />
      <Section title="原图与识别对照">
        {media.map(item => (
          <ImageCompare
            key={item.derivativeRef ?? item.slotKey ?? item.slotOrdinal}
            media={item}
            onOpenImage={onOpenImage}
            register={element => {
              const key = item.derivativeRef;
              if (element && key) images.current.set(key, element);
              return () => {
                if (key) images.current.delete(key);
              };
            }}
          />
        ))}
        {media.length === 0 ? <p>当前没有图片识别结果。</p> : null}
      </Section>
      <details className={styles.advanced}>
        <summary>材料完整度及旧任务输入</summary>
        <CoverageSection payload={payload} />
        <NoteMaterialSection key={`${domain}:${work}`} work={work} domain={domain} />
        <TaskInput payload={payload} work={work} domain={domain} sectionRef={taskSection} />
        <MetaSection payload={payload} />
      </details>
    </div>
  );
}
