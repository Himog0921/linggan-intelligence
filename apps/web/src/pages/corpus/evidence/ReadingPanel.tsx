import { useState } from 'react';
import { DiscussionBlock } from './DiscussionBlock';
import { InlineState, Section, SourceNote, StateLine } from './Primitives';
import { MediaRail } from './MediaRail';
import { laneSummary } from './evidencePresent';
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
const COLLAPSE_CHARACTERS = 360;
const COLLAPSE_LINES = 8;
function Body({ text }: { text: string }) {
  const [expanded, setExpanded] = useState(false);
  const long = Array.from(text).length > COLLAPSE_CHARACTERS || text.split('\n').length > COLLAPSE_LINES;
  return <div>
    <p className={styles.bodyBlock} data-collapsed={long && !expanded}>{text}</p>
    {long ? <button type="button" className={styles.moreText} aria-expanded={expanded}
      onClick={() => setExpanded(value => !value)}>{expanded ? '收起正文' : '展开全文'}</button> : null}
  </div>;
}
/** Work page reads source content only. The shared inspector header owns metadata. */
export function ReadingPanel({ context }: { context: PanelContext }) {
  const { item, inspector, channels, listItem, comments, onOpenViewer } = context;
  const objects = mediaObjects(inspector, (listItem ?? item).fragment);
  const body = inspector.detail.body;
  const ocr = laneSummary(item.lanes, 'ocr')?.state;
  const asr = laneSummary(item.lanes, 'asr')?.state;
  return <div className={styles.readingBody}>
    <Section title="正文">
      {body?.state === 'KNOWN' && body.value
        ? <Body text={body.value} />
        : <><SourceNote>正文通道尚未取得</SourceNote><StateLine state={laneSummary(item.lanes, 'detail')?.state} /></>}
    </Section>
    <Section title="图组">
      {objects.length > 0 ? <MediaRail objects={objects} onOpen={onOpenViewer} />
        : <InlineState tone="warning" title="当前没有可预览的本地媒体对象">没有可内联显示的媒体副本，不代表原作品没有媒体。</InlineState>}
      <div className={styles.gaps}>
        <span>图片文字</span><StateLine state={ocr} />
        {item.media.videoCount > 0 ? <><span>转录</span><StateLine state={asr} /></> : null}
      </div>
    </Section>
    <DiscussionBlock item={item} inspector={inspector} channelUrl={channels.commentsUrl} channel={comments} />
  </div>;
}
