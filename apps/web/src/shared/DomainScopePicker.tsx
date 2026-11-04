import { useEffect, useRef, useState, type ReactNode } from 'react';

export interface DomainScopePickerProps {
  label: string;
  summary: ReactNode;
  children: ReactNode;
  disabled?: boolean;
  className?: string;
}

/** Shared disclosure behavior; page adapters retain option eligibility and selection semantics. */
export function DomainScopePicker({
  label,
  summary,
  children,
  disabled = false,
  className = 'v7-domain-picker',
}: DomainScopePickerProps) {
  const [open, setOpen] = useState(false);
  const root = useRef<HTMLDetailsElement>(null);
  const trigger = useRef<HTMLElement>(null);

  useEffect(() => {
    if (!open || disabled) return;
    const closeOnOutsidePointer = (event: PointerEvent) => {
      if (event.target instanceof Node && !root.current?.contains(event.target)) setOpen(false);
    };
    document.addEventListener('pointerdown', closeOnOutsidePointer, true);
    return () => document.removeEventListener('pointerdown', closeOnOutsidePointer, true);
  }, [open, disabled]);

  return (
    <details
      ref={root}
      className={className}
      open={open && !disabled}
      onToggle={event => {
        const next = event.currentTarget.open;
        if (next !== open) setOpen(next);
      }}
      onBlur={event => {
        if (!event.currentTarget.contains(event.relatedTarget)) setOpen(false);
      }}
      onKeyDown={event => {
        if (event.key === 'Escape') {
          event.preventDefault();
          setOpen(false);
          trigger.current?.focus();
        }
      }}
    >
      <summary
        ref={trigger}
        aria-label={label}
        aria-disabled={disabled || undefined}
        onClick={event => {
          if (disabled) event.preventDefault();
        }}
      >
        {summary}
      </summary>
      {children}
    </details>
  );
}
