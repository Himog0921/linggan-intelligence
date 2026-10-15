import { useState } from 'react';
import { CRITERIA, FIELD_NAMES, evidenceFor, fieldSource, provenanceText, valueLabel } from './creatorsPresent';
import { evidenceLink, type LinkContext } from './creatorsQuery';
import type { Analysis, Fragment, Judgement } from './creatorsTypes';
import styles from './CreatorDrawer.module.css';

const WORK_REF = /^[0-9a-f]{8}-[0-9a-f-]{27,}$/i;

/** 过长的说明默认收起三行，点「展开说明」看全文。 */
export function ExpandableText({ text }: { text: string }) {
  const [expanded, setExpanded] = useState(false);
  const long = text.length > 80 || text.includes('\n');
  return (
    <>
      <p className={styles.reason} data-clamped={long && !expanded}>
        {text}
      </p>
      {long ? (
        <button type="button" className={styles.textButton} onClick={() => setExpanded(!expanded)}>
          {expanded ? '收起说明' : '展开说明'}
        </button>
      ) : null}
    </>
  );
}

/** 一条引文：来源里的原话，标明取自哪个字段，能回查原作品。 */
export function Quote({ fragment, link, creatorKey }: { fragment: Fragment; link: LinkContext; creatorKey: string }) {
  const workRef = fragment.sourceType === 'work_material' && fragment.workRef && WORK_REF.test(fragment.workRef) ? fragment.workRef : null;
  return (
    <blockquote>
      {fragment.text || '片段暂不可读'}
      <small>
        {fieldSource(fragment.field)}
        {workRef ? (
          <>
            {' · '}
            <a href={evidenceLink(link, creatorKey, workRef)} data-return-key={creatorKey}>
              回查原作品
            </a>
          </>
        ) : null}
      </small>
    </blockquote>
  );
}

interface JudgmentBlockProps {
  analysis: Analysis;
  field: string;
  judgement: Judgement;
  creatorKey: string;
  link: LinkContext;
  /** 作品层判断带 workRef；作者层为 null。 */
  workRef: string | null;
  disabled: boolean;
  onCorrect: (field: string, workRef: string | null) => void;
  /** 作者倾向在抽屉概览里是主结论，字号更大。 */
  lead?: boolean;
}

/**
 * 一项判断：结论、判定门槛、理由、来源与时间、可回查的引文。
 * 门槛一直可见（#384 发现：依据门槛要能直接看到），人工调整与自动结果的来源分开写明。
 */
export function JudgmentBlock({ analysis, field, judgement, creatorKey, link, workRef, disabled, onCorrect, lead }: JudgmentBlockProps) {
  const evidence = evidenceFor(analysis, field, judgement);
  const manual = analysis.manual[field];
  return (
    <div className={styles.field}>
      <div className={lead ? styles.judgmentHead : styles.fieldHead}>
        <strong>
          {lead ? valueLabel(judgement.value) : `${FIELD_NAMES[field] ?? field} · ${valueLabel(judgement.value)}`}
        </strong>
        <button type="button" className={styles.textButton} disabled={disabled} onClick={() => onCorrect(field, workRef)}>
          {judgement.value === 'unknown' ? '补充判断' : '调整判断'}
        </button>
      </div>
      <p className={styles.criteria}>
        {lead ? `${FIELD_NAMES[field] ?? field}：` : '判定门槛：'}
        {CRITERIA[field] ?? ''}
      </p>
      {judgement.reason ? <ExpandableText text={judgement.reason} /> : lead ? <p>当前材料尚不足以形成作者整体判断。</p> : null}
      <p className={styles.help}>
        {provenanceText(analysis, field)}
        {manual?.supportUnavailable ? ' · 当时支持这项判断的材料已变化或不可读，请复核；原人工记录仍保留' : ''}
      </p>
      {evidence.length > 0 ? (
        <details>
          <summary>支持依据 · {evidence.length} 个片段</summary>
          {evidence.map(fragment => (
            <Quote key={fragment.fragmentId} fragment={fragment} link={link} creatorKey={creatorKey} />
          ))}
        </details>
      ) : (
        <p className={styles.help}>暂无可回查的支持片段。</p>
      )}
    </div>
  );
}
