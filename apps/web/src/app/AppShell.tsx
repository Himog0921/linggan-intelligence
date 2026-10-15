import type { ReactNode } from 'react';
import { PERSONAL_MENU_LINKS, PRIMARY_ENTRIES, type PrimaryEntry, type PrimarySurface } from './navigation';

interface AppShellProps {
  /** 当前页面所属的一级责任；设置页不属于任何一级入口，传 null。 */
  active: PrimarySurface | null;
  /** 系统区服务状态胶囊的中文文字。 */
  boundaryLabel: string;
  /** 当前页可明确提供的状态；省略时沿用迁移期导航基线。 */
  activeState?: string;
  /**
   * 一级导航「采集」项的真实状态词（与 shell.rs 的 `collection_state` 等价）。读得到时用真的，
   * 省略时保持「状态未知」——读不到就是未知，绝不因此宣布已接通或为空。
   */
  collectionState?: string | undefined;
  crumb: ReactNode;
  meta: ReactNode;
  children: ReactNode;
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
export function AppShell({ active, activeState, collectionState, boundaryLabel, crumb, meta, children }: AppShellProps) {
  const states: EntryStates = { activeState, collectionState };
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
