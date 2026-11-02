import {
  forwardRef,
  useEffect,
  useId,
  useImperativeHandle,
  useRef,
  useState,
  type KeyboardEvent as ReactKeyboardEvent,
} from 'react';
import { DossierIcon } from './DossierIcon';
import { formatReadingTime } from './dossierDocumentModel';
import styles from './DossierDocument.module.css';

export interface FocusTimeEntry {
  ref: string;
  time: { startMs: number };
}

export interface FocusTimeMenuHandle {
  close: () => void;
  isOpen: () => boolean;
}

interface FocusTimeMenuProps {
  times: readonly FocusTimeEntry[];
  /** 当前阅读位置对应的时间；手动滚动时由正文回写。 */
  selected: string;
  onJump: (ref: string) => void;
  onOpenChange?: (open: boolean) => void;
}

const MENU_WIDTH = 220;
const MIN_MENU_HEIGHT = 96;
const MAX_MENU_HEIGHT = 420;

/**
 * 专注阅读里的时间入口：沿用底稿目录的局部浮层语言，不用原生 select。
 * 触发器显示当前时间；菜单是 listbox，方向键／Home／End 移动，Enter／Space 明确跳转，Esc 先关菜单。
 * 菜单留在原生 dialog 内，因此仍在顶层里；高度按专注阅读可用空间计算，长列表在菜单内部滚动。
 */
export const FocusTimeMenu = forwardRef<FocusTimeMenuHandle, FocusTimeMenuProps>(function FocusTimeMenu(
  { times, selected, onJump, onOpenChange },
  ref,
) {
  const [open, setOpen] = useState(false);
  const [active, setActive] = useState(0);
  const [position, setPosition] = useState({ width: MENU_WIDTH, maxHeight: MAX_MENU_HEIGHT });
  const id = useId();
  const wrapper = useRef<HTMLDivElement>(null);
  const trigger = useRef<HTMLButtonElement>(null);
  const panel = useRef<HTMLDivElement>(null);
  const selectedIndex = Math.max(0, times.findIndex(entry => entry.ref === selected));
  const current = times[selectedIndex] ?? times[0];
  const listId = `${id}-list`;
  const optionId = (index: number) => `${id}-option-${index}`;

  useImperativeHandle(ref, () => ({
    close: () => {
      setOpen(false);
      trigger.current?.focus({ preventScroll: true });
    },
    isOpen: () => open,
  }), [open]);

  useEffect(() => {
    onOpenChange?.(open);
  }, [open, onOpenChange]);

  // 菜单打开时，正文手动滚动会更新 selected；让活动行跟着实际位置，避免陈旧选中误导。
  useEffect(() => {
    if (open) setActive(selectedIndex);
  }, [open, selectedIndex]);

  useEffect(() => {
    if (!open) return;
    panel.current?.focus({ preventScroll: true });
  }, [open]);

  useEffect(() => {
    if (!open) return;
    panel.current?.querySelector<HTMLElement>(`[data-index="${active}"]`)?.scrollIntoView?.({ block: 'nearest' });
  }, [active, open]);

  useEffect(() => {
    if (!open) return;
    const outsidePointer = (event: PointerEvent) => {
      if (event.target instanceof Node && !wrapper.current?.contains(event.target)) setOpen(false);
    };
    const outsideFocus = (event: FocusEvent) => {
      if (event.target instanceof Node && !wrapper.current?.contains(event.target)) setOpen(false);
    };
    const resize = () => setOpen(false);
    document.addEventListener('pointerdown', outsidePointer, true);
    document.addEventListener('focusin', outsideFocus);
    window.addEventListener('resize', resize);
    return () => {
      document.removeEventListener('pointerdown', outsidePointer, true);
      document.removeEventListener('focusin', outsideFocus);
      window.removeEventListener('resize', resize);
    };
  }, [open]);

  function place() {
    const host = wrapper.current;
    const button = trigger.current;
    if (!host || !button) return;
    const dialog = host.closest('dialog');
    const bounds = dialog?.getBoundingClientRect();
    const rect = button.getBoundingClientRect();
    const dialogWidth = bounds && bounds.width > 0 ? bounds.width : window.innerWidth;
    const dialogBottom = bounds && bounds.height > 0 ? bounds.bottom : window.innerHeight;
    setPosition({
      width: Math.min(MENU_WIDTH, Math.max(160, dialogWidth - 48)),
      maxHeight: Math.max(MIN_MENU_HEIGHT, Math.min(MAX_MENU_HEIGHT, dialogBottom - rect.bottom - 20)),
    });
  }

  function openMenu() {
    place();
    setActive(selectedIndex);
    setOpen(true);
  }

  function jump(index: number) {
    const entry = times[index];
    if (!entry) return;
    setOpen(false);
    onJump(entry.ref);
  }

  function onTriggerKeyDown(event: ReactKeyboardEvent<HTMLButtonElement>) {
    if (event.key !== 'ArrowDown' && event.key !== 'ArrowUp') return;
    event.preventDefault();
    if (!open) openMenu();
    else panel.current?.focus({ preventScroll: true });
  }

  function onPanelKeyDown(event: ReactKeyboardEvent<HTMLDivElement>) {
    switch (event.key) {
      case 'Escape':
        event.preventDefault();
        event.stopPropagation();
        event.nativeEvent.stopImmediatePropagation?.();
        setOpen(false);
        trigger.current?.focus({ preventScroll: true });
        break;
      case 'ArrowDown':
        event.preventDefault();
        setActive(index => Math.min(times.length - 1, index + 1));
        break;
      case 'ArrowUp':
        event.preventDefault();
        setActive(index => Math.max(0, index - 1));
        break;
      case 'Home':
        event.preventDefault();
        setActive(0);
        break;
      case 'End':
        event.preventDefault();
        setActive(times.length - 1);
        break;
      case 'Enter':
      case ' ':
        event.preventDefault();
        jump(active);
        break;
      default:
        break;
    }
  }

  if (!current) return null;
  return (
    <div ref={wrapper} className={styles.timeField}>
      <button
        ref={trigger}
        type="button"
        className={styles.timeTrigger}
        aria-haspopup="listbox"
        aria-expanded={open}
        aria-controls={open ? listId : undefined}
        aria-label={`定位口播时间，当前 ${formatReadingTime(current.time.startMs)}`}
        onClick={() => { if (open) setOpen(false); else openMenu(); }}
        onKeyDown={onTriggerKeyDown}
      >
        <span>{formatReadingTime(current.time.startMs)}</span>
        <DossierIcon name="chevron"/>
      </button>
      {open ? (
        <div
          ref={panel}
          id={listId}
          role="listbox"
          tabIndex={-1}
          className={styles.timeMenu}
          style={position}
          aria-label="口播时间"
          aria-activedescendant={optionId(active)}
          onKeyDown={onPanelKeyDown}
        >
          {times.map((entry, index) => (
            <div
              key={entry.ref}
              id={optionId(index)}
              role="option"
              className={styles.timeOption}
              data-index={index}
              data-active={index === active ? 'true' : undefined}
              aria-selected={entry.ref === selected}
              onPointerDown={event => event.preventDefault()}
              onMouseEnter={() => setActive(index)}
              onClick={() => jump(index)}
            >
              <span>{formatReadingTime(entry.time.startMs)}</span>
              {entry.ref === selected ? <span className={styles.timeCurrent}>当前</span> : null}
            </div>
          ))}
        </div>
      ) : null}
    </div>
  );
});
