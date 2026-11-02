import type { ReactNode } from 'react';
import styles from './EvidenceLabel.module.css';

/** Local reading-workspace label. Its text retains the existing heading / disclosure meaning. */
export function EvidenceLabel({ children }: { children: ReactNode }) {
  return <span className={styles.label}>
    <svg className={styles.edge} viewBox="0 0 120 40" preserveAspectRatio="none" aria-hidden="true" focusable="false">
      <path d="M5 4 38 3 76 5 113 2 117 12 115 33 85 36 51 34 7 37 3 26Z" />
      <path d="m9 8 29-1 38 2 33-3 4 8-2 15-26 3-34-2-40 3-4-8Z" />
    </svg>
    <span className={styles.text}>{children}</span>
  </span>;
}
