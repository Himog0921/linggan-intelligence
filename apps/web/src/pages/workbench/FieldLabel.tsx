import type { ComponentPropsWithoutRef, MouseEvent } from 'react';

/**
 * A `<label>` that wraps a `SelectField`. The browser forwards a click anywhere inside a label to its first control, so
 * choosing an option would click the menu's trigger again and reopen the menu right after it closed. A click on an option
 * is not meant for the label, so its default action is cancelled; the caption still focuses the select as before.
 */
export function FieldLabel({ onClick, ...rest }: ComponentPropsWithoutRef<'label'>) {
  function keepChoiceClosed(event: MouseEvent<HTMLLabelElement>) {
    if (event.target instanceof Element && event.target.closest('[role="option"]')) event.preventDefault();
    onClick?.(event);
  }
  return <label {...rest} onClick={keepChoiceClosed} />;
}
