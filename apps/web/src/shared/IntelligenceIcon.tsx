import type { CSSProperties, ReactNode } from 'react';
import './IntelligenceIcon.css';

type ExtraKind = 'creators' | 'sidebar-open' | 'sidebar-close' | 'clock' | 'info' | 'check' | 'chevron' | 'refresh' | 'copy' | 'expand' | 'close' | 'normal' | 'wide' | 'focus';
export type IntelligenceIconName = 'evidence' | 'comments' | 'download' | ExtraKind
  | 'external' | 'back' | 'forward' | 'search' | 'filter'
  | 'add' | 'edit' | 'grid' | 'list' | 'zoom-in'
  | 'zoom-out' | 'prev' | 'next' | 'fit' | 'targets'
  | 'tasks' | 'overview' | 'publications' | 'accounts' | 'daily'
  | 'diagnostics' | 'security' | 'link' | 'archive' | 'restore'
  | 'pause' | 'resume' | 'stop' | 'sticky' | 'frame-add'
  | 'adjust' | 'upload' | 'move-up' | 'move-down' | 'password-show'
  | 'password-hide';

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
  'external': <><path className="tone-face" d="M5 6h5v13H5z"/><path d="M10.5 5H6a1 1 0 0 0-1 1v12a1 1 0 0 0 1 1h12a1 1 0 0 0 1-1v-4.5"/><g className="feedback-move"><path d="M11 13l8-8m-5.5 0H19v5.5"/><path className="signal-stroke feedback-trace" pathLength="1" d="M11 13l8-8"/></g></>,
  'back': <><g className="feedback-move"><path d="M19 12H5m5-5-5 5 5 5"/><path className="signal-stroke feedback-trace" pathLength="1" d="M17 12H7"/></g></>,
  'forward': <><g className="feedback-move"><path d="M5 12h14m-5-5 5 5-5 5"/><path className="signal-stroke feedback-trace" pathLength="1" d="M7 12h10"/></g></>,
  'search': <><g className="feedback-move"><circle cx="10.25" cy="10.25" r="5.75"/><path d="m14.35 14.35 4.9 4.9"/><path className="signal-stroke feedback-trace" pathLength="1" d="M6.5 10.25a3.75 3.75 0 0 1 3.75-3.75"/></g></>,
  'filter': <><path d="M4.75 6h14.5M7.5 10.5h9"/><g className="feedback-move"><path d="M10 15h4m-2 0v3.5"/><path className="signal-stroke feedback-trace" pathLength="1" d="M10 15h4"/></g></>,
  'add': <><path d="M6 12h12"/><path className="feedback-move" d="M12 6v12"/></>,
  'edit': <><path className="rear-contour" d="M5 19h14"/><g className="feedback-move"><path d="m6 16 1-4L16 3.75 19.25 7 10.5 16zM13.9 5.8l3.2 3.2"/><path className="signal-stroke feedback-trace" pathLength="1" d="M6 16l1-4"/></g></>,
  'grid': <><rect x="5" y="5" width="5.25" height="5.25" rx=".6"/><rect x="13.75" y="5" width="5.25" height="5.25" rx=".6"/><rect x="5" y="13.75" width="5.25" height="5.25" rx=".6"/><rect className="feedback-move pocket-face" x="13.75" y="13.75" width="5.25" height="5.25" rx=".6"/></>,
  'list': <><path d="M5 6.5h1.5m3.5 0h9M5 17.5h1.5m3.5 0h9"/><g className="feedback-move"><path d="M5 12h1.5m3.5 0h9"/><path className="signal-stroke feedback-trace" pathLength="1" d="M10 12h9"/></g></>,
  'zoom-in': <><circle cx="10" cy="10" r="5.75"/><path d="m14.2 14.2 5 5"/><g className="feedback-move"><path d="M7.25 10h5.5M10 7.25v5.5"/></g></>,
  'zoom-out': <><circle cx="10" cy="10" r="5.75"/><path d="m14.2 14.2 5 5"/><path className="feedback-move" d="M7.25 10h5.5"/></>,
  'prev': <><path className="feedback-move" d="m14 6.5-5 5.5 5 5.5"/></>,
  'next': <><path className="feedback-move" d="m10 6.5 5 5.5-5 5.5"/></>,
  'fit': <><path d="M9 4.75H4.75V9m10.25-4.25h4.25V9m0 6v4.25H15m-6 0H4.75V15"/><rect className="feedback-move pocket-face" x="9.25" y="9.25" width="5.5" height="5.5" rx=".7"/></>,
  'targets': <><path d="M6 4.5v15m-2 0h4"/><g className="feedback-move"><path className="pocket-face" d="M6 5.25h12.5l-2.5 3.25 2.5 3.25H6z"/><path className="signal-stroke" d="M9 8.5h3.75"/></g></>,
  'tasks': <><path d="M8.5 5.75H6a1 1 0 0 0-1 1v12a1 1 0 0 0 1 1h12a1 1 0 0 0 1-1v-12a1 1 0 0 0-1-1h-2.5M8 11h8m-8 4h6"/><rect className="feedback-move pocket-face" x="8.5" y="4" width="7" height="3.5" rx=".75"/></>,
  'overview': <><rect x="4.5" y="5.25" width="15" height="13.5" rx="1"/><path d="M4.5 10.75h15M12 10.75v8"/><path className="feedback-move signal-stroke" d="M7.5 8h4"/></>,
  'publications': <><path d="M16 11v7a1 1 0 0 1-1 1H6a1 1 0 0 1-1-1V6a1 1 0 0 1 1-1h7M8 12h5m-5 3h5"/><g className="feedback-move"><path className="signal-stroke" d="M14.5 9.5 19 5m-4 0h4v4"/></g></>,
  'accounts': <><rect x="4.5" y="6.5" width="15" height="11" rx="1"/><circle cx="9" cy="10.4" r="1.35"/><path d="M6.9 15a2.1 2.1 0 0 1 4.2 0"/><path className="feedback-move signal-stroke" d="M13.5 10.5h3.5m-3.5 3h3.5"/></>,
  'daily': <><rect x="4.25" y="4.5" width="15.5" height="15" rx="1"/><path className="rear-contour" d="M7.15 4.5v15"/><g className="feedback-move" strokeWidth="1.35"><rect x="9.55" y="8.1" width="3.25" height="6.05" rx=".8"/><path d="m14.9 9.3 1.65-1.2v6.05m-1.3 0h2.65"/></g><path className="signal-stroke" d="M10 16.75h6.5"/></>,
  'diagnostics': <><rect x="4.5" y="5.75" width="15" height="12.5" rx="1"/><path className="rear-contour" d="M6.75 12h3l1.25-3.5 2 6.5 1.35-3h2.9"/><path className="signal-stroke feedback-trace" pathLength="1" d="M6.75 12h3l1.25-3.5 2 6.5 1.35-3h2.9"/></>,
  'security': <><path className="pocket-face" d="M12 4.5 18.5 7v5c0 3.4-2.6 5.9-6.5 7.5C8.1 17.9 5.5 15.4 5.5 12V7z"/><path d="M10.25 11V9.8a1.75 1.75 0 0 1 3.5 0V11m-4 0h4.5v4h-4.5z"/></>,
  'link': <><path className="rear-contour" d="m9.3 13.1 4.2-4.2a3 3 0 0 1 4.25 4.25l-2.5 2.5"/><g className="feedback-move"><path d="m14.7 10.9-4.2 4.2a3 3 0 0 1-4.25-4.25l2.5-2.5"/><path className="signal-stroke" d="m10 14 4-4"/></g></>,
  'archive': <><path className="pocket-face" d="M5.5 11.75h13v6.5a1 1 0 0 1-1 1h-11a1 1 0 0 1-1-1z"/><path d="M4.75 10.75h14.5m-9.25 4h4"/><g className="feedback-move"><path d="M9 10.75V4.75h6v6"/><path className="signal-stroke" d="M11 7.25h2"/></g></>,
  'restore': <><path className="pocket-face" d="M5.5 12.5h13v5.75a1 1 0 0 1-1 1h-11a1 1 0 0 1-1-1z"/><path d="M4.75 11.5h14.5"/><g className="feedback-move"><path d="M12 13V5m-3 3 3-3 3 3"/><path className="signal-stroke feedback-trace" pathLength="1" d="M12 13V6"/></g></>,
  'pause': <><rect x="7" y="6" width="3" height="12" rx=".6"/><rect x="14" y="6" width="3" height="12" rx=".6"/></>,
  'resume': <><path d="M8 5.75 18 12 8 18.25z"/></>,
  'stop': <><rect x="6.75" y="6.75" width="10.5" height="10.5" rx=".9"/></>,
  'sticky': <><path d="M19 13.75V5.75a.75.75 0 0 0-.75-.75H5.75a.75.75 0 0 0-.75.75v12.5a.75.75 0 0 0 .75.75h8l5.25-5.25"/><path d="M8 9h7m-7 3h4"/><path className="feedback-move pocket-face" d="M13.75 19v-5.25H19"/></>,
  'frame-add': <><path d="M9 5H5v4m10-4h4v4m0 6v4h-4m-6 0H5v-4"/><path className="feedback-move signal-stroke" d="M9 12h6m-3-3v6"/></>,
  'adjust': <><path d="M5 8h14M5 16h14"/><rect className="feedback-move feedback-plate" x="8" y="5.75" width="2.5" height="4.5" rx=".5"/><rect className="feedback-counter feedback-plate" x="13.5" y="13.75" width="2.5" height="4.5" rx=".5"/></>,
  'upload': <><path className="pocket-face" d="M5 15v2.75a1.5 1.5 0 0 0 1.5 1.5h11a1.5 1.5 0 0 0 1.5-1.5V15"/><g className="feedback-move"><path d="M12 15V4.75m-3.5 3.5L12 4.75l3.5 3.5"/><path className="signal-stroke feedback-trace" pathLength="1" d="M12 14V5.5"/></g></>,
  'move-up': <><path className="rear-contour" d="M5 17h14M14.5 13H19"/><g className="feedback-move"><path d="M9 14V5.75m-3 3 3-3 3 3"/></g></>,
  'move-down': <><path className="rear-contour" d="M5 7h14M14.5 11H19"/><g className="feedback-move"><path d="M9 10v8.25m-3-3 3 3 3-3"/></g></>,
  'password-show': <><path d="M4 12s3-5.5 8-5.5 8 5.5 8 5.5-3 5.5-8 5.5S4 12 4 12Z"/><circle cx="12" cy="12" r="2.25"/></>,
  'password-hide': <><path className="rear-contour" d="M4 12a14 14 0 0 1 3.2-3.6M10.3 6.7a7 7 0 0 1 1.7-.2c5 0 8 5.5 8 5.5a14 14 0 0 1-3.1 3.7M13.7 17.3a7 7 0 0 1-1.7.2C7 17.5 4 12 4 12Z"/><path d="m5 5 14 14"/></>,
};

const feedbackTravel: Partial<Record<IntelligenceIconName, readonly [number, number]>> = {
  'external': [1.2, -1.2],
  'back': [-1.2, 0],
  'forward': [1.2, 0],
  'search': [-0.7, -0.7],
  'filter': [0, 0.95],
  'add': [0, 0.75],
  'edit': [-0.65, 0.65],
  'grid': [0.6, 0.6],
  'list': [0.8, 0],
  'zoom-in': [0, -0.65],
  'zoom-out': [0, 0.65],
  'prev': [-1, 0],
  'next': [1, 0],
  'targets': [0.45, -0.45],
  'tasks': [0, -0.75],
  'overview': [0.65, 0],
  'publications': [0.65, -0.65],
  'accounts': [0.7, 0],
  'daily': [0, -0.45],
  'link': [-0.65, 0.65],
  'archive': [0, 1.1],
  'restore': [0, -1.1],
  'sticky': [0.6, -0.6],
  'frame-add': [0, -0.7],
  'adjust': [0.95, 0],
  'upload': [0, -1.3],
  'move-up': [0, -1],
  'move-down': [0, 1],
};

/** Decorative SVG. The owning control supplies the accessible name and business state. */
export function IntelligenceIcon({name,className='',size,expanded=false}:{name:IntelligenceIconName;className?:string;size?:number;expanded?:boolean}) {
  const travel = feedbackTravel[name];
  const style = travel ? { '--icon-x': `${travel[0]}px`, '--icon-y': `${travel[1]}px` } as CSSProperties : undefined;
  return <svg style={style} className={`lgi-icon ${className}`} data-icon={name} data-expanded={expanded || undefined} aria-hidden="true" focusable="false" viewBox="0 0 24 24" width={size ?? '1em'} height={size ?? '1em'}>{shapes[name]}</svg>;
}
