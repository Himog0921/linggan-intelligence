import type { ReactNode } from 'react';
import { PERSONAL_MENU_LINKS, PRIMARY_ENTRIES, type PrimaryEntry, type PrimarySurface } from './navigation';
import styles from './AppShell.module.css';

interface AppShellProps {
  /** 当前页面所属的一级责任；设置页不属于任何一级入口，传 null。 */
  active: PrimarySurface | null;
  /** 系统区服务状态胶囊的中文文字。 */
  boundaryLabel: string;
  /** 当前页可明确提供的状态；省略时沿用迁移期导航基线。 */
  activeState?: string;
  crumb: ReactNode;
  meta: ReactNode;
  children: ReactNode;
  /** V3 创作专用空间；其他页面继续使用迁移期标准页头。 */
  workbenchMode?: 'compact' | 'immersive';
}

function PrimaryEntryLink({ entry, active, activeState }: { entry: PrimaryEntry; active: PrimarySurface | null; activeState?: string }) {
  const current = entry.surface !== null && entry.surface === active ? 'page' : undefined;
  const label = (
    <>
      <b className="v7-nav-zh">{entry.zh}</b>
      <span className="v7-nav-readout">
        {entry.technicalKey ? <small className="v7-tech-key">{entry.technicalKey}</small> : null}
        <span className="v7-nav-state">{current ? activeState ?? entry.state : entry.state}</span>
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
  // 一级入口指向的页面多数仍由旧服务提供，必须整页跳转，不能交给前端路由。
  return (
    <a href={entry.href} aria-current={current}>
      {label}
    </a>
  );
}

/**
 * 全站唯一继续演进的页头（WEB-REACT-001 R4）。结构与样式类名与 shell.rs 的 `global_header`
 * 保持一致，样式只来自 `shell.css`，迁移期两种页头看起来是同一个。
 */
export function AppShell({ active, activeState, boundaryLabel, crumb, meta, children, workbenchMode }: AppShellProps) {
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
            </nav>
          </details>
        </div>
      </header> : null}
      {children}
    </div>;
  }
  return (
    <div className="v7-app">
      <header className="v7-global-header">
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
              <PrimaryEntryLink key={entry.zh} entry={entry} active={active} {...(activeState === undefined ? {} : { activeState })} />
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
              </nav>
            </details>
          </div>
        </div>
        <div className="v7-context-row">
          <div aria-hidden="true" />
          <div className="v7-context-main">
            <div className="v7-context-crumb">{crumb}</div>
            <div className="v7-context-meta">{meta}</div>
          </div>
        </div>
      </header>
      {children}
    </div>
  );
}
