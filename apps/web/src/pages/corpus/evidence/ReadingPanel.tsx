import { useQuery } from '@tanstack/react-query';
import { fetchDossier } from './dossierApi';
import { dossierKey } from './evidenceKeys';
import { DossierBlocks } from './DossierBlocks';
import { emptyMaterialNote, hideCachedReading } from './dossierPresent';
import { DossierComments } from './DossierComments';
import { InlineState, Section, SourceNote, StateLine } from './Primitives';
import { MediaRail } from './MediaRail';
import { OcrRecognition } from './OcrRecognition';
import { laneSummary, metricReading } from './evidencePresent';
import { mediaObjects } from './mediaObjects';
import type { Channels, Inspector, WorkItem } from './evidenceTypes';
import type { CommentChannel } from './useCommentChannel';
import styles from './Inspector.module.css';

export interface PanelContext {
  item: WorkItem;
  inspector: Inspector;
  channels: Channels;
  listItem: WorkItem | null;
  comments: CommentChannel;
  onOpenViewer: (index: number, opener: HTMLElement) => void;
}
/** Work page reads source content only. The shared inspector header owns metadata. */
export function ReadingPanel({ context, domain, active }: { context: PanelContext; domain: string; active: boolean }) {
  const { item, inspector, listItem, onOpenViewer } = context;
  const objects = mediaObjects(inspector, (listItem ?? item).fragment);
  const reading = useQuery({ queryKey: dossierKey(domain, item.ref), queryFn: ({ signal }) => fetchDossier(item.ref, domain, signal), enabled: active, staleTime: 30_000, gcTime: 0 });
  const body = hideCachedReading(reading.error) ? undefined : reading.data?.dossier.materials.find(unit => unit.kind === 'body');
  const ocr = laneSummary(item.lanes, 'ocr')?.state;
  const asr = laneSummary(item.lanes, 'asr')?.state;
  return <div className={styles.readingBody}>
    <Section title="正文">
      {reading.isPending ? <p role="status">正在读取正文…</p> : reading.isError && !body ? <p role="status">正文读本暂不可读。<button type="button" onClick={() => void reading.refetch()}>重试读取</button></p> : body ? <>{body.blocks.length ? <DossierBlocks blocks={body.blocks} issues={body.issues} /> : <SourceNote>{emptyMaterialNote(body)}</SourceNote>}</> : <SourceNote>正文来源尚未取得。</SourceNote>}
      {reading.isError && body ? <p role="status">刷新未完成，保留上次读到的正文版本。</p> : null}
    </Section>
    <OcrRecognition key={`${domain}:${item.ref}`} workRef={item.ref} domain={domain} onOpenSource={(image, opener) => {
      const index = objects.findIndex(object => object.slotKey !== null && (object.slotKey === image.slotKey || image.aliases.includes(object.slotKey)));
      if (index >= 0) onOpenViewer(index, opener);
      else if (image.originalUrl) window.open(image.originalUrl, '_blank', 'noopener,noreferrer');
    }}>
      {objects.length > 0 ? <MediaRail objects={objects} onOpen={onOpenViewer} />
        : <InlineState tone="warning" title="当前没有可预览的本地媒体对象">没有可内联显示的媒体副本，不代表原作品没有媒体。</InlineState>}
      <div className={styles.gaps}>
        <span>图片文字</span><StateLine state={ocr} />
        {item.media.videoCount > 0 ? <><span>转录</span><StateLine state={asr} /></> : null}
      </div>
    </OcrRecognition>
    <Section title="评论与回复">
      <div className={styles.commentSummary}>
        <span>平台显示 <strong>{metricReading(item.display.engagement.comment)}</strong> 条</span>
        <span>本地评论 <strong>{inspector.commentsCoverage?.retained ?? '未知'}</strong> 条</span>
        <span>已采回复 <strong>{inspector.repliesCoverage?.retained ?? '未知'}</strong> 条</span>
      </div>
      <DossierComments work={item.ref} domain={domain} platformComments={item.display.engagement.comment} />
    </Section>
  </div>;
}
