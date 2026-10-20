import { useLayoutEffect, useRef, useState, type ReactNode } from 'react';
import { FontNotice } from '../shared/FontNotice';
import { SidebarState } from '../shared/sidebarState';
import { PERSONAL_MENU_LINKS, PRIMARY_ENTRIES, type PrimaryEntry, type PrimarySurface } from './navigation';
import styles from './AppShell.module.css';

interface AppShellProps {
  /** 当前页面所属的一级责任；设置页不属于任何一级入口，传 null。 */
  active: PrimarySurface | null;
  /** 系统区服务状态胶囊的中文文字。 */
  boundaryLabel: ReactNode;
  className?: string;
  /** 当前页可明确提供的状态；省略时沿用迁移期导航基线。 */
  activeState?: string;
  /**
   * 一级导航「采集」项的真实状态词（与 shell.rs 的 `collection_state` 等价）。读得到时用真的，
   * 省略时保持「状态未知」——读不到就是未知，绝不因此宣布已接通或为空。
   */
  collectionState?: string | undefined;
  crumb: ReactNode;
  /** 可选三级业务页导航，固定在 Context Bar 路径之后；展示模式开关仍留内容区。 */
  contextNav?: ReactNode;
  meta: ReactNode;
  children: ReactNode;
  /** V3 创作专用空间；其他页面继续使用迁移期标准页头。 */
  workbenchMode?: 'compact' | 'immersive';
}

interface EntryStates {
  activeState: string | undefined;
  collectionState: string | undefined;
}

/** 采集项的真实状态词只作用于采集项（不论当前在哪一页）；当前页状态只作用于当前项。 */
function stateOf(entry: PrimaryEntry, current: boolean, { activeState, collectionState }: EntryStates): string {
  if (entry.surface === 'collection' && collectionState !== undefined) return collectionState;
  if (current && activeState !== undefined) return activeState;
  return entry.state;
}

function PrimaryEntryLink({ entry, active, states }: { entry: PrimaryEntry; active: PrimarySurface | null; states: EntryStates }) {
  const current = entry.surface !== null && entry.surface === active ? 'page' : undefined;
  const label = (
    <>
      <b className="v7-nav-zh">{entry.zh}</b>
      <span className="v7-nav-readout">
        {entry.technicalKey ? <small className="v7-tech-key">{entry.technicalKey}</small> : null}
        <span className="v7-nav-state">{stateOf(entry, current !== undefined, states)}</span>
      </span>
    </>
  );
  if (entry.href === null) {
    return (
      <button type="button" disabled aria-disabled="true" aria-current={current}>
        {label}
      </button>
    );
  }
  // 职责根入口由 Axum 精确跳转到唯一 React 页面，保留可复制的默认入口。
  return (
    <a href={entry.href} aria-current={current}>
      {label}
    </a>
  );
}

/**
 * 全站唯一 React 页头；样式由 apps/web/src/styles/shell.css 提供。
 */
export function AppShell({ className, active, activeState, collectionState, boundaryLabel, crumb, contextNav, meta, children, workbenchMode }: AppShellProps) {
  const states: EntryStates = { activeState, collectionState };
  const [collapsed, setCollapsed] = useState(false);
  const header = useRef<HTMLElement>(null);

  // 固定抽屉的起点必须等于实际共享页头的下沿：窄屏一级导航换行、三级导航及字体完成加载都会改变它。
  // Collection 抽屉经 portal 挂到 body，故写入既有根变量；卸载时原样恢复，工作台特例不受影响。
  useLayoutEffect(() => {
    if (workbenchMode) return;
    const root = document.documentElement;
    const shellHeader = header.current;
    if (!shellHeader) return;
    const previous = root.style.getPropertyValue('--v7-header-height');
    let frame: number | null = null;
    let disposed = false;
    const measure = () => {
      frame = null;
      if (disposed || !shellHeader.isConnected) return;
      root.style.setProperty('--v7-header-height', `${Math.max(0, shellHeader.getBoundingClientRect().bottom)}px`);
    };
    const schedule = () => {
      if (disposed) return;
      if (frame === null) frame = window.requestAnimationFrame(measure);
    };
    // useLayoutEffect 在绘制前先量一次，避免 portal 在首帧沿用 CSS 的估算值。
    measure();
    const observer = typeof ResizeObserver === 'undefined' ? null : new ResizeObserver(schedule);
    // The measured value is the header's border-box bottom, so observe the border box: a change of the header's own
    // padding or border moves that bottom without touching its content box, which the default observation would miss.
    observer?.observe(shellHeader, { box: 'border-box' });
    window.addEventListener('resize', schedule);
    // 窄屏页头随文档一起滚出视口时，fixed portal 也必须跟随当前可见下沿。
    window.addEventListener('scroll', schedule, { passive: true });
    void document.fonts?.ready?.then(() => {
      if (!disposed) schedule();
    });
    return () => {
      disposed = true;
      if (frame !== null) window.cancelAnimationFrame(frame);
      observer?.disconnect();
      window.removeEventListener('resize', schedule);
      window.removeEventListener('scroll', schedule);
      if (previous) root.style.setProperty('--v7-header-height', previous);
      else root.style.removeProperty('--v7-header-height');
    };
  }, [workbenchMode]);
  if (workbenchMode) {
    return <div className={`${styles.workbench} ${workbenchMode === 'immersive' ? styles.immersive : ''}`}>
      {workbenchMode === 'compact' ? <header className={styles.header}>
        <div className={styles.crumb}>{crumb}</div>
        <div className={styles.menus}>
          <details className={styles.menu} onKeyDown={event => {
            if (event.key === 'Escape') { event.currentTarget.open = false; event.currentTarget.querySelector('summary')?.focus(); }
          }}>
            <summary>intelligence <span aria-hidden="true">⌄</span></summary>
            <nav aria-label="一级导航">
              {PRIMARY_ENTRIES.map(entry => entry.href ? <a key={entry.zh} href={entry.href} aria-current={entry.surface === active ? 'page' : undefined}>{entry.zh}</a> : <span key={entry.zh} aria-disabled="true">{entry.zh} · 未开放</span>)}
              {PERSONAL_MENU_LINKS.map(link => <a key={link.href} href={link.href}>{link.label}</a>)}
              <FontNotice/>
            </nav>
          </details>
        </div>
      </header> : null}
      {children}
    </div>;
  }
  return (
    <SidebarState value={{ collapsed, toggle: () => setCollapsed(value => !value) }}>
    <div className={["v7-app", className].filter(Boolean).join(" ")} data-sidebar={collapsed ? "collapsed" : "expanded"}>
      <header ref={header} className="v7-global-header" data-v7-shell-header>
        <div className="v7-global-row">
          <div className="v7-global-brand" aria-label="Linggan Intelligence">
            <div className="v7-li-mark">LI</div>
            <div className="v7-global-brand-copy">
              <div className="v7-global-brand-name">Linggan Intelligence</div>
              <div className="v7-global-brand-sub">
                <span className="v7-brand-sub-zh">领域情报工作台</span>
                <small className="v7-tech-key">EDITORIAL INTELLIGENCE</small>
              </div>
            </div>
          </div>
          <nav className="v7-primary-nav" aria-label="一级导航">
            {PRIMARY_ENTRIES.map(entry => (
              <PrimaryEntryLink key={entry.zh} entry={entry} active={active} states={states} />
            ))}
          </nav>
          <div className="v7-global-flex" aria-hidden="true" />
          <div className="v7-global-system">
            <div className="v7-system-boundary">{boundaryLabel}</div>
            <details className="v7-personal-menu">
              <summary className="v7-global-command">个人菜单</summary>
              <nav aria-label="个人菜单">
                {PERSONAL_MENU_LINKS.map(link => (
                  <a key={link.href} href={link.href}>
                    {link.label}
                  </a>
                ))}
                <FontNotice/>
              </nav>
            </details>
          </div>
        </div>
        <div className="v7-context-row">
          <div aria-hidden="true" />
          <div className="v7-context-main">
            <div className="v7-context-crumb">{crumb}</div>
            {contextNav ? <div className="v7-context-section-nav">{contextNav}</div> : null}
            <div className="v7-context-meta">{meta}</div>
          </div>
        </div>
      </header>
      {children}
    </div>
    </SidebarState>
  );
}
