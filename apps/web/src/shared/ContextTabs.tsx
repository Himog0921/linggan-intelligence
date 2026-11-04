import type { HTMLAttributes, ReactNode } from 'react';

export interface ContextTabsProps extends HTMLAttributes<HTMLElement> {
  label: string;
  children: ReactNode;
}

/** Shared navigation rail for third-level destinations and in-place views. */
export function ContextTabs({ label, className, children, ...props }: ContextTabsProps) {
  return (
    <nav {...props} aria-label={label} className={['v7-tertiary-nav', className].filter(Boolean).join(' ')}>
      {children}
    </nav>
  );
}
