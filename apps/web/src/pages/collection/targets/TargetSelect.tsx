import { useEffect, useRef, useState } from 'react';
export function TargetSelect({
  className = 'c-tg-field',
  value,
  onChange,
  options,
  label,
  name,
  required = false,
}: {
  className?: string;
  value: string;
  onChange: (value: string) => void;
  options: { value: string; label: string; disabled?: boolean }[];
  label: string;
  name?: string;
  required?: boolean;
}) {
  const [open, setOpen] = useState(false),
    host = useRef<HTMLSpanElement>(null),
    toggle = useRef<HTMLButtonElement>(null);
  useEffect(() => {
    if (!open) return;
    const close = (event: PointerEvent) => {
      if (!host.current?.contains(event.target instanceof Node ? event.target : null)) setOpen(false);
    };
    document.addEventListener('pointerdown', close);
    return () => document.removeEventListener('pointerdown', close);
  }, [open]);
  return (
    <span
      ref={host}
      className={`${className} c-tg-drawn`}
      data-drawn-select
      onKeyDown={(e) => {
        if (e.key === 'Escape') {
          e.stopPropagation();
          setOpen(false);
          toggle.current?.focus();
        }
      }}
    >
      <select
        name={name}
        aria-label={label}
        value={value}
        onChange={(e) => onChange(e.target.value)}
        required={required}
      >
        {options.map((o) => (
          <option key={o.value} value={o.value} disabled={o.disabled}>
            {o.label}
          </option>
        ))}
      </select>
      <button
        ref={toggle}
        type="button"
        className="c-tg-ctl c-tg-drawn-toggle"
        aria-label={label}
        aria-haspopup="listbox"
        aria-expanded={open}
        onClick={() => setOpen((v) => !v)}
        onKeyDown={(e) => {
          if (e.key === 'ArrowDown' || e.key === 'ArrowUp') {
            e.preventDefault();
            setOpen(true);
            queueMicrotask(() =>
              host.current
                ?.querySelector<HTMLButtonElement>(
                  e.key === 'ArrowDown' ? '[role="option"]' : '[role="option"]:last-child',
                )
                ?.focus(),
            );
          }
        }}
      >
        <span>{options.find((o) => o.value === value)?.label ?? '选择领域'}</span>
        <i className="c-tg-caret" aria-hidden="true" />
      </button>
      <div className="c-tg-drawn-list" role="listbox" aria-label={label} hidden={!open}>
        {options
          .filter((o) => !o.disabled)
          .map((o, index, list) => (
            <button
              type="button"
              role="option"
              key={o.value}
              aria-selected={o.value === value}
              onClick={() => {
                onChange(o.value);
                setOpen(false);
                toggle.current?.focus();
              }}
              onKeyDown={(e) => {
                if (['ArrowDown', 'ArrowUp', 'Home', 'End'].includes(e.key)) {
                  e.preventDefault();
                  const next =
                    e.key === 'Home'
                      ? 0
                      : e.key === 'End'
                        ? list.length - 1
                        : (index + (e.key === 'ArrowDown' ? 1 : list.length - 1)) % list.length;
                  host.current?.querySelectorAll<HTMLButtonElement>('[role="option"]')[next]?.focus();
                }
              }}
            >
              {o.label}
            </button>
          ))}
      </div>
    </span>
  );
}
