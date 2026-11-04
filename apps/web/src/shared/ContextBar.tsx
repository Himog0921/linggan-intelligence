import { Fragment, type ReactNode } from 'react';
import { ContextTexture } from '../app/ContextTexture';

export interface ContextBarContent {
  breadcrumb: ReactNode;
  navigation?: ReactNode;
  meta?: ReactNode;
}

/** The single shared Z02 context-rail structure. */
export function ContextBar({ breadcrumb, navigation, meta }: ContextBarContent) {
  return (
    <div className="v7-context-row" data-context-texture="dark">
      <ContextTexture />
      <div aria-hidden="true" />
      <div className="v7-context-main">
        <nav className="v7-context-crumb" aria-label="当前位置">{breadcrumb}</nav>
        {navigation ? <div className="v7-context-section-nav">{navigation}</div> : null}
        <div className="v7-context-meta">{meta}</div>
      </div>
    </div>
  );
}

export function ContextBreadcrumb({ items }: { items: readonly ReactNode[] }) {
  return (
    <>
      {items.map((item, index) => (
        <Fragment key={index}>
          {index > 0 ? <span className="v7-slash" aria-hidden="true">/</span> : null}
          <span className="v7-context-crumb-item" aria-current={index === items.length - 1 ? 'page' : undefined}>
            {item}
          </span>
        </Fragment>
      ))}
    </>
  );
}
