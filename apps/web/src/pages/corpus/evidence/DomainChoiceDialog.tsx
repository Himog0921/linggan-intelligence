import { useId } from 'react';
import { Link } from 'react-router';
import { ModalDialog } from '../../../corpus/ModalDialog';
import type { ShellDomain } from '../../../corpus/corpusShellApi';
import styles from './Dialogs.module.css';

interface DomainChoiceDialogProps {
  domains: readonly ShellDomain[];
  /** 地址带着合法 `work` 但还没选领域：每个领域链接都带上它，选定领域后继续定位这篇作品（F09）。 */
  pendingWork: string | null;
  onClose: () => void;
}

/** 选择区间的说明与旧页一致：暂停的领域仍可读历史材料；作品数未知写「作品数未知」，不写 0。 */
function choiceMeta(domain: ShellDomain): string {
  if (domain.status === 'paused') return '已暂停 · 历史材料可读';
  const works = domain.sampleCount === null ? '作品数未知' : `${domain.sampleCount} 篇作品`;
  return `运行中 · ${domain.targetCount} 个目标 · ${works}`;
}

/** 关闭后焦点回到面包屑里的领域选择器（或没有领域时的管理链接）：键盘用户不会被丢回页面顶部。 */
function focusDomainEntry() {
  window.setTimeout(() => document.querySelector<HTMLElement>('.v7-domain-picker summary, [data-domain-manage]')?.focus(), 0);
}

/**
 * 第一次进入语料页还没有领域：外壳保持完整，弹出聚焦的领域选择。关闭只回到「请选择研究领域」空态，
 * 不自动代选、不读材料。一个领域也没有时写明，并给出前往领域管理的出口。
 */
export function DomainChoiceDialog({ domains, pendingWork, onClose }: DomainChoiceDialogProps) {
  const titleId = useId();
  const copyId = useId();
  const close = () => {
    onClose();
    focusDomainEntry();
  };
  return (
    <ModalDialog labelledBy={titleId} className={styles.dialog} onClose={close}>
      <div className={styles.form} aria-describedby={copyId}>
        <header className={styles.head}>
          <div>
            <small>语料范围</small>
            <h2 id={titleId}>选择研究领域</h2>
          </div>
          <button type="button" className={styles.textButton} aria-label="关闭领域选择" onClick={close}>
            关闭
          </button>
        </header>
        <div className={styles.body}>
          <p id={copyId} className={styles.copy}>
            选择后读取该领域的作品材料；也可随后在页头切换。
          </p>
          {domains.length === 0 ? (
            <>
              <p>还没有可选择的研究领域。</p>
              <a className={styles.manage} href="/collection/domains" data-domain-manage="">
                前往领域管理 →
              </a>
            </>
          ) : (
            <nav className={styles.choices} aria-label="选择语料领域">
              {domains.map(domain => (
                <Link
                  key={domain.domainRef}
                  className={styles.choice}
                  to={`/corpus/evidence?domain=${encodeURIComponent(domain.domainRef)}${pendingWork ? `&work=${encodeURIComponent(pendingWork)}` : ''}`}
                  aria-label={`${domain.name}，${choiceMeta(domain)}`}
                >
                  <span className={styles.choiceName}>{domain.name}</span>
                  <span className={styles.choiceMeta}>{choiceMeta(domain)}</span>
                  <span className={styles.choiceArrow} aria-hidden="true">
                    →
                  </span>
                </Link>
              ))}
            </nav>
          )}
        </div>
      </div>
    </ModalDialog>
  );
}

