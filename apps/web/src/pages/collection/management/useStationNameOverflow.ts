import { useLayoutEffect, useRef } from 'react';

/** Reserve control space only when the legacy name is already clipped. */
export function useStationNameOverflow(name: string, editing: boolean) {
  const ref = useRef<HTMLSpanElement>(null);
  useLayoutEffect(() => {
    const label = ref.current;
    if (!label) return;
    let active = true;
    const measure = () => {
      if (!active) return;
      // Measure the original non-shrinking layout first. A short name that fits
      // must retain its complete text and the original control positions.
      delete label.dataset.nameOverflow;
      if (label.scrollWidth > label.clientWidth) label.dataset.nameOverflow = '';
    };
    measure();
    const observer = typeof ResizeObserver === 'undefined' ? null : new ResizeObserver(measure);
    if (label.parentElement) observer?.observe(label.parentElement);
    void document.fonts?.ready.then(measure);
    return () => { active = false; observer?.disconnect(); };
  }, [name, editing]);
  return ref;
}
