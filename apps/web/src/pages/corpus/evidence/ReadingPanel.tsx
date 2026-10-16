import { useState } from 'react';
import { DiscussionBlock } from './DiscussionBlock';
import { InlineState, Section, SourceNote, StateLine } from './Primitives';
import { ReobservationSection } from './ReobservationSection';
import { TechnicalFacts } from './TechnicalFacts';
import { MediaRail } from './MediaRail';
import { AuthorFact, Engagement, MaterialLegend, MaterialRail } from './WorkParts';
import { laneSummary, materialSummary, publishedCopy, workTitle } from './evidencePresent';
import { mediaObjects } from './mediaObjects';
import type { Channels, Inspector, WorkItem } from './evidenceTypes';
import type { CommentChannel } from './useCommentChannel';
import type { Reobservation } from './useReobservation';
import styles from './Inspector.module.css';

export interface PanelContext {
  item: WorkItem;
  inspector: Inspector;
  channels: Channels;
  /** 列表行里的同一个作品（带检索命中的原声）；按 `work` 直读时没有。 */
  listItem: WorkItem | null;
  comments: CommentChannel;
  onOpenViewer: (index: number, opener: HTMLElement) => void;
}

/** 长文折叠：正文在这里完整给出，只是默认不占满整屏。 */
const COLLAPSE_CHARACTERS = 360;
const COLLAPSE_LINES = 8;

function Body({ text }: { text: string }) {
  const [expanded, setExpanded] = useState(false);
  const long = Array.from(text).length > COLLAPSE_CHARACTERS || text.split('\n').length > COLLAPSE_LINES;
  return (
    <div>
      <p className={styles.bodyBlock} data-collapsed={long && !expanded}>
        {text}
      </p>
      {long ? (
        <button type="button" aria-expanded={expanded} onClick={() => setExpanded(value => !value)}>
          {expanded ? '收起' : '展开全文'}
        </button>
      ) : null}
    </div>
  );
}

/**
 * 概览（阅读优先，F08）：标题、作者、发布时间、互动 → 正文 → 图组 → 讨论 → 材料完整度 → 立即复观测；
 * 字段码、UUID、时间来源与精度、通道状态降到最底下的「技术事实」。不新增数据、不新增接口、一项事实都不删。
 * 材料缺口写在对应的块上（正文、图片、图片文字、转录、评论各用同一套状态词），不集中放页尾。
 */
export function ReadingPanel({ context, domain, reobservation, onRecheck }: { context: PanelContext; domain: string; reobservation: Reobservation; onRecheck: () => void }) {
  const { item, inspector, channels, listItem, comments, onOpenViewer } = context;
  const objects = mediaObjects(inspector, (listItem ?? item).fragment);
  const summary = materialSummary(item);
  const body = inspector.detail.body;
  const ocr = laneSummary(item.lanes, 'ocr')?.state;
  const asr = laneSummary(item.lanes, 'asr')?.state;
  return (
    <div>
      <header className={styles.reading}>
        <h3 className={styles.readingTitle}>{workTitle(item)}</h3>
        <div className={styles.readingTags}>
          <span>{item.platform.toUpperCase()}</span>
          {item.display.titleSource === 'cover_ocr' ? <span className={styles.tag}>封面 OCR</span> : null}
        </div>
        <div className={styles.readingMeta}>
          <AuthorFact item={item} />
          <span>{publishedCopy(item.display, { withFieldName: false })}</span>
        </div>
        <Engagement engagement={item.display.engagement} />
      </header>

      <Section title="正文">
        {body?.state === 'KNOWN' && body.value ? (
          <Body text={body.value} />
        ) : (
          <>
            <SourceNote>正文通道尚未取得</SourceNote>
            <StateLine state={laneSummary(item.lanes, 'detail')?.state} />
          </>
        )}
      </Section>

      <Section title="图组">
        {objects.length > 0 ? (
          <MediaRail objects={objects} onOpen={onOpenViewer} />
        ) : (
          <InlineState tone="warning" title="当前没有本地媒体对象">
            这只说明本机还没有留存这个作品的媒体副本；这不表示作品没有媒体。
          </InlineState>
        )}
        <div className={styles.gaps}>
          <span>图片文字</span>
          <StateLine state={ocr} />
          {item.media.videoCount > 0 ? (
            <>
              <span>转录</span>
              <StateLine state={asr} />
            </>
          ) : null}
        </div>
      </Section>

      <DiscussionBlock item={item} inspector={inspector} channelUrl={channels.commentsUrl} channel={comments} />

      <Section title="材料完整度">
        <MaterialRail summary={summary} />
        <MaterialLegend summary={summary} />
      </Section>

      <ReobservationSection item={item} channel={channels.reobservation} domain={domain} state={reobservation} onRecheck={onRecheck} />

      <TechnicalFacts item={item} inspector={inspector} />
    </div>
  );
}
