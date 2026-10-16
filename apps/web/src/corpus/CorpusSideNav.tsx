import type { ReactNode } from 'react';
import { CORPUS_ENTRIES, type CorpusPageKey } from './corpusPages';

/** 证据库页填写的侧栏计数：本次读取的作品数，及悬停说明（读模型不给总数，所以只说「本次读取」）。 */
export interface EvidenceCount {
  count: number;
  title: string;
}

interface CorpusSideNavProps {
  active: CorpusPageKey;
  /** 当前已确认的领域。没有选定领域时链接保持裸路径，不传入虚假的读取范围。 */
  domain: string | null;
  /** 侧栏底部的一句话，由所在页面说明自己的数据范围。 */
  foot: ReactNode;
  /** 只有证据库页会填；没有或零作品时徽标保持隐藏，标记与 shell.rs 逐字相同。 */
  evidenceCount?: EvidenceCount | null;
}

/**
 * 语料模块的二级导航。结构、类名与 shell.rs 的 `corpus_side_nav` 一致，样式只来自 shell.css。
 *
 * 当前领域由这里统一带上：领域由整个语料模块承载，任何一个子页都无权在自己的链接里把它丢掉
 * （F09）。三项指向的页面多数仍由旧服务提供，所以用普通链接整页跳转，不交给前端路由。
 */
export function CorpusSideNav({ active, domain, foot, evidenceCount = null }: CorpusSideNavProps) {
  return (
    <aside className="v7-side" aria-label="语料导航">
      {CORPUS_ENTRIES.map(entry => {
        if (entry.href === null) {
          return (
            <span key={entry.ordinal} className="v7-side-nav" aria-disabled="true">
              <i>{entry.ordinal}</i>
              <span>{entry.label}</span>
            </span>
          );
        }
        const href = domain ? `${entry.href}?domain=${encodeURIComponent(domain)}` : entry.href;
        return (
          <a key={entry.ordinal} className="v7-side-nav" href={href} aria-current={entry.page === active ? 'page' : undefined}>
            <i>{entry.ordinal}</i>
            <span>{entry.label}</span>
            {/* 证据库的计数徽标位：由证据库页填写，其他页保持隐藏。 */}
            {entry.page === 'evidence' ? (
              evidenceCount && evidenceCount.count > 0 ? (
                <b className="ev-rail-count" id="ev-rail-count" title={evidenceCount.title}>
                  {evidenceCount.count}
                </b>
              ) : (
                <b className="ev-rail-count" id="ev-rail-count" hidden />
              )
            ) : null}
          </a>
        );
      })}
      <div className="v7-side-foot">{foot}</div>
    </aside>
  );
}
