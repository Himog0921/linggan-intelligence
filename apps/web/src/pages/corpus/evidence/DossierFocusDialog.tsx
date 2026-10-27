import { useEffect, useId, useRef, useState } from 'react';
import { ModalDialog } from '../../../corpus/ModalDialog';
import { ActionButton } from '../../../shared/ActionButton';
import { DossierIcon } from './DossierIcon';
import { TranscriptGroups } from './DossierDocumentBlocks';
import { formatReadingTime, groupTime, readingAnchor, transcriptStats, type ResolvedReadingGroup } from './dossierDocumentModel';
import { captureReadingPosition, jumpReading, restoreReadingPosition, type ReadingPosition } from './useDossierReadingPosition';
import styles from './DossierDocument.module.css';
const positions = new Map<string, ReadingPosition>();

export function DossierFocusDialog({ title, groups, memoryKey, onClose }: { title: string; groups: readonly ResolvedReadingGroup[]; memoryKey: string; onClose: () => void }) {
  const id = useId();
  const root = useRef<HTMLDivElement>(null);
  const prefix = `focus-${id}`;
  const times = groups.map(value => ({ ref: value.group.ref, time: groupTime(value) })).filter(entry => entry.time !== null);
  const [selected, setSelected] = useState(times[0]?.ref ?? '');
  useEffect(() => {
    const element = root.current;
    if (!element) return;
    const frame = requestAnimationFrame(() => {
      restoreReadingPosition(element, 14, positions.get(memoryKey) ?? null);
      const y = element.getBoundingClientRect().top + 15;
      let active = '';
      for (const anchor of Array.from(element.querySelectorAll<HTMLElement>('[data-transcript-paragraph]'))) {
        if (anchor.getBoundingClientRect().top <= y || !active) active = anchor.dataset.readingAnchor ?? '';
      }
      if (active) setSelected(active);
    });
    const save = () => { if (element.isConnected && element.clientHeight > 0) positions.set(memoryKey, captureReadingPosition(element, 14)); };
    element.addEventListener('scroll', save, { passive: true });
    return () => { cancelAnimationFrame(frame); save(); element.removeEventListener('scroll', save); };
  }, [memoryKey]);
  return <ModalDialog labelledBy={id} className={`${styles.surface} ${styles.dialog} ${styles.focusDialog}`} onClose={onClose} closeOnBackdrop>
    <header className={styles.modalHeader}><div><h2 id={id}>视频口播 · 专注阅读</h2><p className={styles.focusTitle} title={title}>{title}</p></div><button type="button" className={styles.close} aria-label="返回底稿" onClick={onClose}><DossierIcon name="close"/></button></header>
    <div className={styles.focusTools}><span>{transcriptStats(groups)}</span>{times.length ? <label><span>跳到</span><select value={selected} aria-label="定位口播时间" onChange={event => {
      const value = event.target.value; setSelected(value);
      const target = document.getElementById(readingAnchor(prefix, value));
      if (root.current && target && root.current.contains(target)) jumpReading(root.current, target, 14);
    }}>{times.map(entry => <option value={entry.ref} key={entry.ref}>{formatReadingTime(entry.time!.startMs)}</option>)}</select></label> : <span>时间尚未取得</span>}</div>
    <div className={styles.focusBody} ref={root} tabIndex={0} aria-label="完整口播正文"><div className={styles.focusDocument}><TranscriptGroups groups={groups} prefix={prefix}/><p className={styles.focusEnd}>已显示本次全部口播文字</p></div></div>
    <footer className={styles.modalFooter}><span className={styles.focusHelp}>按 Esc 返回 · 保留原阅读位置</span><ActionButton className={`${styles.control} ${styles.primary}`} onClick={onClose}>返回底稿</ActionButton></footer>
  </ModalDialog>;
}
