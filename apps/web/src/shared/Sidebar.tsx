import { useContext, useId, type ComponentPropsWithoutRef } from 'react';
import { SidebarState } from './sidebarState';
import { IntelligenceIcon } from './IntelligenceIcon';

/** Shared shell slot. Collapsing changes presentation only, never route/query or business scope. */
export function Sidebar({ children, className = '', ...props }: ComponentPropsWithoutRef<'aside'>) {
  const { collapsed, toggle } = useContext(SidebarState);
  const id = useId();
  return <aside {...props} className={`v7-side ${className}`}>
    <button type="button" className="v7-side-toggle" aria-label={collapsed ? '展开侧栏' : '折叠侧栏'} aria-expanded={!collapsed} aria-controls={id} onClick={toggle} title={collapsed ? '展开侧栏' : '折叠侧栏'}>
      <IntelligenceIcon name={collapsed ? 'sidebar-open' : 'sidebar-close'}/>
      <span>{collapsed ? '展开侧栏' : '折叠侧栏'}</span>
    </button>
    <div id={id} className="v7-side-content">{children}</div>
  </aside>;
}

const paths: Record<string, string> = {
  creation: 'M5 3h14v18H5zM8 7h8M8 11h8M8 15h5',
  io: 'M4 16v5h16v-5M12 3v12M7 10l5 5 5-5',
  default: 'M4 4h16v16H4zM8 8h8M8 12h8M8 16h5',
};
export function NavIcon({ name }: { name: string }) {
  if (name === 'evidence' || name === 'comments' || name === 'creators' || name === 'targets' || name === 'tasks' || name === 'overview') return <IntelligenceIcon name={name} className="v7-side-icon"/>;
  return <svg className="v7-side-icon" viewBox="0 0 24 24" aria-hidden="true"><path d={paths[name] ?? paths.default}/></svg>;
}
