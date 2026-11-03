import { endingBlockAttribution, endingGroupAttribution, groupTagLabels, groupText, groupTime, formatReadingTime, formatReadingTimeRange, readingAnchor, showReadingAttribution, type ReadingIssue, type AttributionBoundary, type ResolvedReadingGroup } from './dossierDocumentModel';
import { confirmedEmptySpeech } from './dossierPresent';
import { hasReadingSourceText } from './dossierReadingContent';
import { normalizedTagLabel } from './dossierTagText';
import type { DossierBlock, DossierMaterial, Speaker } from './dossierTypes';
import styles from './DossierDocument.module.css';

const ROLE_LABELS: Record<Speaker, string> = { author: '作品作者表述', other: '他人表述', unconfirmed: '表达归属未确认' };
export function ReadingText({ text }: { text: string }) {
  const parts = text.split(/((?:周[一二三四五六日天]\s*)?\d{1,2}:\d{2}|\d{1,6}(?:\.\d{1,2})?\s*(?:个工作日|分钟|小时|秒|天|字|条|页|项|次|人|%))/gu);
  return <>{parts.map((part, index) => index % 2 ? <span className={styles.unit} key={index}>{part}</span> : part)}</>;
}
export function ReadingIssues({ issues }: { issues: readonly ReadingIssue[] }) {
  const specific = [...new Map(issues.filter(issue => issue.code !== 'cloud_image_unchecked').map(issue => [issue.message, issue])).values()];
  return specific.length ? <details className={styles.reviewNotes}><summary>核对说明{specific.length > 1 ? ` · ${specific.length}` : ''}</summary><ul className={styles.issues}>{specific.map(issue => <li key={issue.displayKey}>{issue.message}</li>)}</ul></details> : null;
}
export function EmptySpeechNotes({ materials }: { materials: readonly DossierMaterial[] }) {
  return <>{materials.filter(unit => unit.kind === 'audio').map((unit, index, units) => {
    if (unit.blocks.some(hasReadingSourceText)) return null;
    const note = confirmedEmptySpeech(unit) ? '已确认无口播。'
      : unit.processing?.state === 'completed' && unit.processing.reason === 'no_audio_stream_observed' ? '未发现音轨。'
      : unit.processing?.state === 'completed' && unit.processing.reason === 'no_speech_observed' ? '未检出语音。'
      : unit.sourceState === 'known_empty' && unit.currentRevisionRef ? '未取得可用口播。' : null;
    return note ? <p className={styles.end} key={unit.ref}>{units.length > 1 ? `音轨 ${index + 1}` : '视频口播'}：{note}</p> : null;
  })}</>;
}
function Tags({ labels }: { labels: readonly string[] }) { return <div className={styles.tags}>{labels.map((label, index) => <span className={styles.tag} key={index}>{label}</span>)}</div>; }
function shortDefinitions(block: Extract<DossierBlock, { kind: 'table' }>): boolean {
  return block.columns.length === 2 && block.columns[0] === '术语' && block.columns[1] === '解释' && block.rows.length === 3
    && block.rows.every(row => row.length === 2 && row[0]?.trim() && Array.from(row[0]).length <= 16 && !/[\r\n]/u.test(row[0]) && row[1]?.trim());
}
function ContentBlock({ block, tags }: { block: DossierBlock; tags?: boolean }) {
  if (tags && block.kind === 'list') return <Tags labels={block.items.map(normalizedTagLabel)}/>;
  switch (block.kind) {
    case 'heading': {
      const Heading = `h${Math.min(6, (block.level ?? 1) + 2)}` as 'h3' | 'h4' | 'h5' | 'h6';
      return <Heading className={styles.heading} data-source-level={block.level}><ReadingText text={block.text}/></Heading>;
    }
    case 'paragraph': return <p className={styles.paragraph}><ReadingText text={block.text}/></p>;
    case 'quote': return <blockquote className={styles.quote}><ReadingText text={block.text}/></blockquote>;
    case 'gap': return null;
    case 'list': return block.ordered
      ? <ol className={styles.list} start={block.start}>{block.items.map((item, index) => <li key={index}><ReadingText text={item}/></li>)}</ol>
      : <ul className={styles.list}>{block.items.map((item, index) => <li key={index}><ReadingText text={item}/></li>)}</ul>;
    case 'table': return shortDefinitions(block)
      ? <dl className={styles.definitions} aria-label={block.columns.join('与')}>{block.rows.map((row, index) => <div className={styles.definition} key={index}><dt><ReadingText text={row[0]!}/></dt><dd><ReadingText text={row[1]!}/></dd></div>)}</dl>
      : <div className={styles.tableWrap} role="region" aria-label="正文表格" tabIndex={0}><table className={styles.table}><thead><tr>{block.columns.map((column, index) => <th scope="col" key={index}>{column}</th>)}</tr></thead><tbody>{block.rows.map((row, index) => <tr key={index}>{row.map((cell, column) => <td key={column}><ReadingText text={cell}/></td>)}</tr>)}</tbody></table></div>;
    case 'dialogue': return <dl className={styles.dialogue}>{block.turns.map((turn, index) => <div key={index}><dt>{ROLE_LABELS[turn.speaker]}</dt><dd><ReadingText text={turn.text}/></dd></div>)}</dl>;
  }
}
function GroupContent({ value, previousRole }: { value: ResolvedReadingGroup; previousRole: AttributionBoundary }) {
  const { group, entries } = value;
  if (group.mode === 'heading') return <h3 className={styles.heading}><ReadingText text={groupText(value)}/></h3>;
  if (group.mode === 'paragraph' || group.mode === 'transcript_paragraph') return <p className={styles.paragraph}><ReadingText text={groupText(value)}/></p>;
  if (group.mode === 'list') {
    const first = entries[0]?.block;
    if (first?.kind !== 'list') return null;
    const items = entries.flatMap(entry => entry.block.kind === 'list' ? entry.block.items : []);
    return <ContentBlock block={{ ...first, items }}/>;
  }
  if (group.mode === 'table') {
    const first = entries[0]?.block;
    if (first?.kind !== 'table') return null;
    return <ContentBlock block={{ ...first, rows: entries.flatMap(entry => entry.block.kind === 'table' ? entry.block.rows : []) }}/>;
  }
  if (group.mode === 'tags') {
    return <Tags labels={groupTagLabels(entries)}/>;
  }
  return <>{entries.map(({ block, unit }, index) => <div key={block.ref} data-block-ref={block.ref} data-speaker={block.speaker}>
    {!block.contentType && block.kind !== 'dialogue' && showReadingAttribution(block.speaker ?? 'unconfirmed', index ? endingBlockAttribution(entries[index - 1]?.block) : previousRole) ? <p className={styles.attribution}>{ROLE_LABELS[block.speaker ?? 'unconfirmed']}</p> : null}
    {block.time ? <span className={styles.transcriptTime} data-time-start-ms={block.time.startMs} data-time-end-ms={block.time.endMs}>{formatReadingTimeRange(block.time)}</span> : null}
    {block.contentType ? <p className={styles.attribution}><span>{block.contentType === 'visual_description' ? '视觉描述 · 模型观察，非原文' : '图片文字 · 识别提取'}</span>{block.regionLabel && block.regionLabel !== '整图' ? <span> · {block.regionLabel}</span> : null}</p> : null}
    <ContentBlock block={block} tags={unit.kind === 'tags'}/>
  </div>)}</>;
}
function Continuation({ from }: { from: boolean }) { return <p className={styles.continuation}>{from ? '承接上一段内容' : '后续内容不在本次范围内'}</p>; }
export function DossierGroup({ value, prefix, previousRole, hideIssues = false }: { value: ResolvedReadingGroup; prefix: string; previousRole?: AttributionBoundary; hideIssues?: boolean }) {
  const id = readingAnchor(prefix, value.group.ref);
  const first = value.entries[0]?.block;
  const role = first?.speaker ?? 'unconfirmed';
  const time = value.group.mode !== 'blocks' ? groupTime(value) : null;
  return <div className={styles.group} id={id} data-reading-anchor={value.group.ref} tabIndex={-1}>
    {value.group.continuation.fromPrevious ? <Continuation from/> : null}
    {value.group.mode !== 'blocks' && !first?.contentType && showReadingAttribution(role, previousRole) ? <p className={styles.attribution}>{ROLE_LABELS[role]}</p> : null}
    {value.group.mode !== 'blocks' && first?.contentType ? <p className={styles.attribution}><span>{first.contentType === 'visual_description' ? '视觉描述 · 模型观察，非原文' : '图片文字 · 识别提取'}</span>{first.regionLabel && first.regionLabel !== '整图' ? <span> · {first.regionLabel}</span> : null}</p> : null}
    {time ? <span className={styles.transcriptTime} data-time-start-ms={time.startMs} data-time-end-ms={time.endMs}>{formatReadingTimeRange(time)}</span> : null}
    <GroupContent value={value} previousRole={previousRole}/>
    {!hideIssues ? <ReadingIssues issues={value.issues}/> : null}
    {value.group.continuation.toNext ? <Continuation from={false}/> : null}
  </div>;
}
export function TranscriptParagraph({ value, prefix, showRole }: { value: ResolvedReadingGroup; prefix: string; showRole: boolean }) {
  const time = groupTime(value), role = value.entries[0]?.block.speaker ?? 'unconfirmed';
  return <div className={styles.transcriptRow} id={readingAnchor(prefix, value.group.ref)} data-reading-anchor={value.group.ref} data-transcript-paragraph tabIndex={-1}>
    <span className={styles.transcriptTime} title={time ? formatReadingTimeRange(time) : '时间未取得'} data-time-start-ms={time?.startMs} data-time-end-ms={time?.endMs}>{time ? formatReadingTime(time.startMs) : '—'}</span>
    <div>
      {showRole ? <span className={styles.attribution}>{ROLE_LABELS[role]}</span> : null}
      {value.group.continuation.fromPrevious ? <Continuation from/> : null}
      <p className={styles.transcriptText} data-speaker={role}><ReadingText text={groupText(value)}/></p>
      <ReadingIssues issues={value.issues}/>
      {value.group.continuation.toNext ? <Continuation from={false}/> : null}
    </div>
  </div>;
}
export function TranscriptGroups({ groups, prefix }: { groups: readonly ResolvedReadingGroup[]; prefix: string }) {
  return <>{groups.map((value, index) => {
    const role = value.entries[0]?.block.speaker ?? 'unconfirmed';
    const previousRole = endingGroupAttribution(groups[index - 1]);
    const showRole = showReadingAttribution(role, previousRole);
    return value.group.mode === 'transcript_paragraph'
      ? <TranscriptParagraph key={value.group.ref} value={value} prefix={prefix} showRole={showRole}/>
      : <DossierGroup key={value.group.ref} value={value} prefix={prefix} previousRole={previousRole}/>;
  })}</>;
}
