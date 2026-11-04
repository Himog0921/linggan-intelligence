import { DomainScopePicker } from '../../shared/DomainScopePicker';

interface Scope { domainRef: string; name: string; accountCount?: number }

/** Business scope semantics stay here; disclosure behavior and visuals are shared. */
export function OpsScopePicker({ options, value, onChange, disabled, loading, failed, creation = false }: {
  options: readonly Scope[]; value: string; onChange: (value: string) => void;
  disabled?: boolean; loading?: boolean; failed?: boolean; creation?: boolean;
}) {
  const selected = options.find(option => option.domainRef === value);
  const name = selected?.name ?? (creation ? value ? '领域不可用' : '选择领域' : '全部业务');
  const unavailable = disabled || loading || failed;
  const label = creation ? '创作领域' : '切换业务';
  return (
    <DomainScopePicker
      label={label}
      disabled={unavailable}
      summary={
        <>
          <span>{loading ? '读取领域…' : failed ? '领域读取失败' : name}</span>
          <small>{creation ? '创作领域' : selected ? `${selected.accountCount ?? '—'} 个账号` : '全部账号'}</small>
          <i aria-hidden="true" />
        </>
      }
    >
      <nav aria-label={creation ? '可选创作领域' : '可选业务'}>
        {(!creation ? [{ domainRef: 'all', name: '全部业务' }, ...options] : options).map(option => (
          <button
            type="button"
            key={option.domainRef}
            aria-current={value === option.domainRef ? 'page' : undefined}
            disabled={unavailable}
            onClick={event => {
              if (unavailable) return;
              onChange(option.domainRef);
              const details = event.currentTarget.closest('details');
              details?.removeAttribute('open');
              details?.querySelector('summary')?.focus();
            }}
          >
            <span>{option.name}</span>
            {'accountCount' in option && option.accountCount !== undefined ? <small>{option.accountCount} 个账号</small> : null}
          </button>
        ))}
        {creation && !options.length ? <span>暂无可用创作领域</span> : null}
      </nav>
    </DomainScopePicker>
  );
}
