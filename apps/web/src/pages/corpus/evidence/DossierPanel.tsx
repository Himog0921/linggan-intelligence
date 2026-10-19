import { useQuery, useQueryClient } from '@tanstack/react-query';
import { useRef, useState } from 'react';
import { fetchDossier } from './dossierApi';
import { DossierComments } from './DossierComments';
import { comparableImages } from './dossierPresent';
import { CoverageSection, FragmentsSection, MetaSection } from './DossierSections';
import type { DossierMedia, DossierPayload } from './dossierTypes';
import { ImageCompare } from './ImageCompare';
import { FeedbackLine, Section } from './Primitives';
import { TaskInput } from './TaskInput';
import { errorCode } from './evidenceFeedback';
import { DOSSIER_KEY, dossierKey } from './evidenceKeys';
import { momentText } from './evidencePresent';
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

function Head({ payload, relayer, busy, onRefresh, onPreview }: { payload: DossierPayload; relayer: ReturnType<typeof useRelayer>; busy: boolean; onRefresh: () => void; onPreview: () => void }) {
  const { dossier, relayerPlan: plan } = payload;
  const characters = dossier.fragments.reduce((sum, fragment) => sum + fragment.end, 0);
  const author = dossier.metadata.authorDisplayName ?? '作者未知';
  const unconfirmed = relayer.feedback?.tone === 'unknown' || relayer.feedback?.code === 'dossier_changed_or_scope_unavailable';
  return (
    <Section title="研究底稿">
      <p className={styles.note}>导出保留平台作品和作者编号，用于来源追溯；分享文件前请检查其中的来源信息。</p>
      <p className={styles.note}>{`${dossier.metadata.platform} · ${author} · 发布时间 ${momentText(dossier.metadata.publishedAt, '未知')}`}</p>
      <p>{`已整理 ${dossier.fragments.length} 个文字片段 · ${characters} 字符 · ${dossier.coverage.uncertainImages} 张图片部分待确认`}</p>
      <p className={styles.note}>底稿使用当前可读取的共享材料。历史运行继续保留其冻结输入；待确认行不进入正文。</p>
      <p className={styles.note}>{`本篇可用布局 ${plan.layoutCount} 个 · 待分层 ${plan.pendingLayouts} 个 · 已是当前版本 ${plan.alreadyCurrent} 个 · 需先识字 ${plan.needsOcr} 个`}</p>
      <div className={styles.toolbar}>
        <button type="button" className={page.primary} onClick={onPreview}>
          预览 AI 输入
        </button>
        <button type="button" disabled={busy} onClick={onRefresh}>
          刷新底稿
        </button>
        <button type="button" onClick={() => downloadDossier(payload, 'md')}>
          导出 Markdown
        </button>
        <button type="button" onClick={() => downloadDossier(payload, 'json')}>
          导出 JSON
        </button>
        <button type="button" disabled={plan.layoutCount === 0 || plan.layoutCount > 100 || relayer.sending} onClick={relayer.run}>
          {`重新分层（${plan.layoutCount} 个版面）`}
        </button>
      </div>
      <p className={styles.note}>重新分层处理已有版面；封面与首图可能是同一图片，送给 AI 时会去重。不重新识图，也不启动 AI 研究。</p>
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
export function DossierPanel({ work, domain, active, platformComments, onOpenImage, onApplied }: DossierPanelProps) {
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
      <Head payload={payload} relayer={relayer} busy={query.isFetching} onRefresh={() => void query.refetch()} onPreview={() => taskSection.current?.scrollIntoView?.({ block: 'start' })} />
      {query.isFetching ? <p role="status">正在刷新底稿…</p> : null}
      {query.isError ? (
        <p role="status" className={styles.note}>
          {`${READ_FAILED} 当前保留上一次可读底稿。`}
          <button type="button" onClick={() => void query.refetch()}>
            重试刷新
          </button>
        </p>
      ) : null}
      <CoverageSection payload={payload} />
      <TaskInput payload={payload} work={work} domain={domain} sectionRef={taskSection} />
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
      <DossierComments work={work} domain={domain} platformComments={platformComments} />
      <MetaSection payload={payload} />
    </div>
  );
}
