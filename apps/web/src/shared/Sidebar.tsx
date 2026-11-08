import { useContext, useId, type ComponentPropsWithoutRef } from 'react';
import { SidebarState } from './sidebarState';
import { IntelligenceIcon } from './IntelligenceIcon';

/** Shared shell slot. Collapsing changes presentation only, never route/query or business scope. */
export function Sidebar({ children, className = '', toggleLabels, ...props }: ComponentPropsWithoutRef<'aside'> & { toggleLabels?: { expanded: string; collapsed: string } }) {
  const { collapsed, toggle } = useContext(SidebarState);
  const id = useId();
  const expandedLabel = toggleLabels?.expanded ?? '折叠侧栏';
  const collapsedLabel = toggleLabels?.collapsed ?? '展开侧栏';
  return <aside {...props} className={`v7-side ${className}`}>
    <button type="button" className="v7-side-toggle" aria-label={collapsed ? collapsedLabel : expandedLabel} aria-expanded={!collapsed} aria-controls={id} onClick={toggle} title={collapsed ? collapsedLabel : expandedLabel}>
      <IntelligenceIcon name={collapsed ? 'sidebar-open' : 'sidebar-close'}/>
      <span>{collapsed ? collapsedLabel : expandedLabel}</span>
    </button>
    <div id={id} className="v7-side-content">{children}</div>
  </aside>;
}

const paths: Record<string, string> = {
  targets: 'M3 12h18M12 3v18M5 12a7 7 0 1 0 14 0a7 7 0 1 0-14 0',
  tasks: 'M5 3h14v18H5zM8 8h8M8 12h8M8 16h5',
  overview: 'M3 13h7V3H3zM14 21h7V11h-7zM3 21h7v-4H3zM14 7h7V3h-7z',
  publications: 'M5 3h14v18H5zM8 7h8M8 11h8M8 15h5',
  accounts: 'M4 5h16v14H4zM8 9h3M8 13h3M15 9h2M15 13h2',
  daily: 'M4 5h16v16H4zM8 2v6M16 2v6M4 10h16M8 14h3',
  io: 'M4 16v5h16v-5M12 3v12M7 10l5 5 5-5',
  default: 'M4 4h16v16H4zM8 8h8M8 12h8M8 16h5',
};
export function NavIcon({ name }: { name: string }) {
  if (name === 'evidence' || name === 'comments' || name === 'creators') return <IntelligenceIcon name={name} className="v7-side-icon"/>;
  return <svg className="v7-side-icon" viewBox="0 0 24 24" aria-hidden="true"><path d={paths[name] ?? paths.default}/></svg>;
}
