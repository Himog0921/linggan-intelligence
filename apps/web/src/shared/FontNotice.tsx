import styles from './FontNotice.module.css';

/** Visible attribution required by the original, unmodified OPPO font license. */
export function FontNotice() {
  return <small className={styles.notice}>本软件使用 OPPO Sans 字体 · <a href="/ui-assets/fonts/lids/NOTICE.txt" target="_blank" rel="noreferrer">字体许可与鸣谢</a></small>;
}
