import { useId, type ReactNode } from 'react';
import { AppShell } from '../app/AppShell';
import { CorpusSideNav } from './CorpusSideNav';
import type { CorpusPageKey } from './corpusPages';
import type { CorpusShell } from './corpusShellApi';
import { DomainPicker } from './DomainPicker';
import styles from './CorpusFrame.module.css';

interface CorpusFrameProps {
  /** 当前页，决定侧栏高亮和领域切换后停留的页面。 */
  page: CorpusPageKey;
  /** 页面名：面包屑末级、页面标题（读屏）和文档标题。 */
  pageLabel: string;
  /** 页面路径，领域切换后停留在这里（`/corpus/creators`）。 */
  action: string;
  /** 语料外壳；读取中或读取失败时为 null——此时不显示选择器，侧栏也不带领域。 */
  shell: CorpusShell | null;
  /** 页头右侧读数，由页面自己提供。 */
  meta: ReactNode;
  /** 侧栏底部说明本页的数据范围。 */
  foot: ReactNode;
  children: ReactNode;
}

/**
 * 语料三页共用的页面框架：全局页头 + 语料侧栏 + 可滚动内容区（WEB-REACT-001 阶段 D）。
 * 页头、侧栏、领域选择器的位置和样式与旧页面同源（shell.css）；领域在三页之间切换不丢（F09）。
 * 一级导航「采集」项的状态词与领域列表同源（语料外壳接口），三页一致；外壳没读到时保持「状态未知」。
 */
export function CorpusFrame({ page, pageLabel, action, shell, meta, foot, children }: CorpusFrameProps) {
  const titleId = useId();
  const picker = shell ? <DomainPicker domains={shell.domains} selected={shell.selectedDomain} action={action} /> : null;
  const crumb = picker ? (
    <>
      语料 <span className="v7-slash">/</span> {picker} <span className="v7-slash">/</span> <b>{pageLabel}</b>
    </>
  ) : (
    <>
      语料 <span className="v7-slash">/</span> <b>{pageLabel}</b>
    </>
  );
  return (
    <AppShell
      active="corpus"
      boundaryLabel="本机服务状态"
      collectionState={shell?.collectionState}
      crumb={crumb}
      meta={meta}
    >
      <title>{`${pageLabel} · 语料`}</title>
      <div className={`v7-shell ${styles.frame}`}>
        <CorpusSideNav active={page} domain={shell?.selectedDomain ?? null} foot={foot} />
        <main className={styles.main} aria-labelledby={titleId}>
          <h1 id={titleId} className="v7-sr-only">
            {pageLabel}
          </h1>
          {children}
        </main>
      </div>
    </AppShell>
  );
}
