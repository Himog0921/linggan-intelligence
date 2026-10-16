import { useRef, useState } from 'react';
import { Link } from 'react-router';
import { domainMeta, type ShellDomain } from './corpusShellApi';

interface DomainPickerProps {
  domains: readonly ShellDomain[];
  /** 当前已确认的领域；null 表示还没选。 */
  selected: string | null;
  /** 切换后停留的页面路径。领域切换只带 `domain`：此前领域下的检索词和筛选不能跟去描述另一批材料。 */
  action: string;
}

/**
 * 面包屑里的领域选择器（照搬 shell.rs 的 `render_domain_picker`，语料侧）。
 *
 * 领域是当前观察对象，不是页面内的筛选项，所以用 disclosure + 链接而不是原生 select：菜单的打开态、
 * 焦点和当前项由 LIDS 接管（样式来自 shell.css 的 `.v7-domain-picker*`）。只有一个领域且已选中，
 * 或一个领域也没有时，整个控件不渲染——一个永远只有一项的下拉是噪音。
 */
export function DomainPicker({ domains, selected, action }: DomainPickerProps) {
  const [open, setOpen] = useState(false);
  const summary = useRef<HTMLElement>(null);
  const current = domains.find(domain => domain.domainRef === selected) ?? null;
  if (domains.length === 0 || (domains.length < 2 && current !== null)) return null;
  return (
    <details
      className="v7-domain-picker"
      open={open}
      onToggle={event => setOpen(event.currentTarget.open)}
      onKeyDown={event => {
        if (event.key === 'Escape' && open) {
          setOpen(false);
          summary.current?.focus();
        }
      }}
    >
      <summary ref={summary} aria-label="切换当前观察领域">
        <span>{current ? current.name : '选择领域'}</span>
        <small>{current ? domainMeta(current) : '请选择一个领域'}</small>
        <i aria-hidden="true" />
      </summary>
      <nav aria-label="可选观察领域">
        {domains.map(domain => {
          const meta = domainMeta(domain);
          return (
            <Link
              key={domain.domainRef}
              to={`${action}?domain=${encodeURIComponent(domain.domainRef)}`}
              aria-current={domain.domainRef === selected ? 'page' : undefined}
              aria-label={`${domain.name}，${meta}`}
              onClick={() => setOpen(false)}
            >
              <span>{domain.name}</span>
              <small>{meta}</small>
            </Link>
          );
        })}
      </nav>
    </details>
  );
}
