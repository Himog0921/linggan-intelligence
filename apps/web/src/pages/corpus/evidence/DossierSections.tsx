import { coverageLabel, sectionEmptyNote } from './dossierPresent';
import type { DossierFragment, DossierPayload } from './dossierTypes';
import { Section } from './Primitives';
import { metricReading } from './evidencePresent';
import styles from './Dossier.module.css';

/** 完整度与下一步：原标题／正文各自的状态，媒体处理结果 x/y 可读取，缺口的补救指引。 */
export function CoverageSection({ payload }: { payload: DossierPayload }) {
  const { coverage } = payload.dossier;
  return (
    <Section title="完整度与下一步">
      <p>{`原标题：${coverageLabel(coverage.title)}；正文：${coverageLabel(coverage.body)}；媒体处理结果：${coverage.mediaAvailable}/${coverage.mediaCount} 可读取。`}</p>
      {coverage.titleQualification === 'suspected_count' ? (
        <p className={styles.note}>发现记录中的数字与同次点赞数相同，且未取得详情标题，已排除出平台标题；列表可使用封面大字作为展示标题。</p>
      ) : null}
      <p className={styles.note}>缺少原文或原图：先在“材料”查看获取状态，再通过现有采集任务补齐。识别已取得但分层不理想：使用上方“重新分层”。这份底稿只代表已经采到的材料。</p>
    </Section>
  );
}

function FragmentBlock({ fragment, onCompare }: { fragment: DossierFragment; onCompare: (sourceRef: string | null) => void }) {
  return (
    <>
      <details className={styles.disclosure} open={fragment.end < 500}>
        <summary>{`展开全文 · ${fragment.end} 字符`}</summary>
        <p className={styles.text}>{fragment.text}</p>
      </details>
      {fragment.field === 'cover_headline' || fragment.field === 'ocr' ? (
        <button type="button" onClick={() => onCompare(fragment.sourceRef)}>
          对照原图
        </button>
      ) : null}
      <details className={styles.disclosure}>
        <summary>查看来源</summary>
        <pre className={styles.pre}>{JSON.stringify({ fragmentId: fragment.fragmentId, sourceRef: fragment.sourceRef, sourceVersion: fragment.sourceVersion }, null, 2)}</pre>
      </details>
    </>
  );
}

/** 完整文字：按栏目分块，空栏各写自己的说明；图片类片段可「对照原图」。 */
export function FragmentsSection({ payload, onCompare }: { payload: DossierPayload; onCompare: (sourceRef: string | null) => void }) {
  const { fragments, sections } = payload.dossier;
  return (
    <Section title="完整文字">
      {fragments.length === 0 ? <p>暂未取得可用文字；这不等于原作品没有内容。</p> : null}
      {sections.map(part => {
        const own = fragments.filter(fragment => part.fragmentIds.includes(fragment.fragmentId));
        return (
          <Section key={`${part.kind}-${part.label}`} title={part.label}>
            {own.length === 0 ? <p className={styles.note}>{sectionEmptyNote(part.kind)}</p> : null}
            {own.map(fragment => (
              <FragmentBlock key={fragment.fragmentId} fragment={fragment} onCompare={onCompare} />
            ))}
          </Section>
        );
      })}
    </Section>
  );
}

/** 互动与来源信息：点赞／评论／收藏（未知写「未知」）＋「底稿版本与观察信息」折叠诊断。 */
export function MetaSection({ payload }: { payload: DossierPayload }) {
  const { metadata, raw } = payload.dossier;
  const header = raw as { contract?: unknown; revision?: unknown; asOf?: unknown; metadata?: unknown };
  return (
    <Section title="互动与来源信息">
      <p>{`点赞 ${metricReading(metadata.likes)} · 评论 ${metricReading(metadata.comments)} · 收藏 ${metricReading(metadata.collects)}`}</p>
      <p className={styles.note}>评论原文继续从现有评论研究入口读取；评论中的表述不会混入作品正文。</p>
      {/* 折叠诊断：原样序列化（含平台作者编号），旧页如此；普通层不渲染（DR-9）。 */}
      <details className={styles.disclosure}>
        <summary>底稿版本与观察信息</summary>
        <p className={styles.note}>本机诊断保留平台作者编号，用于核对来源身份。</p>
        <pre className={styles.pre}>{JSON.stringify({ contract: header.contract, revision: header.revision, asOf: header.asOf, metadata: header.metadata }, null, 2)}</pre>
      </details>
    </Section>
  );
}
