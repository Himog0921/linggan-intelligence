import { useEffect, useId, useMemo, useRef, useState } from 'react';
import { ModalDialog } from '../../../corpus/ModalDialog';
import { ActionButton } from '../../../shared/ActionButton';
import { DossierIcon } from './DossierIcon';
import { FocusTimeMenu, type FocusTimeMenuHandle } from './FocusTimeMenu';
import { TranscriptGroups } from './DossierDocumentBlocks';
import { groupTime, readingAnchor, transcriptStats, type ResolvedReadingGroup } from './dossierDocumentModel';
import { captureReadingPosition, jumpReading, restoreReadingPosition, type ReadingPosition } from './useDossierReadingPosition';
import styles from './DossierDocument.module.css';
const positions = new Map<string, ReadingPosition>();

export function DossierFocusDialog({ title, groups, memoryKey, onClose }: { title: string; groups: readonly ResolvedReadingGroup[]; memoryKey: string; onClose: () => void }) {
  const id = useId();
  const root = useRef<HTMLDivElement>(null);
  const timeMenu = useRef<FocusTimeMenuHandle>(null);
  const prefix = `focus-${id}`;
  const times = useMemo(
    () => groups.map(value => ({ ref: value.group.ref, time: groupTime(value) })).filter((entry): entry is { ref: string; time: NonNullable<ReturnType<typeof groupTime>> } => entry.time !== null),
    [groups],
  );
  const [selected, setSelected] = useState(times[0]?.ref ?? '');
  useEffect(() => {
    const element = root.current;
    if (!element) return;
    let scrollFrame = 0;
    const currentAnchor = () => {
      const y = element.getBoundingClientRect().top + 15;
      let active = '';
      for (const anchor of Array.from(element.querySelectorAll<HTMLElement>('[data-transcript-paragraph]'))) {
        if (anchor.getBoundingClientRect().top <= y || !active) active = anchor.dataset.readingAnchor ?? '';
      }
      return active;
    };
    const sync = () => {
      const active = currentAnchor();
      if (active) setSelected(active);
    };
    const save = () => { if (element.isConnected && element.clientHeight > 0) positions.set(memoryKey, captureReadingPosition(element, 14)); };
    // 手动滚动后，触发器显示的时间要跟着实际阅读位置走，不能停在打开时的旧值。
    // 位置在滚动当下就落盘：紧接着关闭时不能因为等下一帧而丢掉刚读到的位置。
    const onScroll = () => {
      save();
      if (scrollFrame) return;
      scrollFrame = requestAnimationFrame(() => { scrollFrame = 0; sync(); });
    };
    const frame = requestAnimationFrame(() => {
      restoreReadingPosition(element, 14, positions.get(memoryKey) ?? null);
      sync();
    });
    element.addEventListener('scroll', onScroll, { passive: true });
    return () => {
      cancelAnimationFrame(frame);
      if (scrollFrame) cancelAnimationFrame(scrollFrame);
      save();
      element.removeEventListener('scroll', onScroll);
    };
  }, [memoryKey]);
  function jumpToTime(value: string) {
    setSelected(value);
    const target = document.getElementById(readingAnchor(prefix, value));
    if (root.current && target && root.current.contains(target)) jumpReading(root.current, target, 14);
  }
  // 原生 dialog 的 Esc 先经过这里：菜单开着时只关菜单并回到触发器，第二次 Esc 才交给 ModalDialog 退出专注。
  function menuCancelGuard(dialog: HTMLDialogElement) {
    const cancel = (event: Event) => {
      if (!timeMenu.current?.isOpen()) return;
      event.preventDefault();
      event.stopImmediatePropagation();
      timeMenu.current.close();
    };
    dialog.addEventListener('cancel', cancel, true);
    return () => dialog.removeEventListener('cancel', cancel, true);
  }
  return <ModalDialog labelledBy={id} className={`${styles.surface} ${styles.dialog} ${styles.focusDialog}`} onClose={onClose} closeOnBackdrop setup={menuCancelGuard}>
    <header className={styles.modalHeader}><div><h2 id={id}>视频口播 · 专注阅读</h2><p className={styles.focusTitle} title={title}>{title}</p></div><button type="button" className={styles.close} aria-label="返回底稿" onClick={onClose}><DossierIcon name="close"/></button></header>
    <div className={styles.focusTools}>
      <span>{transcriptStats(groups)}</span>
      {times.length ? (
        <div className={styles.timeField}>
          <span className={styles.timeLabel}>跳到</span>
          <FocusTimeMenu ref={timeMenu} times={times} selected={selected} onJump={jumpToTime}/>
        </div>
      ) : <span>时间尚未取得</span>}
    </div>
    <div className={styles.focusBody} ref={root} tabIndex={0} aria-label="完整口播正文"><div className={styles.focusDocument}><TranscriptGroups groups={groups} prefix={prefix}/><p className={styles.focusEnd}>已显示本次全部口播文字</p></div></div>
    <footer className={styles.modalFooter}><span className={styles.focusHelp}>按 Esc 返回 · 保留原阅读位置</span><ActionButton className={`${styles.control} ${styles.primary}`} onClick={onClose}>返回底稿</ActionButton></footer>
  </ModalDialog>;
}
