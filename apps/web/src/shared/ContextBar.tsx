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

/**
 * The single breadcrumb renderer: items are separated by `/`, and the last item is the current
 * page — bold, marked with the hand-drawn line and exposed as `aria-current`. Pages pass content only.
 */
export function ContextBreadcrumb({ items }: { items: readonly ReactNode[] }) {
  return (
    <>
      {items.map((item, index) => (
        <Fragment key={index}>
          {index > 0 ? <span className="v7-slash" aria-hidden="true">/</span> : null}
          {index === items.length - 1 ? (
            <b className="v7-annotated-current" aria-current="page">
              {item}
              <svg viewBox="0 0 54 8" preserveAspectRatio="none" aria-hidden="true" focusable="false">
                <path d="M2 4.5C13 2 29 6 52 3.5M7 6c15-1 26-.8 39-1.4" />
              </svg>
            </b>
          ) : item}
        </Fragment>
      ))}
    </>
  );
}
