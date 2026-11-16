import type { CreatorGroup } from './opportunityApi';
import { flagLabels, formatValue } from './labels';
import styles from './Opportunity.module.css';

/** All metric values carry the server's quality flags; no thresholds are inferred here. */
export function Metric({ value, flags, suffix = '' }: { value: number | null; flags: readonly string[]; suffix?: string }) {
  const note = flags.map(flag => flagLabels[flag] ?? flag).join('；');
  return <span className={styles.metricValue} title={value === null ? note || '未知' : `${value}${note ? `；${note}` : ''}`} data-quality={flags.join(' ')}>{formatValue(value)}{value === null ? '' : suffix}</span>;
}
export function QualityFlags({ flags }: { flags: readonly string[] }) {
  return flags.length ? <ul className={styles.flags} aria-label="质量标记">{flags.map(flag => <li key={flag}>{flagLabels[flag] ?? flag}</li>)}</ul> : null;
}
export function FocusAuthors({ groups, protocol }: { groups: readonly CreatorGroup[]; protocol: number | null }) {
  const group = groups.find(item => item.protocolVersion === protocol);
  if (!group || !group.ranked.length) return <span className={styles.muted}>暂无可排序作者</span>;
  return <span>{group.ranked.slice(0, 3).map(item => item.name ?? '姓名未知').join('、')}</span>;
}
