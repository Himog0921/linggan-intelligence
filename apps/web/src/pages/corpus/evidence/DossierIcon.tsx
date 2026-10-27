import type { ReactNode } from 'react';
import styles from './DossierDocument.module.css';
type Name = 'clock' | 'chevron' | 'refresh' | 'download' | 'check' | 'info' | 'expand' | 'close' | 'copy';
const paths: Record<Name, ReactNode> = {
  clock: <><circle cx="12" cy="12" r="9"/><path d="M12 7v5l3 2"/></>,
  chevron: <path d="m6 9 6 6 6-6"/>,
  refresh: <><path d="M20 7v5h-5M4 17v-5h5M6.1 7a7 7 0 0 1 11.5-2L20 8M4 16l2.4 3A7 7 0 0 0 18 17"/></>,
  download: <><path d="M12 3v12m-5-5 5 5 5-5M4 15v5h16v-5"/></>,
  check: <path d="m5 12 4 4L19 6"/>,
  info: <><circle cx="12" cy="12" r="9"/><path d="M12 8v5m0 3h.01"/></>,
  expand: <><path d="M14 4h6v6m-6 0 6-6M10 20H4v-6m0 6 6-6"/></>,
  close: <path d="m6 6 12 12M18 6 6 18"/>,
  copy: <><rect x="8" y="8" width="12" height="12" rx="1"/><path d="M16 8V4H4v12h4"/></>,
};
export function DossierIcon({ name }: { name: Name }) {
  return <svg className={styles.icon} aria-hidden="true" viewBox="0 0 24 24">{paths[name]}</svg>;
}
