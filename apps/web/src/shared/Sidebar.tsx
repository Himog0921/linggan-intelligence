import { useContext, useEffect, useId, useState, type ComponentPropsWithoutRef, type MouseEvent, type ReactNode } from 'react';
import { SidebarState } from './sidebarState';
import { IntelligenceIcon, type IntelligenceIconName } from './IntelligenceIcon';

export interface SidebarNavigationItem {
  id: string;
  label: string;
  index?: string;
  code?: string;
  icon?: string;
  href?: string;
  onSelect?: () => void;
  disabled?: boolean;
  disabledReason?: string;
  badge?: string | number;
  badgeTitle?: string;
  children?: readonly SidebarNavigationItem[];
}

interface SidebarProps extends ComponentPropsWithoutRef<'aside'> {
  eyebrow?: string;
  title?: string;
  sectionLabel?: string;
  sectionCode?: string;
  items?: readonly SidebarNavigationItem[];
  activeId?: string;
  activeParentId?: string;
  foot?: ReactNode;
}

/** Shared shell slot. Collapsing changes presentation only, never route/query or business scope. */
export function Sidebar({
  children,
  className = '',
  eyebrow,
  title,
  sectionLabel = '页面导航',
  sectionCode = 'LEVEL 02 / 03',
  items,
  activeId,
  activeParentId,
  foot,
  ...props
}: SidebarProps) {
  const { collapsed, toggle } = useContext(SidebarState);
  const id = useId();
  const [expandedGroups, setExpandedGroups] = useState<string[]>(() => activeParentId ? [activeParentId] : []);

  useEffect(() => {
    setExpandedGroups(activeParentId ? [activeParentId] : []);
  }, [activeParentId]);

  function toggleGroup(groupId: string) {
    setExpandedGroups(current => current.includes(groupId)
      ? current.filter(value => value !== groupId)
      : [...current, groupId]);
  }

  const renderLink = (item: SidebarNavigationItem, level: 'secondary' | 'tertiary', index?: string, parent = false) => {
    const current = item.id === activeId;
    const icon = level === 'secondary' && item.icon ? <NavIcon name={item.icon} /> : null;
    const classes = level === 'secondary'
      ? `v7-side-nav v7-side-nav-secondary${parent ? ' v7-side-nav-parent' : ''}`
      : 'v7-side-subnav-item';
    const content = level === 'secondary' ? (
      <>
        <span className="v7-nav-index">{index ?? item.index ?? '00'}</span>
        {icon}
        <span className="v7-nav-label">{item.label}</span>
        {item.badge !== undefined && item.badge !== null && item.badge !== '' ? (
          <b className="v7-nav-badge" title={item.badgeTitle}>{item.badge}</b>
        ) : null}
      </>
    ) : item.label;

    if (item.disabled) {
      return <span key={item.id} className={classes} aria-disabled="true" title={item.disabledReason ?? item.label}>{content}</span>;
    }
    if (item.href) {
      const handleClick = (event: MouseEvent<HTMLAnchorElement>) => {
        if (!item.onSelect || event.button !== 0 || event.metaKey || event.ctrlKey || event.shiftKey || event.altKey) return;
        event.preventDefault();
        item.onSelect();
      };
      return (
        <a
          key={item.id}
          className={classes}
          href={item.href}
          aria-label={level === 'secondary' ? item.label : undefined}
          title={item.label}
          aria-current={current ? 'page' : undefined}
          data-code={level === 'tertiary' ? item.code : undefined}
          onClick={handleClick}
        >
          {content}
        </a>
      );
    }
    if (item.onSelect) {
      return (
        <button
          key={item.id}
          type="button"
          className={classes}
          aria-current={current ? 'page' : undefined}
          data-code={level === 'tertiary' ? item.code : undefined}
          onClick={item.onSelect}
        >
          {content}
        </button>
      );
    }
    return <span key={item.id} className={classes} aria-disabled="true" title={item.label}>{content}</span>;
  };

  return (
    <aside {...props} className={`v7-side ${className}`}>
      <div className="v7-side-head">
        <div className="v7-side-kicker" hidden={!eyebrow && !title}>
          {eyebrow ? <strong>{eyebrow}</strong> : null}
          {title ? <span>{title}</span> : null}
        </div>
        <button
          type="button"
          className="v7-side-toggle"
          aria-label={collapsed ? '展开侧栏' : '折叠侧栏'}
          aria-expanded={!collapsed}
          aria-controls={id}
          onClick={toggle}
          title={collapsed ? '展开侧栏' : '折叠侧栏'}
        >
          <IntelligenceIcon name={collapsed ? 'sidebar-open' : 'sidebar-close'} />
          <span>{collapsed ? '展开侧栏' : '折叠侧栏'}</span>
        </button>
      </div>
      <div id={id} className="v7-side-content">
        {items ? (
          <>
            <div className="v7-nav-section-label">
              <span>{sectionLabel}</span>
              <b>{sectionCode}</b>
            </div>
            <nav className="v7-side-navigation" aria-label="页面导航">
              {items.map((item, index) => {
                const children = item.children ?? [];
                if (!children.length) return renderLink(item, 'secondary', String(index + 1).padStart(2, '0'));
                const parentActive = item.id === activeParentId || item.id === activeId || children.some(child => child.id === activeId);
                const expanded = expandedGroups.includes(item.id);
                const subnavId = `${id}-${item.id.replace(/[^a-zA-Z0-9_-]/g, '-')}`;
                return (
                  <div key={item.id} className="v7-nav-group" data-active-parent={parentActive || undefined}>
                    <div className="v7-nav-parent-row">
                      {renderLink(item, 'secondary', String(index + 1).padStart(2, '0'), true)}
                      <button
                        type="button"
                        className="v7-nav-disclosure"
                        aria-label={`${expanded ? '收起' : '展开'}${item.label}子页面`}
                        aria-expanded={expanded}
                        aria-controls={subnavId}
                        onClick={() => toggleGroup(item.id)}
                        title={expanded ? '收起子页面' : '展开子页面'}
                      >
                        <IntelligenceIcon name="chevron" />
                      </button>
                    </div>
                    <div id={subnavId} className="v7-nav-subnav" hidden={!expanded}>
                      {children.map(child => renderLink(child, 'tertiary'))}
                    </div>
                  </div>
                );
              })}
            </nav>
          </>
        ) : children}
        {foot ? <div className="v7-side-foot">{foot}</div> : null}
      </div>
    </aside>
  );
}

const intelligenceIcons: readonly IntelligenceIconName[] = [
  'evidence', 'comments', 'creators', 'sidebar-open', 'sidebar-close', 'clock', 'info', 'check', 'chevron',
  'refresh', 'copy', 'expand', 'close', 'normal', 'wide', 'focus', 'external', 'back', 'forward', 'search',
  'filter', 'add', 'edit', 'grid', 'list', 'zoom-in', 'zoom-out', 'prev', 'next', 'fit', 'targets', 'tasks',
  'overview', 'publications', 'accounts', 'daily', 'diagnostics', 'security', 'link', 'archive', 'restore',
  'pause', 'resume', 'stop', 'sticky', 'frame-add', 'adjust', 'upload', 'move-up', 'move-down',
  'password-show', 'password-hide', 'download',
];

const paths: Record<string, string> = {
  creation: 'M5 3h14v18H5zM8 7h8M8 11h8M8 15h5',
  io: 'M4 16v5h16v-5M12 3v12M7 10l5 5 5-5',
  topic: 'M4 5h16v14H4zM7 9h10M7 13h6M7 16h4',
  default: 'M4 4h16v16H4zM8 8h8M8 12h8M8 16h5',
};

export function NavIcon({ name }: { name: string }) {
  if (intelligenceIcons.includes(name as IntelligenceIconName)) {
    return <IntelligenceIcon name={name as IntelligenceIconName} className="v7-side-icon" />;
  }
  return <svg className="v7-side-icon" viewBox="0 0 24 24" aria-hidden="true"><path d={paths[name] ?? paths.default}/></svg>;
}
