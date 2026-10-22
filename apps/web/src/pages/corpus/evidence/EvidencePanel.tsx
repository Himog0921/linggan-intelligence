import { DiscussionBlock } from './DiscussionBlock';
import { FactGrid, InlineState, Section, SourceNote, StateLine, Tech } from './Primitives';
import type { PanelContext } from './ReadingPanel';
import { QuoteBlock } from './WorkParts';
import {
  fragmentSourceLabel,
  momentText,
  receiptRows,
  stateMeta,
  stoppedReasonMeaning,
  type FactRowData,
} from './evidencePresent';
import type { CoverageEntry, Derivative, Inspector } from './evidenceTypes';
import styles from './Inspector.module.css';

function Layering({ derivative }: { derivative: Derivative }) {
  const layering = derivative.layering;
  if (layering === null) return null;
  return (
    <details className={styles.technical}>
      <summary>分层详情</summary>
      <FactGrid
        rows={[
          ['OCR 版面记录', layering.layoutRef ?? '当前未表达', layering.layoutRef ? null : 'UNKNOWN'],
          ['分层状态', stateMeta(layering.state)[0], layering.state],
          ['判定来源', layering.decisionSource === 'rules' ? '本地规则' : layering.decisionSource === 'vision' ? '视觉选择器' : '当前未表达', layering.decisionSource ?? 'UNKNOWN'],
          ['封面主文案', layering.coverHeadline ?? '当前未形成', layering.coverHeadline ? null : 'UNKNOWN'],
          ['干净语料资格', layering.cleanCorpusEligible ? '可进入图片实质语料' : '不可作为干净语料', layering.cleanCorpusEligible ? 'ACCEPTED' : layering.state],
        ]}
      />
      {layering.imageSubstantiveText?.trim() ? <SourceNote>{`已接纳的图片实质文字：${layering.imageSubstantiveText}`}</SourceNote> : null}
    </details>
  );
}

/**
 * 机器识别的图片文字与转录原文，原样列出，不被任何生成的摘要替换。封面是设计排版，识别结果常为噪声，
 * 所以封面图的识别文字照样显示，但不再当作可引用的原声。
 */
function DerivedTexts({ inspector }: { inspector: Inspector }) {
  const texts = inspector.derivatives
    .filter(derivative => derivative.displayText?.trim())
    .sort((left, right) => (left.slotKey ?? '').localeCompare(right.slotKey ?? ''));
  const coverSlots = new Set(inspector.mediaSlots.filter(slot => slot.purpose === 'cover' && slot.slotKey).map(slot => slot.slotKey));
  return (
    <Section title="图片文字与转录原文">
      {texts.length === 0 ? (
        <SourceNote>当前详情没有已识别的图片文字或转录文本；不把缺少记录写成处理成功或失败。</SourceNote>
      ) : (
        <>
          <SourceNote>以下为所选图片文字与转录版本，机器识别可能有出入；人工修正来源在分层详情保留。</SourceNote>
          {texts.map(derivative => {
            const cover = derivative.slotKey !== null && coverSlots.has(derivative.slotKey);
            return (
              <article key={`${derivative.slotKey}-${derivative.kind}`} className={styles.derivative}>
                <div className={styles.derivativeHead}>
                  <strong>{fragmentSourceLabel(derivative.kind)}</strong>
                  {cover ? <span className={styles.inlineNote}>封面图 · 不宜直接引用</span> : null}
                  {derivative.slotKey ? <Tech>{derivative.slotKey}</Tech> : null}
                </div>
                <div className={styles.derivativeText}>{derivative.displayText}</div>
                <StateLine state={derivative.state} />
                <Layering derivative={derivative} />
                {cover ? <SourceNote>封面是设计排版，机器识别结果常为噪声；列表引用的原声不采用它。</SourceNote> : null}
              </article>
            );
          })}
        </>
      )}
    </Section>
  );
}

function historyRows(entry: CoverageEntry): FactRowData[] {
  return [
    ['范围', entry.collectionScope ?? '当前未知'],
    ['请求上限', entry.requestedLimit ?? '当前未知'],
    ['页面评论数', entry.pageCommentCount ?? '当前未知'],
    ['预期 / 去重保留', `${entry.expectedCount ?? '当前未知'} / ${entry.uniqueCollectedCount ?? '当前未知'}`, entry.collectionState ?? 'UNKNOWN'],
    ['采集 / 保留 / 失败', `${entry.producerAcquired ?? '当前未知'} / ${entry.retained ?? '当前未知'} / ${entry.failed ?? '当前未知'}`],
  ];
}

function HistoryEntry({ entry }: { entry: CoverageEntry }) {
  return (
    <article className={styles.derivative}>
      <div className={styles.derivativeHead}>
        <strong>{momentText(entry.observedAt, '观察时间当前未知')}</strong>
        <StateLine state={entry.state} />
      </div>
      <FactGrid rows={historyRows(entry)} />
      <details className={styles.technical}>
        <summary>来源引用</summary>
        <FactGrid
          rows={[
            ['Task / Attempt', `${entry.taskRef ?? '当前未知'} / ${entry.attemptRef ?? '当前未知'}`],
            ['Package / Receipt', `${entry.packageRef ?? '当前未知'} / ${entry.receiptRef ?? '未见回执'}`],
          ]}
        />
      </details>
    </article>
  );
}

/** 评论／回复复观测历史：每条是一个独立的 Package／回执，不跨尝试合计；按范围分三组。 */
function History({ label, entries }: { label: string; entries: readonly CoverageEntry[] }) {
  if (entries.length === 0) return <SourceNote>{`当前没有${label}覆盖历史；这不表示该通道未曾执行。`}</SourceNote>;
  const groups: readonly (readonly [string, CoverageEntry[]])[] = [
    ['详情窗口（最多 30 条）', entries.filter(entry => entry.collectionScope === 'detail_window')],
    ['独立全量或其他范围', entries.filter(entry => entry.collectionScope !== null && entry.collectionScope !== 'detail_window')],
    ['范围未完整表达', entries.filter(entry => entry.collectionScope === null)],
  ];
  return (
    <div>
      <p className={styles.inlineNote}>{`${label}：每条为一个独立的采集包与回执，不做跨尝试合计`}</p>
      {groups.map(([heading, group]) =>
        group.length === 0 ? null : (
          <div key={heading} className={styles.historyGroup}>
            <strong>{heading}</strong>
            {group.map((entry, index) => (
              <HistoryEntry key={`${entry.packageRef}-${index}`} entry={entry} />
            ))}
          </div>
        ),
      )}
    </div>
  );
}

/** 证据：列表引用的原声、机器识别文字、评论研究；覆盖历史与通道回执折叠在「技术事实」里。 */
export function EvidencePanel({ context }: { context: PanelContext }) {
  const { item, inspector, channels, listItem, comments } = context;
  const fragment = (listItem ?? item).fragment;
  return (
    <div>
      <Section title="列表引用的原声">
        {fragment === null ? (
          <SourceNote>这个作品还没有可引用的原文、评论或图片文字。这只描述当前已接纳的材料，不表示平台上没有内容。</SourceNote>
        ) : (
          <>
            <QuoteBlock fragment={fragment} wrap />
            <FactGrid
              rows={[
                ['来源类型', fragmentSourceLabel(fragment.sourceKind)],
                ['选取依据', fragment.selectionBasis === 'SEARCH_MATCH' ? '包含当前检索词' : '当前最具作者性的可读材料'],
                ['是否截断', fragment.truncated ? '是，已按展示边界截断' : '否'],
              ]}
            />
            <details className={styles.technical}>
              <summary>来源引用</summary>
              <FactGrid rows={[['来源引用', fragment.sourceRef ?? '当前未表达', fragment.sourceRef ? null : 'UNKNOWN'], ...(fragment.slotKey ? ([['所在媒体对象', fragment.slotKey]] as FactRowData[]) : [])]} />
            </details>
          </>
        )}
      </Section>
      <DerivedTexts inspector={inspector} />
      <DiscussionBlock item={item} inspector={inspector} channelUrl={channels.commentsUrl} channel={comments} />
      <details className={styles.technical}>
        <summary>技术事实</summary>
        <Section title="评论 / 回复复观测历史">
          <History label="评论" entries={inspector.commentsHistory} />
          <History label="回复" entries={inspector.repliesHistory} />
        </Section>
        <Section title="评论研究通道">
          <FactGrid rows={receiptRows(channels.commentsReceipt ?? inspector.commentsReceipt)} />
          {(channels.commentsReceipt ?? inspector.commentsReceipt)?.truncated && channels.commentsUrl === null ? (
            <InlineState tone="warning" title="继续读取入口尚未提供">
              回执表明通道已截断，但详情没有给出可调用的下一页地址；页面不会猜测后端路由。
            </InlineState>
          ) : null}
          {inspector.commentsCoverage?.stoppedReason ? (
            <SourceNote>{`评论停止原因：${stoppedReasonMeaning(inspector.commentsCoverage.stoppedReason)}`}</SourceNote>
          ) : null}
        </Section>
      </details>
    </div>
  );
}
