import { useId, useLayoutEffect, useRef, useState, type CSSProperties, type KeyboardEvent } from 'react';
import styles from './SelectField.module.css';

export interface SelectOption<T extends string> { value: T; label: string; disabled?: boolean }
interface Props<T extends string> {
  label: string;
  value: T;
  options: readonly SelectOption<T>[];
  onChange: (value: T) => void;
  disabled?: boolean;
  compact?: boolean;
}

/** Shared basic C17 candidate: selection only; the caller owns query timing and eligibility. No CMP certification is implied. */
export function SelectField<T extends string>({ label, value, options, onChange, disabled, compact }: Props<T>) {
  const id = useId();
  const [hasPopover] = useState(() => typeof HTMLElement.prototype.showPopover === 'function');
  const trigger = useRef<HTMLButtonElement>(null);
  const menu = useRef<HTMLDivElement>(null);
  const [open, setOpen] = useState(false);
  const [active, setActive] = useState(0);
  const [position, setPosition] = useState<CSSProperties>({});
  const search = useRef({ text: '', time: 0 });
  const selected = options.findIndex(option => option.value === value);

  useLayoutEffect(() => {
    if (!open) return;
    const popup = menu.current;
    popup?.showPopover?.();
    const place = () => {
      const rect = trigger.current?.getBoundingClientRect();
      if (!rect || !popup) return;
      const gap = parseFloat(getComputedStyle(popup).getPropertyValue('--lgi-space-1')) || 0;
      const edge = parseFloat(getComputedStyle(popup).getPropertyValue('--lgi-space-3')) || 0;
      const width = Math.min(Math.max(rect.width, popup.scrollWidth + popup.offsetWidth - popup.clientWidth), window.innerWidth - edge * 2);
      const below = window.innerHeight - rect.bottom - edge - gap;
      const above = rect.top - edge - gap;
      const flip = below < popup.scrollHeight && above > below;
      setPosition({ width, left: Math.max(edge, Math.min(rect.left, window.innerWidth - width - edge)),
        maxHeight: Math.max(0, flip ? above : below), top: flip ? undefined : rect.bottom + gap,
        bottom: flip ? window.innerHeight - rect.top + gap : undefined });
    };
    place();
    const outside = (event: PointerEvent) => {
      if (event.target instanceof Node && !trigger.current?.contains(event.target) && !popup?.contains(event.target)) setOpen(false);
    };
    const dismiss = (event: Event) => { if (event.target instanceof Node && popup?.contains(event.target)) return; setOpen(false); };
    document.addEventListener('pointerdown', outside, true);
    window.addEventListener('resize', dismiss);
    document.addEventListener('scroll', dismiss, true);
    return () => {
      if (popup?.hidePopover && popup.matches(':popover-open')) popup.hidePopover();
      document.removeEventListener('pointerdown', outside, true);
      window.removeEventListener('resize', dismiss);
      document.removeEventListener('scroll', dismiss, true);
    };
  }, [open]);

  useLayoutEffect(() => {
    if (open) menu.current?.children[active]?.scrollIntoView?.({ block: 'nearest' });
  }, [active, open]);

  function choose(index: number) {
    const option = options[index];
    if (!option || option.disabled) return;
    setOpen(false);
    trigger.current?.focus({ preventScroll: true });
    if (option.value !== value) onChange(option.value);
  }
  function move(start: number, direction: number) {
    for (let step = 1; step <= options.length; step++) {
      const index = (start + direction * step + options.length) % options.length;
      if (!options[index]?.disabled) return setActive(index);
    }
  }
  function keys(event: KeyboardEvent) {
    if (event.key === 'Escape' && open) {
      event.preventDefault(); event.stopPropagation(); setOpen(false); return;
    }
    if (event.key === 'Tab') { setOpen(false); return; }
    if (['ArrowDown', 'ArrowUp', 'Home', 'End', 'Enter', ' '].includes(event.key)) {
      event.preventDefault(); event.stopPropagation();
      if (!open) {
        if (event.key === 'Home') move(options.length - 1, 1);
        else if (event.key === 'End') move(0, -1);
        else setActive(Math.max(0, selected));
        setOpen(true);
      } else if (event.key === 'Enter' || event.key === ' ') choose(active);
      else if (event.key === 'Home') move(options.length - 1, 1);
      else if (event.key === 'End') move(0, -1);
      else move(active, event.key === 'ArrowDown' ? 1 : -1);
      return;
    }
    if (event.key.length === 1 && !event.ctrlKey && !event.metaKey && !event.altKey) {
      const now = Date.now();
      search.current = { text: (now - search.current.time < 700 ? search.current.text : '') + event.key, time: now };
      const index = options.findIndex(option => !option.disabled && option.label.toLocaleLowerCase().startsWith(search.current.text.toLocaleLowerCase()));
      if (index >= 0) { setActive(index); setOpen(true); }
    }
  }
  return <span className={styles.select} data-compact={compact || undefined}>
    <button ref={trigger} type="button" className={styles.trigger} role="combobox" aria-label={label}
      aria-describedby={`${id}-value`} aria-expanded={open} aria-controls={id} aria-haspopup="listbox" aria-activedescendant={open ? `${id}-${active}` : undefined}
      disabled={disabled} onKeyDown={keys} onClick={() => { setActive(Math.max(0, selected)); setOpen(current => !current); }}>
      <span id={`${id}-value`} title={options[selected]?.label}>{options[selected]?.label ?? value}</span><span className={styles.chevron} aria-hidden="true">⌄</span>
    </button>
    {open ? <div ref={menu} id={id} popover={hasPopover ? 'manual' : undefined} role="listbox" aria-label={label} className={styles.menu} style={position}>
      {options.map((option, index) => <div key={option.value} id={`${id}-${index}`} role="option"
        aria-selected={option.value === value} aria-disabled={option.disabled || undefined} data-active={active === index}
        onPointerMove={() => { if (!option.disabled) setActive(index); }}
        onPointerDown={event => event.preventDefault()} onClick={() => choose(index)}>
        <span title={option.label}>{option.label}</span><span aria-hidden="true">{option.value === value ? '✓' : ''}</span>
      </div>)}
    </div> : null}
  </span>;
}
