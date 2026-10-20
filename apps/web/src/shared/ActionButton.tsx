import type { ComponentPropsWithRef } from 'react';
import styles from './ActionButton.module.css';

import type { ActionVariant } from './lidsComponents.generated';
export type { ActionVariant } from './lidsComponents.generated';
export interface ActionButtonProps extends ComponentPropsWithRef<'button'> {
  variant?: ActionVariant;
  busy?: boolean;
}

/** The V8 action family is opt-in; navigational and selection surfaces stay separate. */
export function ActionButton({ variant = 'outline', busy = false, disabled, className, children,
  type = 'button', 'aria-busy': ariaBusy, ...props }: ActionButtonProps) {
  const loading = busy || variant === 'loading';
  return <button {...props} type={type} className={[styles.action, styles[variant === 'loading' ? 'outline' : variant], className].filter(Boolean).join(' ')}
    data-action-button="v8" data-lids-component="C09" data-variant={variant} data-loading={loading ? '' : undefined}
    aria-busy={loading ? true : ariaBusy} disabled={disabled || loading}>
    {loading && <span className={styles.cells} aria-hidden="true"><i/><i/><i/><i/></span>}
    <span className={styles.label}>{children}</span>
  </button>;
}
