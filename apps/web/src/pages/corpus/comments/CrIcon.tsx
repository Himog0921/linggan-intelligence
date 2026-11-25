/*
 * Icon outlines from Lucide (ISC License, Copyright (c) 2026 Lucide Icons and Contributors), as embedded in the
 * B-04 design reference (apps/web/design-reference/comment-research-b04.html). Only the shapes used on this page.
 */
import { c } from './styles';

const ICONS = {
  History: ['M3 12a9 9 0 1 0 9-9 9.75 9.75 0 0 0-6.74 2.74L3 8', 'M3 3v5h5', 'M12 7v5l4 2'],
  ArrowRight: ['M5 12h14', 'm12 5 7 7-7 7'],
  ArrowUpRight: ['M7 7h10v10', 'M7 17 17 7'],
  SlidersHorizontal: ['M10 5H3', 'M12 19H3', 'M14 3v4', 'M16 17v4', 'M21 12h-9', 'M21 19h-5', 'M21 5h-7', 'M8 10v4', 'M8 12H3'],
  ListChecks: ['M13 5h8', 'M13 12h8', 'M13 19h8', 'm3 17 2 2 4-4', 'm3 7 2 2 4-4'],
  X: ['M18 6 6 18', 'm6 6 12 12'],
  Info: ['M12 16v-4', 'M12 8h.01'],
} as const;

export type CrIconName = keyof typeof ICONS;

export function CrIcon({ name }: { name: CrIconName }) {
  return <svg className={c('icon')} viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
    {name === 'Info' ? <circle cx="12" cy="12" r="10" /> : null}
    {ICONS[name].map(d => <path key={d} d={d} />)}
  </svg>;
}
