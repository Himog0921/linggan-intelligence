import type { ReactNode } from 'react';
import './IntelligenceIcon.css';

type ExtraKind = 'creators' | 'sidebar-open' | 'sidebar-close' | 'clock' | 'info' | 'check' | 'chevron' | 'refresh' | 'copy' | 'expand' | 'close' | 'normal' | 'wide' | 'focus';
export type IntelligenceIconName = 'evidence' | 'comments' | 'download' | ExtraKind;

function Pane({divider}:{divider:number}) {
  return <><path className="tone-face" d={`M${divider} 5.75H19.75v12.5H${divider}z`}/><rect x="4.25" y="5.75" width="15.5" height="12.5" rx="1"/><path className="pane-divider" d={`M${divider} 5.75v12.5`}/></>;
}
const shapes: Record<IntelligenceIconName,ReactNode> = {
    evidence: <><path className="rear-contour" d="M8 10.25v-5.5h8v5.5"/><path className="paper-rule" d="M10.75 7.5h2.5"/><path className="pocket-face" d="M4.25 10.25h15.5l-1.75 9H6z"/><path className="signal-stroke" d="M9 15h4"/></>,
    comments: <><path className="tone-face" d="M8.5 4.75h10.25a1.5 1.5 0 0 1 1.5 1.5v6.25H16.5V8.25h-8z"/><path className="signal-stroke rear-contour" d="M8.5 4.75h10.25a1.5 1.5 0 0 1 1.5 1.5v6.25"/><path d="M5.25 8.25H15a1.5 1.5 0 0 1 1.5 1.5V15a1.5 1.5 0 0 1-1.5 1.5H9L5.25 20v-3.5a1.5 1.5 0 0 1-1.5-1.5V9.75a1.5 1.5 0 0 1 1.5-1.5z"/><path d="M7.25 12.25H13"/></>,
    download: <><g className="export-tray"><path className="tone-face" d="M4.5 14.75v3a1.5 1.5 0 0 0 1.5 1.5h12a1.5 1.5 0 0 0 1.5-1.5v-3"/><path d="M4.5 14.75v3a1.5 1.5 0 0 0 1.5 1.5h12a1.5 1.5 0 0 0 1.5-1.5v-3"/><path className="export-trace signal-stroke" pathLength="1" d="M8.5 17.5h7"/></g><g className="export-payload"><path className="export-stem" pathLength="1" d="M12 3.75V13"/><path className="export-head" d="m8.5 9.5 3.5 3.5 3.5-3.5"/></g></>,
  creators: <><path className="creator-rear signal-stroke rear-contour" d="M17.25 12.25a6 6 0 0 1 3 5.25V19"/><circle className="creator-head" cx="10.5" cy="7.75" r="3.25"/><path d="M4.25 19v-1.5a6.25 6.25 0 0 1 12.5 0V19"/></>,
  'sidebar-close': <><path className="tone-face" d="M4.25 5.25h5v13.5h-5z"/><rect x="4.25" y="5.25" width="15.5" height="13.5" rx="1"/><path className="rear-contour" d="M9.25 5.25v13.5"/><path className="side-arrow side-arrow-left" d="m15.75 9.25-2.75 2.75 2.75 2.75"/></>,
  'sidebar-open': <><path className="tone-face" d="M4.25 5.25h5v13.5h-5z"/><rect x="4.25" y="5.25" width="15.5" height="13.5" rx="1"/><path className="rear-contour" d="M9.25 5.25v13.5"/><path className="side-arrow side-arrow-right" d="m13 9.25 2.75 2.75L13 14.75"/></>,
  clock: <><circle cx="12" cy="12" r="8"/><path className="clock-hand" d="M12 7.25V12l3.25 2"/></>,
  info: <><circle className="state-face" cx="12" cy="12" r="8"/><circle cx="12" cy="12" r="8"/><path d="M12 7.75v5.5"/><circle className="ink-solid" cx="12" cy="16.25" r=".7"/></>,
  check: <><circle className="state-face" cx="12" cy="12" r="8"/><path className="check-mark" pathLength="1" d="m6.5 12 3.75 3.75 7.25-7.5"/></>,
  chevron: <g className="chapter-chevron"><path d="m6.5 9.5 5.5 5 5.5-5"/></g>,
  refresh: <g className="refresh-wheel"><path d="M5 9a7.5 7.5 0 0 1 13-3l1 1.75M19 3.75v4h-4M19 15a7.5 7.5 0 0 1-13 3l-1-1.75M5 20.25v-4h4"/></g>,
  copy: <><path className="signal-stroke rear-contour" d="M15.25 7.75v-3.5h-11v11h3.5"/><g className="copy-front"><rect className="pocket-face" x="7.75" y="7.75" width="11.5" height="11.5" rx="1"/></g></>,
  expand: <><g className="focus-out focus-out-top"><path d="M14.75 4.75h4.5v4.5m0-4.5-5.5 5.5"/></g><g className="focus-out focus-out-bottom"><path className="signal-stroke rear-contour" d="M9.25 19.25h-4.5v-4.5m0 4.5 5.5-5.5"/></g></>,
  close: <g className="close-lines"><path d="m7 7 10 10M17 7 7 17"/></g>,
  normal: <Pane divider={13.5}/>,
  wide: <Pane divider={10.5}/>,
  focus: <Pane divider={7.5}/>,
};

/** Decorative SVG. The owning control supplies the accessible name and business state. */
export function IntelligenceIcon({name,className='',size,expanded=false}:{name:IntelligenceIconName;className?:string;size?:number;expanded?:boolean}) {
  return <svg className={`lgi-icon ${className}`} data-icon={name} data-expanded={expanded || undefined} aria-hidden="true" focusable="false" viewBox="0 0 24 24" width={size ?? '1em'} height={size ?? '1em'}>{shapes[name]}</svg>;
}
