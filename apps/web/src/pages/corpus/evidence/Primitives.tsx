import type { ReactNode } from 'react';
import type { Feedback } from './evidenceFeedback';
import { stateMeta } from './evidencePresent';
import styles from './Primitives.module.css';

/** 状态线：圆点＋中文状态词。原始状态码只放在 `data-state`，不进普通层的文字（F06）。 */
export function StateLine({ state, showCode = false }: { state: string | null | undefined; showCode?: boolean }) {
  const normalized = state || 'UNKNOWN';
  const [label, tone] = stateMeta(normalized);
  return (
    <span className={styles.stateLine} data-state={normalized}>
      <i className={styles.dot} data-tone={tone} aria-hidden="true" />
      <span>{label}</span>
      {showCode ? <code className={styles.tech}>{normalized}</code> : null}
    </span>
  );
}

/** 技术码旁注：只在技术事实与诊断位置使用。 */
export function Tech({ children }: { children: ReactNode }) {
  return <code className={styles.tech}>{children}</code>;
}

export type FactRow = readonly [label: string, value: string | number, code?: string | null];

const UNKNOWN_COLLAPSE_THRESHOLD = 3;

/** 技术键为 UNKNOWN 且值读作未知：折叠只是排布，不改写任何一行。 */
function isUnknownRow([, value, code]: FactRow): boolean {
  return code === 'UNKNOWN' && typeof value === 'string' && value.includes('未知');
}

function FactList({ rows }: { rows: readonly FactRow[] }) {
  return (
    <dl className={styles.facts}>
      {rows.map(([label, value, code]) => (
        <div key={label} className={styles.pair}>
          <dt>{label}</dt>
          <dd>
            {value}
            {code ? <Tech>{code}</Tech> : null}
          </dd>
        </div>
      ))}
    </dl>
  );
}

/**
 * 事实表。连续的「当前未知」事实行不少于 3 项时折叠成「尚未取得 N 项」，展开后逐条原文案；
 * 少于 3 项时平铺。已知值永远不会被折进去。
 */
export function FactGrid({ rows }: { rows: readonly FactRow[] }) {
  const unknown = rows.filter(isUnknownRow);
  if (unknown.length < UNKNOWN_COLLAPSE_THRESHOLD) return <FactList rows={rows} />;
  const known = rows.filter(row => !isUnknownRow(row));
  return (
    <>
      {known.length > 0 ? <FactList rows={known} /> : null}
      <details className={styles.collapsed}>
        <summary>{`尚未取得 ${unknown.length} 项`}</summary>
        <FactList rows={unknown} />
      </details>
    </>
  );
}

export function Section({ title, tech, children }: { title: string; tech?: string; children: ReactNode }) {
  return (
    <section className={styles.section}>
      <h3>
        <span>{title}</span>
        {tech ? <Tech>{tech}</Tech> : null}
      </h3>
      {children}
    </section>
  );
}

export type InlineTone = 'warning' | 'danger' | 'restricted' | 'neutral';

export function InlineState({ tone = 'neutral', title, children, code }: { tone?: InlineTone; title: string; children?: ReactNode; code?: string | null }) {
  return (
    <div className={styles.inline} data-tone={tone}>
      <strong>{title}</strong>
      {children ? <p>{children}</p> : null}
      {code ? <Tech>{code}</Tech> : null}
    </div>
  );
}

/** 「来源信息不完整」说明：弱化的单行灰字，中文说明；原始英文码不外露（F06）。 */
export function SourceNote({ children }: { children: ReactNode }) {
  return <p className={styles.note}>{children}</p>;
}

export function Hint({ children }: { children: ReactNode }) {
  return <p className={styles.hint}>{children}</p>;
}

/** 动作回执。状态用结论词＋位置＋颜色表达，不只靠颜色（LIDS）；读屏以 polite 播报。 */
export function FeedbackLine({ feedback, children }: { feedback: Feedback | null; children?: ReactNode }) {
  if (feedback === null) return <p role="status" aria-live="polite" className={styles.feedbackEmpty} />;
  return (
    <p role="status" aria-live="polite" className={styles.feedback} data-tone={feedback.tone}>
      <b className={styles.feedbackLabel}>{feedback.label}</b>
      <span>{feedback.text}</span>
      {feedback.code ? <Tech>{feedback.code}</Tech> : null}
      {children ? <span className={styles.feedbackActions}>{children}</span> : null}
    </p>
  );
}
