import type { ReactNode } from 'react';
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
        <div className="v7-context-crumb">{breadcrumb}</div>
        {navigation ? <div className="v7-context-section-nav">{navigation}</div> : null}
        <div className="v7-context-meta">{meta}</div>
      </div>
    </div>
  );
}
