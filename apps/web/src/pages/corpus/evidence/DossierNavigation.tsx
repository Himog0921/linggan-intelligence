import { useEffect, useId, useRef, useState, type CSSProperties, type RefObject } from 'react';
import { DossierIcon } from './DossierIcon';
import { formatReadingTime } from './dossierDocumentModel';
import styles from './DossierDocument.module.css';

export interface ReadingNavSection { ref: string; label: string; number: string }
export interface ReadingNavTime { ref: string; startMs: number }
interface Props {
  sections: readonly ReadingNavSection[]; times: readonly ReadingNavTime[];
  current: { section: string | null; group: string | null };
  root: RefObject<HTMLDivElement | null>; toolbar: RefObject<HTMLDivElement | null>;
  onJump: (ref: string, first?: boolean) => void;
}
export function DossierNavigation({ sections, times, current, root, toolbar, onJump }: Props) {
  const [open, setOpen] = useState(false);
  const [position, setPosition] = useState<CSSProperties>({});
  const id = useId();
  const trigger = useRef<HTMLButtonElement>(null);
  const panel = useRef<HTMLDivElement>(null);
  const active = sections.find(section => section.ref === current.section) ?? sections[0];
  useEffect(() => {
    if (!open) return;
    const outside = (event: PointerEvent | FocusEvent) => {
      if (event.target instanceof Node && !panel.current?.contains(event.target) && !trigger.current?.contains(event.target)) setOpen(false);
    };
    const key = (event: KeyboardEvent) => {
      if (event.key !== 'Escape') return;
      event.preventDefault(); event.stopPropagation(); setOpen(false); trigger.current?.focus({ preventScroll: true });
    };
    const resize = () => setOpen(false);
    document.addEventListener('pointerdown', outside, true);
    document.addEventListener('focusin', outside);
    document.addEventListener('keydown', key, true);
    window.addEventListener('resize', resize);
    const frame = requestAnimationFrame(() => panel.current?.querySelector<HTMLButtonElement>('button')?.focus());
    return () => { cancelAnimationFrame(frame); document.removeEventListener('pointerdown', outside, true); document.removeEventListener('focusin', outside); document.removeEventListener('keydown', key, true); window.removeEventListener('resize', resize); };
  }, [open]);
  function toggle() {
    if (!open && root.current && toolbar.current && trigger.current) {
      const host = toolbar.current.getBoundingClientRect(), button = trigger.current.getBoundingClientRect();
      const width = Math.min(294, Math.max(0, host.width - 32));
      setPosition({ left: Math.max(16, Math.min(button.left - host.left, host.width - width - 16)), width, maxHeight: Math.max(0, Math.min(440, root.current.getBoundingClientRect().bottom - host.bottom - 12)) });
    }
    setOpen(value => !value);
  }
  function jump(ref: string, first = false) { setOpen(false); onJump(ref, first); }
  return <div className={styles.readerNav}>
    <button ref={trigger} type="button" className={styles.navToggle} aria-expanded={open} aria-controls={id} aria-label={`章节导航${active ? `，当前${active.number} ${active.label}` : ''}`} title={active ? `${active.number} ${active.label}` : '章节导航'} onClick={toggle}><span>{active ? `${active.number} ${active.label}` : '章节'}</span><DossierIcon name="chevron"/></button>
    {open ? <div ref={panel} id={id} className={styles.navPanel} style={position} aria-label="底稿章节与时间">
      <div className={styles.navHead}><span>跳转阅读位置</span><button type="button" onClick={() => { const first = sections[0]; if (first) jump(first.ref, true); }}>回到开头</button></div>
      <nav className={styles.navSections} aria-label="底稿章节">{sections.map(section => <button type="button" key={section.ref} className={styles.navSection} aria-current={current.section === section.ref ? 'location' : undefined} onClick={() => jump(section.ref)}><span>{section.number}</span>{section.label}</button>)}</nav>
      {times.length ? <><p className={styles.navTimeTitle}>视频口播 · 段落起始时间</p><div className={styles.navTimes}>{times.map(time => <button type="button" key={time.ref} aria-current={current.group === time.ref ? 'location' : undefined} onClick={() => jump(time.ref)}>{formatReadingTime(time.startMs)}</button>)}</div></> : null}
    </div> : null}
  </div>;
}
